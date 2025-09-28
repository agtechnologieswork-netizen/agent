use super::{Aggregate, Processor};
use crate::event::{Event, TypedToolResult, ToolKind};
use crate::processor::thread;
use crate::toolbox::{Tool, ToolDyn, ToolCallExt};
use crate::llm::CompletionResponse;
use dabgent_mq::{EventDb, EventStore, Query};
use dabgent_sandbox::SandboxDyn;
use async_trait::async_trait;
use eyre::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

pub mod databricks;
pub mod compaction;

#[async_trait]
pub trait DelegationHandler: Send + Sync {
    fn trigger_tool(&self) -> &str;
    fn thread_prefix(&self) -> &str;
    fn worker_name(&self) -> &str;

    // Handler owns its sandbox and tools
    fn tools(&self) -> &[Box<dyn ToolDyn>];

    // Execute a tool by name - this avoids borrowing conflicts
    async fn execute_tool_by_name(
        &mut self,
        tool_name: &str,
        args: serde_json::Value
    ) -> eyre::Result<Result<serde_json::Value, serde_json::Value>>;

    // Check if this handler should process a specific event
    fn should_handle_tools(&self, event: &EventDb<Event>) -> bool {
        if let Event::AgentMessage { recipient: Some(r), .. } = &event.data {
            r == self.worker_name() && event.aggregate_id.starts_with(self.thread_prefix())
        } else {
            false
        }
    }

    fn handle(
        &self,
        catalog: &str,
        prompt: &str,
        model: &str,
        parent_aggregate_id: &str,
        parent_tool_id: &str
    ) -> Result<(String, Event, Event)>;
    fn format_result(&self, summary: &str) -> String;
}

pub struct DelegationProcessor<E: EventStore> {
    event_store: E,
    default_model: String,
    handlers: Vec<Box<dyn DelegationHandler>>,
    compaction_threshold: Option<usize>,
}

impl<E: EventStore> Processor<Event> for DelegationProcessor<E> {
    async fn run(&mut self, event: &EventDb<Event>) -> eyre::Result<()> {
        match &event.data {
            Event::AgentMessage { response, .. } if self.has_delegation_trigger_tool_call(response) => {
                tracing::info!(
                    "Delegation trigger tool call detected for aggregate {}",
                    event.aggregate_id
                );
                self.handle_delegation_trigger(event, response).await?;
            }
            Event::ToolResult(tool_results) if self.is_delegation_tool_result(tool_results) => {
                tracing::info!(
                    "Delegation tool result detected for aggregate {}",
                    event.aggregate_id
                );
                self.handle_delegation_request(event, tool_results).await?;
            }
            Event::AgentMessage { response, .. } if self.is_delegated_tool_execution(event) => {
                tracing::info!(
                    "Tool execution detected for delegated thread {}",
                    event.aggregate_id
                );
                self.handle_tool_execution(event, response).await?;
            }
            Event::ToolResult(tool_results) if !self.is_delegation_tool_result(tool_results) => {
                // Skip non-delegation tool results - they're handled by their respective ToolProcessors
            }
            Event::WorkComplete { result, .. } if self.is_delegated_thread(&event.aggregate_id) => {
                tracing::info!(
                    "Delegated work completed successfully for aggregate {}",
                    event.aggregate_id,
                );
                self.handle_work_completion(event, result).await?;
            }
            _ => {}
        }
        Ok(())
    }
}

impl<E: EventStore> DelegationProcessor<E> {
    pub fn new(event_store: E, default_model: String, handlers: Vec<Box<dyn DelegationHandler>>) -> Self {
        // Extract compaction threshold if a compaction handler exists
        let compaction_threshold = handlers.iter()
            .find(|h| h.trigger_tool() == "compact_error")
            .map(|_| 2048_usize); // Default threshold, could be made configurable

        Self {
            event_store,
            default_model,
            handlers,
            compaction_threshold,
        }
    }

    fn has_delegation_trigger_tool_call(&self, response: &CompletionResponse) -> bool {
        response.choice.iter().any(|content| {
            if let rig::message::AssistantContent::ToolCall(call) = content {
                self.handlers.iter().any(|h| h.trigger_tool() == call.function.name)
            } else {
                false
            }
        })
    }

    async fn handle_delegation_trigger(&mut self, event: &EventDb<Event>, response: &CompletionResponse) -> eyre::Result<()> {
        // Extract trigger tool calls and start delegation for each
        for content in response.choice.iter() {
            if let rig::message::AssistantContent::ToolCall(call) = content {
                if let Some(handler_idx) = self.handlers.iter().position(|h| h.trigger_tool() == call.function.name) {
                    // Extract catalog and prompt from arguments
                    let catalog = call.function.arguments
                        .get("catalog")
                        .and_then(|v| v.as_str())
                        .unwrap_or("main");

                    let prompt = call.function.arguments
                        .get("prompt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    // Create delegation using handler
                    let (task_thread_id, config_event, user_event) = self.handlers[handler_idx]
                        .handle(catalog, prompt, &self.default_model, &event.aggregate_id, &call.id)?;

                    // Push events to start delegation
                    self.event_store.push_event(
                        &event.stream_id,
                        &task_thread_id,
                        &config_event,
                        &Default::default()
                    ).await?;

                    self.event_store.push_event(
                        &event.stream_id,
                        &task_thread_id,
                        &user_event,
                        &Default::default()
                    ).await?;
                }
            }
        }
        Ok(())
    }

    fn is_delegated_thread(&self, aggregate_id: &str) -> bool {
        self.handlers.iter().any(|h| aggregate_id.starts_with(h.thread_prefix()))
    }

    fn is_delegation_tool_result(&self, tool_results: &[crate::event::TypedToolResult]) -> bool {
        // Check for explicit delegation tool results
        let has_explicit_delegation = tool_results.iter().any(|result| {
            if let crate::event::ToolKind::Other(tool_name) = &result.tool_name {
                self.handlers.iter().any(|h| tool_name == h.trigger_tool())
            } else {
                false
            }
        });

        // Check for auto-compaction trigger (Done tools with large content)
        let should_auto_compact = self.should_auto_compact(tool_results);

        has_explicit_delegation || should_auto_compact
    }

    fn should_auto_compact(&self, tool_results: &[crate::event::TypedToolResult]) -> bool {
        // Check if we have a compaction handler and threshold configured
        if let Some(threshold) = self.compaction_threshold {
            return tool_results.iter().any(|result| {
                // Must be a Done tool result
                result.tool_name == crate::event::ToolKind::Done &&
                // Must not be a delegation result (avoid recursion)
                !self.has_delegation_tool_result(tool_results) &&
                // Must exceed size threshold
                self.calculate_text_size(&[result.clone()]) > threshold
            });
        }

        false
    }

    fn has_delegation_tool_result(&self, results: &[crate::event::TypedToolResult]) -> bool {
        results.iter().any(|result| {
            match &result.tool_name {
                crate::event::ToolKind::Other(tool_name) => {
                    // Check if this tool belongs to any delegation handler
                    self.handlers.iter().any(|handler| {
                        handler.trigger_tool() == tool_name ||
                        handler.tools().iter().any(|t| t.name() == *tool_name)
                    })
                }
                _ => false,
            }
        })
    }

    fn calculate_text_size(&self, results: &[crate::event::TypedToolResult]) -> usize {
        results
            .iter()
            .map(|result| {
                result.result
                    .content
                    .iter()
                    .map(|content| match content {
                        rig::message::ToolResultContent::Text(text) => text.text.len(),
                        _ => 0,
                    })
                    .sum::<usize>()
            })
            .sum()
    }

    fn extract_text_content(&self, results: &[crate::event::TypedToolResult]) -> String {
        results
            .iter()
            .flat_map(|result| {
                result.result.content.iter().filter_map(|content| match content {
                    rig::message::ToolResultContent::Text(text) => Some(text.text.clone()),
                    _ => None,
                })
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    async fn handle_delegation_request(
        &mut self,
        event: &EventDb<Event>,
        tool_results: &[crate::event::TypedToolResult],
    ) -> eyre::Result<()> {
        // Check for auto-compaction first
        if self.should_auto_compact(tool_results) {
            return self.handle_auto_compaction(event, tool_results).await;
        }

        // Handle explicit delegation tool results
        let delegation_result = tool_results.iter().find(|result| {
            if let crate::event::ToolKind::Other(tool_name) = &result.tool_name {
                self.handlers.iter().any(|h| tool_name == h.trigger_tool())
            } else {
                false
            }
        });

        if let Some(delegation_result) = delegation_result {
            let parent_tool_id = delegation_result.result.id.clone();

            // Find matching handler index
            let handler_idx = if let crate::event::ToolKind::Other(tool_name) = &delegation_result.tool_name {
                self.handlers.iter().position(|h| tool_name == h.trigger_tool())
            } else {
                None
            };

            if let Some(handler_idx) = handler_idx {
                // Load events to find the original tool call with arguments
                let query = Query::stream(&event.stream_id).aggregate(&event.aggregate_id);
                let events = self.event_store.load_events::<Event>(&query, None).await?;

                // Find the most recent AgentMessage with the matching tool call
                let tool_call = events.iter().rev()
                    .find_map(|e| match e {
                        Event::AgentMessage { response, .. } => {
                            response.choice.iter().find_map(|content| {
                                if let rig::message::AssistantContent::ToolCall(call) = content {
                                    if call.id == parent_tool_id {
                                        Some(call)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                        }
                        _ => None,
                    });

                if let Some(tool_call) = tool_call {
                    // Extract arguments from the tool call
                    let catalog = tool_call.function.arguments.get("catalog")
                        .and_then(|v| v.as_str())
                        .unwrap_or("main"); // Default to "main" if not provided
                    let prompt_arg = tool_call.function.arguments.get("prompt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Explore the catalog for relevant data");

                    self.handle_delegation_by_index(event, handler_idx, catalog, prompt_arg, &parent_tool_id).await?;
                } else {
                    return Err(eyre::eyre!(
                        "Could not find original tool call with id '{}' for delegation",
                        parent_tool_id
                    ));
                }
            }
        }

        Ok(())
    }

    async fn handle_auto_compaction(
        &mut self,
        event: &EventDb<Event>,
        tool_results: &[crate::event::TypedToolResult],
    ) -> eyre::Result<()> {
        // Find compaction handler
        let handler_idx = self.handlers.iter()
            .position(|h| h.trigger_tool() == "compact_error")
            .ok_or_else(|| eyre::eyre!("Compaction handler not found"))?;

        // Find the Done tool result that needs compaction
        let done_result = tool_results.iter().find(|result| {
            result.tool_name == crate::event::ToolKind::Done
        }).ok_or_else(|| eyre::eyre!("No Done tool result found for compaction"))?;

        let parent_tool_id = done_result.result.id.clone();
        let error_text = self.extract_text_content(&[done_result.clone()]);

        // Use empty catalog since it's not relevant for compaction
        self.handle_delegation_by_index(event, handler_idx, "", &error_text, &parent_tool_id).await
    }

    async fn handle_delegation_by_index(
        &mut self,
        event: &EventDb<Event>,
        handler_idx: usize,
        catalog: &str,
        prompt_arg: &str,
        parent_tool_id: &str,
    ) -> eyre::Result<()> {
        let (task_thread_id, config_event, user_event) = self.handlers[handler_idx].handle(
            catalog,
            prompt_arg,
            &self.default_model,
            &event.aggregate_id,
            parent_tool_id,
        )?;

        // Send LLMConfig first with parent tracking
        self.event_store
            .push_event(
                &event.stream_id,
                &task_thread_id,
                &config_event,
                &Default::default(),
            )
            .await?;

        // Send the exploration task
        self.event_store
            .push_event(
                &event.stream_id,
                &task_thread_id,
                &user_event,
                &Default::default(),
            )
            .await?;

        Ok(())
    }

    async fn handle_work_completion(
        &mut self,
        event: &EventDb<Event>,
        summary: &str,
    ) -> eyre::Result<()> {
        // Load task thread to get parent info from LLMConfig
        let task_query = Query::stream(&event.stream_id).aggregate(&event.aggregate_id);
        let task_events = self.event_store.load_events::<Event>(&task_query, None).await?;

        // Find the LLMConfig event to get parent info
        let parent_info = task_events.iter()
            .find_map(|e| match e {
                Event::LLMConfig { parent, .. } => parent.as_ref(),
                _ => None,
            });

        if let Some(parent) = parent_info {
            // Find matching handler based on thread prefix
            let handler = self.handlers.iter()
                .find(|h| event.aggregate_id.starts_with(h.thread_prefix()));

            if let Some(handler) = handler {
                // Check if this is compaction (needs ToolResult) or other delegation (needs UserMessage)
                if handler.thread_prefix() == "compact_" {
                    // For compaction, send ToolResult with original tool_id to replace the Done tool result
                    if let Some(tool_id) = &parent.tool_id {
                        let compacted_result = vec![TypedToolResult {
                            tool_name: ToolKind::Done,
                            result: rig::message::ToolResult {
                                id: tool_id.clone(),
                                call_id: None,
                                content: rig::OneOrMany::one(rig::message::ToolResultContent::Text(
                                    summary.into()
                                )),
                            },
                        }];

                        // Convert ToolResult directly to UserMessage for original thread
                        let tools = compacted_result.iter().map(|t| rig::message::UserContent::ToolResult(t.result.clone()));
                        let user_content = rig::OneOrMany::many(tools)?;

                        // Load original thread state and process
                        let original_query = Query::stream(&event.stream_id).aggregate(&parent.aggregate_id);
                        let events = self.event_store.load_events::<Event>(&original_query, None).await?;
                        let mut thread = thread::Thread::fold(&events);
                        let new_events = thread.process(thread::Command::User(user_content))?;

                        for new_event in new_events.iter() {
                            self.event_store
                                .push_event(
                                    &event.stream_id,
                                    &parent.aggregate_id,
                                    new_event,
                                    &Default::default(),
                                )
                                .await?;
                        }
                    }
                } else {
                    // For other delegations, send formatted UserMessage text
                    let result_content = handler.format_result(summary);

                    let user_content = rig::OneOrMany::one(rig::message::UserContent::Text(
                        rig::message::Text {
                            text: result_content,
                        }
                    ));

                    // Load original thread state and process
                    let original_query = Query::stream(&event.stream_id).aggregate(&parent.aggregate_id);
                    let events = self.event_store.load_events::<Event>(&original_query, None).await?;
                    let mut thread = thread::Thread::fold(&events);
                    let new_events = thread.process(thread::Command::User(user_content))?;

                    for new_event in new_events.iter() {
                        self.event_store
                            .push_event(
                                &event.stream_id,
                                &parent.aggregate_id,
                                new_event,
                                &Default::default(),
                            )
                            .await?;
                    }
                }
            }
        }

        Ok(())
    }

    fn is_delegated_tool_execution(&self, event: &EventDb<Event>) -> bool {
        self.handlers.iter().any(|h| h.should_handle_tools(event))
    }

    async fn handle_tool_execution(
        &mut self,
        event: &EventDb<Event>,
        response: &CompletionResponse
    ) -> eyre::Result<()> {
        // Find the handler for this event
        let handler_idx = self.handlers.iter()
            .position(|h| h.should_handle_tools(event))
            .ok_or_else(|| eyre::eyre!("No handler found for tool execution"))?;

        let mut tool_results = Vec::new();

        // Collect tool calls first to avoid borrowing issues
        let mut tool_calls = Vec::new();
        for content in response.choice.iter() {
            if let rig::message::AssistantContent::ToolCall(call) = content {
                tool_calls.push(call.clone());
            }
        }

        // Execute each tool call using the handler's execute_tool_by_name method
        for call in tool_calls {
            let tool_name = call.function.name.clone();
            let args = call.function.arguments.clone();

            // Execute using the handler's method which handles borrowing internally
            let result = self.handlers[handler_idx]
                .execute_tool_by_name(&tool_name, args)
                .await?;

            // Check if this is a successful terminal tool call using the tool object
            let tool = self.handlers[handler_idx].tools()
                .iter()
                .find(|t| t.name() == tool_name);

            if let Some(tool) = tool {
                if tool.is_terminal() && result.is_ok() {
                    tracing::info!("Terminal tool completed successfully, emitting WorkComplete event");

                    // Extract result from finish_delegation arguments (now unified)
                    let summary = call.function.arguments
                        .get("result")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| eyre::eyre!("Missing 'result' argument in finish_delegation tool call"))?
                        .to_string();

                    let work_complete_event = Event::WorkComplete {
                        agent_type: "delegated_worker".to_string(),
                        result: summary,
                        parent: crate::event::ParentAggregate {
                            aggregate_id: event.aggregate_id.clone(),
                            tool_id: Some(call.id.clone()),
                        },
                    };

                    self.event_store.push_event(
                        &event.stream_id,
                        &event.aggregate_id,
                        &work_complete_event,
                        &Default::default()
                    ).await?;
                }
            }

            let tool_result = call.to_result(result);
            tool_results.push(TypedToolResult {
                tool_name: ToolKind::Other(tool_name),
                result: tool_result,
            });
        }

        if !tool_results.is_empty() {
            // Push the ToolResult event first
            self.event_store.push_event(
                &event.stream_id,
                &event.aggregate_id,
                &Event::ToolResult(tool_results.clone()),
                &Default::default()
            ).await?;

            // Convert ToolResults to UserMessage only if they're not from terminal tools
            // Terminal tools complete the delegated work and don't need further LLM processing
            let non_terminal_results: Vec<_> = tool_results.iter()
                .filter(|tr| {
                    if let ToolKind::Other(tool_name) = &tr.tool_name {
                        // Check if this tool is terminal by finding it in handler tools
                        let is_terminal = self.handlers[handler_idx].tools()
                            .iter()
                            .find(|t| t.name() == *tool_name)
                            .map(|t| t.is_terminal())
                            .unwrap_or(false);
                        !is_terminal
                    } else {
                        true // Non-other tools are not terminal
                    }
                })
                .collect();

            if !non_terminal_results.is_empty() {
                let tools = non_terminal_results.iter().map(|t|
                    rig::message::UserContent::ToolResult(t.result.clone())
                );
                let user_content = rig::OneOrMany::many(tools)?;

                // Load thread state and process the UserMessage
                let query = Query::stream(&event.stream_id).aggregate(&event.aggregate_id);
                let events = self.event_store.load_events::<Event>(&query, None).await?;
                let mut thread = thread::Thread::fold(&events);
                let new_events = thread.process(thread::Command::User(user_content))?;

                // Push the new events (including UserMessage and any LLM responses)
                for new_event in new_events.iter() {
                    self.event_store
                        .push_event(
                            &event.stream_id,
                            &event.aggregate_id,
                            new_event,
                            &Default::default(),
                        )
                        .await?;
                }
            }
        }

        Ok(())
    }
}

// Unified terminal tool for all delegation handlers
#[derive(Deserialize, Serialize)]
pub struct FinishDelegationArgs {
    pub result: String,
}

#[derive(Serialize)]
pub struct FinishDelegationOutput {
    pub success: String,
}

pub struct FinishDelegationTool;

impl Tool for FinishDelegationTool {
    type Args = FinishDelegationArgs;
    type Output = FinishDelegationOutput;
    type Error = serde_json::Value;

    fn name(&self) -> String {
        "finish_delegation".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Tool::name(self),
            description: "Complete the delegated work with a result summary".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "result": {
                        "type": "string",
                        "description": "The result of the delegated work"
                    }
                },
                "required": ["result"]
            }),
        }
    }

    fn is_terminal(&self) -> bool {
        true
    }

    async fn call(
        &self,
        args: Self::Args,
        _sandbox: &mut Box<dyn SandboxDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        Ok(Ok(FinishDelegationOutput {
            success: format!("Delegated work completed: {}", args.result),
        }))
    }
}