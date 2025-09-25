use super::Processor;
use crate::event::{Event, TypedToolResult, ToolKind};
use crate::llm::{CompletionResponse, FinishReason};
use crate::toolbox::{ToolCallExt, ToolDyn};
use dabgent_mq::{EventDb, EventStore};
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

                // Check for delegation tools and emit DelegateWork events
                for typed_result in &tool_results {
                    if let ToolKind::Other(tool_name) = &typed_result.tool_name {
                        if tool_name == "explore_databricks_catalog" {
                            // Extract arguments from the original tool call in the response
                            if let Some(tool_call) = self.find_tool_call_by_id(&response, &typed_result.result.id) {
                                tracing::info!("Databricks exploration requested, emitting DelegateWork event");

                                let catalog = tool_call.function.arguments.get("catalog")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("main");
                                let prompt = tool_call.function.arguments.get("prompt")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Explore the catalog for relevant data");

                                let delegate_event = Event::DelegateWork {
                                    agent_type: "databricks_explorer".to_string(),
                                    prompt: format!("Explore catalog '{}': {}", catalog, prompt),
                                    parent_tool_id: typed_result.result.id.clone(),
                                };
                                self.event_store
                                    .push_event(
                                        &event.stream_id,
                                        &event.aggregate_id,
                                        &delegate_event,
                                        &Default::default(),
                                    )
                                    .await?;
                            }
                        }
                    }
                }

                let tool_result_event = Event::ToolResult(tool_results);

                self.event_store.push_event(
                    &event.stream_id,
                    &event.aggregate_id,
                    &tool_result_event,
                    &Default::default(),
                ).await?;
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

    fn find_tool_call_by_id<'a>(&self, response: &'a CompletionResponse, tool_id: &str) -> Option<&'a rig::message::ToolCall> {
        response.choice.iter().find_map(|content| {
            if let rig::message::AssistantContent::ToolCall(call) = content {
                if call.id == tool_id {
                    Some(call)
                } else {
                    None
                }
            } else {
                None
            }
        })
    }
}
