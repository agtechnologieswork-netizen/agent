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
                    // Check if compaction will be triggered before emitting tool results
                    if self.is_done_tool_result(&non_delegation_results) && self.should_trigger_compaction(&non_delegation_results) {
                        // Large Done tool - trigger compaction instead of normal processing
                        let done_result = non_delegation_results.iter()
                            .find(|r| r.tool_name == ToolKind::Done)
                            .expect("Done tool result should exist");

                        tracing::info!("Large Done tool result detected, triggering compaction delegation");

                        // Extract the error text from the Done tool result
                        let error_text = done_result.result.content.iter()
                            .filter_map(|content| match content {
                                rig::message::ToolResultContent::Text(text) => Some(text.text.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n");

                        // Emit DelegateWork event for compaction
                        let delegate_work_event = Event::DelegateWork {
                            agent_type: "compact_worker".to_string(),
                            prompt: error_text,
                            parent_tool_id: done_result.result.id.clone(),
                        };
                        self.event_store.push_event(
                            &event.stream_id,
                            &event.aggregate_id,
                            &delegate_work_event,
                            &Default::default(),
                        ).await?;
                    } else {
                        // No compaction needed - emit tool results normally and convert to UserMessage
                        let tool_result_event = Event::ToolResult(non_delegation_results.clone());
                        self.event_store.push_event(
                            &event.stream_id,
                            &event.aggregate_id,
                            &tool_result_event,
                            &Default::default(),
                        ).await?;

                        // Emit TaskCompleted event for Done tools (since no compaction)
                        if self.is_done_tool_result(&non_delegation_results) {
                            // Find the Done tool result to extract summary and check success
                            if let Some(done_result) = non_delegation_results.iter().find(|r| r.tool_name == ToolKind::Done) {
                                // Extract summary from the tool result content
                                let summary = done_result.result.content.iter()
                                    .filter_map(|content| match content {
                                        rig::message::ToolResultContent::Text(text) => Some(text.text.clone()),
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");

                                // Check if Done tool succeeded by examining the tool result content structure
                                // When DoneTool validator fails: to_result wraps as {"error": "validation error: ..."}
                                // When DoneTool validator succeeds: to_result stores plain text summary
                                let success = done_result.result.content.iter().all(|content| {
                                    match content {
                                        rig::message::ToolResultContent::Text(text) => {
                                            // Parse as JSON - if it has "error" field, Done failed
                                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text.text) {
                                                if let Some(obj) = parsed.as_object() {
                                                    !obj.contains_key("error")
                                                } else {
                                                    true // Not an object, so no error field
                                                }
                                            } else {
                                                true // Not JSON, plain text summary = success
                                            }
                                        }
                                        _ => true, // Non-text content doesn't indicate failure
                                    }
                                });

                                if success {
                                    tracing::info!("Task completed successfully, emitting TaskCompleted event");
                                } else {
                                    tracing::info!("Task completed with errors, emitting TaskCompleted event with success=false");
                                }

                                let task_completed_event = Event::TaskCompleted {
                                    success,
                                    summary: if summary.is_empty() { "Task completed".to_string() } else { summary }
                                };
                                self.event_store.push_event(
                                    &event.stream_id,
                                    &event.aggregate_id,
                                    &task_completed_event,
                                    &Default::default(),
                                ).await?;
                            }
                        }

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

                        // Note: TaskCompleted event will be handled later in tool result processing
                        // to coordinate with compaction logic

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
                        let available_tools: Vec<String> = self.tools.iter()
                            .map(|tool| tool.name())
                            .collect();
                        let error = format!(
                            "Tool '{}' does not exist. Available tools: [{}]",
                            call.function.name,
                            available_tools.join(", ")
                        );
                        Err(serde_json::json!(error))
                    }
                };
                results.push(TypedToolResult {
                    tool_name: match call.function.name.as_str() {
                        "done" => ToolKind::Done,
                        "explore_databricks_catalog" => ToolKind::ExploreDatabricksCatalog,
                        "finish_delegation" => ToolKind::FinishDelegation,
                        "compact_error" => ToolKind::CompactError,
                        other => ToolKind::Regular(other.to_string())
                    },
                    result: call.to_result(result)
                });
            }
        }

        Ok(results)
    }

    fn is_delegation_trigger_tool(&self, result: &TypedToolResult) -> bool {
        matches!(&result.tool_name,
            ToolKind::ExploreDatabricksCatalog | ToolKind::CompactError)
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
