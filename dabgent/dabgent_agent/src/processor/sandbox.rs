use super::{Aggregate, Processor, thread};
use crate::event::{Event, TypedToolResult, ToolKind};
use crate::llm::{CompletionResponse, FinishReason};
use crate::toolbox::{ToolCallExt, ToolDyn};
use dabgent_mq::{EventDb, EventStore, Query};
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use std::path::Path;
use crate::sandbox_seed::{collect_template_files, write_template_files};


pub struct ToolProcessor<E: EventStore> {
    sandbox: Box<dyn SandboxDyn>,
    event_store: E,
    tools: Vec<Box<dyn ToolDyn>>,
    recipient: Option<String>,
}

impl<E: EventStore> Processor<Event> for ToolProcessor<E> {
    async fn run(&mut self, event: &EventDb<Event>) -> eyre::Result<()> {
        match &event.data {
            Event::SeedSandboxFromTemplate { template_path, base_path } => {
                // Seed sandbox from template on host filesystem
                let template_path = Path::new(template_path);
                if !template_path.exists() {
                    tracing::warn!("Template path does not exist: {:?}", template_path);
                } else {
                    match collect_template_files(template_path, base_path) {
                        Err(err) => {
                            tracing::error!("Failed to collect template files: {:?}", err);
                        }
                        Ok(tf) => {
                            let template_hash = tf.hash.clone();
                            let template_path_str = template_path.display().to_string();

                            let file_count = tf.files.len();
                            if let Err(err) = write_template_files(&mut self.sandbox, &tf.files).await {
                                tracing::error!("Failed to write template files to sandbox: {:?}", err);
                            } else {
                                let seeded = Event::SandboxSeeded {
                                    template_path: template_path_str,
                                    base_path: base_path.clone(),
                                    file_count,
                                    template_hash: Some(template_hash),
                                };
                                self.event_store
                                    .push_event(&event.stream_id, &event.aggregate_id, &seeded, &Default::default())
                                    .await?;
                            }
                        }
                    }
                }
            }
            // Phase 1: AgentMessage with ToolUse -> emit ToolResult
            Event::AgentMessage {
                response,
                recipient,
                ..
            } if response.finish_reason == FinishReason::ToolUse
                && recipient.eq(&self.recipient) =>
            {
                let tool_results = self.run_tools(&response, &event.stream_id, &event.aggregate_id).await?;

                // Don't emit ToolResult for delegation trigger tools
                // DelegationProcessor will handle these directly from AgentMessage
                let non_delegation_results: Vec<_> = tool_results.into_iter()
                    .filter(|tr| !self.is_delegation_trigger_tool(tr))
                    .collect();

                if !non_delegation_results.is_empty() {
                    // Push ToolResult event only for non-delegation tools
                    let tool_result_event = Event::ToolResult(non_delegation_results.clone());
                    self.event_store.push_event(
                        &event.stream_id,
                        &event.aggregate_id,
                        &tool_result_event,
                        &Default::default(),
                    ).await?;

                    // Convert ToolResults to UserMessage for non-delegation tools
                    if self.is_done_tool_result(&non_delegation_results) && self.should_trigger_compaction(&non_delegation_results) {
                        // Trigger compaction via delegation to compact_error tool
                        let compact_tool_result = TypedToolResult {
                            tool_name: ToolKind::Other("compact_error".to_string()),
                            result: rig::message::ToolResult {
                                id: "compact_trigger".to_string(),
                                call_id: None,
                                content: rig::OneOrMany::one(rig::message::ToolResultContent::Text(
                                    rig::message::Text { text: "Triggering compaction for large Done tool result".to_string() }
                                )),
                            },
                        };
                        let compact_event = Event::ToolResult(vec![compact_tool_result]);
                        self.event_store.push_event(
                            &event.stream_id,
                            &event.aggregate_id,
                            &compact_event,
                            &Default::default(),
                        ).await?;
                    } else {
                        // Convert to UserMessage for normal processing
                        let tools = non_delegation_results.iter().map(|t|
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
            }

            _ => {}
        }
        Ok(())
    }
}

impl<E: EventStore> ToolProcessor<E> {
    pub fn new(
        sandbox: Box<dyn SandboxDyn>,
        event_store: E,
        tools: Vec<Box<dyn ToolDyn>>,
        recipient: Option<String>,
    ) -> Self {
        Self {
            sandbox,
            event_store,
            tools,
            recipient,
        }
    }

    async fn run_tools(
        &mut self,
        response: &CompletionResponse,
        stream_id: &str,
        aggregate_id: &str,
    ) -> Result<Vec<TypedToolResult>> {
        let mut results = Vec::new();
        for content in response.choice.iter() {
            if let rig::message::AssistantContent::ToolCall(call) = content {
                let tool = self.tools.iter().find(|t| t.name() == call.function.name);
                let result = match tool {
                    Some(tool) => {
                        let args = call.function.arguments.clone();
                        let tool_result = tool.call(args, &mut self.sandbox).await?;

                        // Check if this is a successful DoneTool call
                        if call.function.name == "done" && tool_result.is_ok() {
                            tracing::info!("Task completed successfully, emitting TaskCompleted event");

                            // Extract summary from Done tool arguments
                            let summary = call.function.arguments
                                .get("summary")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Task completed")
                                .to_string();

                            let task_completed_event = Event::TaskCompleted {
                                success: true,
                                summary
                            };
                            self.event_store
                                .push_event(
                                    stream_id,
                                    aggregate_id,
                                    &task_completed_event,
                                    &Default::default(),
                                )
                                .await?;
                        }

                        // Check if this is a successful FinishDelegationTool call
                        if call.function.name == "finish_delegation" && tool_result.is_ok() {
                            tracing::info!("Delegated work completed successfully, emitting WorkComplete event");

                            // Extract summary from finish_delegation tool arguments
                            let summary = call.function.arguments
                                .get("summary")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Delegated work completed")
                                .to_string();

                            // For now, we'll create a minimal WorkComplete event
                            // Later we can extract parent info if needed
                            let work_complete_event = Event::WorkComplete {
                                agent_type: "databricks_explorer".to_string(),
                                result: summary,
                                parent: crate::event::ParentAggregate {
                                    aggregate_id: "unknown".to_string(), // Will be populated properly later
                                    tool_id: None,
                                },
                            };
                            self.event_store
                                .push_event(
                                    stream_id,
                                    aggregate_id,
                                    &work_complete_event,
                                    &Default::default(),
                                )
                                .await?;
                        }

                        tool_result
                    }
                    None => {
                        let error = format!("{} not found", call.function.name);
                        Err(serde_json::json!(error))
                    }
                };
                results.push(TypedToolResult {
                    tool_name: match call.function.name.as_str() {
                        "done" => ToolKind::Done,
                        other => ToolKind::Other(other.to_string())
                    },
                    result: call.to_result(result)
                });
            }
        }

        Ok(results)
    }

    fn is_delegation_trigger_tool(&self, result: &TypedToolResult) -> bool {
        match &result.tool_name {
            ToolKind::Other(tool_name) => {
                // These tools trigger delegation but don't return results to main thread
                tool_name == "explore_databricks_catalog" || tool_name == "compact_error"
            }
            _ => false,
        }
    }


    fn is_done_tool_result(&self, results: &[TypedToolResult]) -> bool {
        results.iter().any(|t| t.tool_name == ToolKind::Done)
    }

    fn should_trigger_compaction(&self, results: &[TypedToolResult]) -> bool {
        let size = self.calculate_text_size(results);
        size > 2048 // Use standard compaction threshold
    }

    fn calculate_text_size(&self, results: &[TypedToolResult]) -> usize {
        results
            .iter()
            .map(|result| {
                result.result
                    .content
                    .iter()
                    .map(|content| match content {
                        rig::message::ToolResultContent::Text(text) => text.text.len(),
                        _ => 0, // Skip non-text content for size calculation
                    })
                    .sum::<usize>()
            })
            .sum()
    }

}
