use crate::event::{Event, ToolKind, TypedToolResult};
use crate::llm::{CompletionResponse, FinishReason};
use crate::sandbox_seed::{collect_template_files, write_template_files};
use crate::toolbox::{ToolCallExt, ToolDyn};
<<<<<<< HEAD
use dabgent_mq::{Aggregate, Callback, Envelope, EventStore, Handler};
=======
use dabgent_mq::{EventDb, EventStore};
>>>>>>> origin
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    template_path: String,
    base_path: String,
    file_count: usize,
    template_hash: Option<String>,
}

pub enum Command {
    Seed {
        template_path: String,
        base_path: String,
    },
    RunTools {
        response: CompletionResponse,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Template error: {0}")]
    TemplateError(String),
    #[error("Tool execution error: {0}")]
    ToolError(String),
}

pub struct SandboxServices {
    pub sandbox: Arc<tokio::sync::Mutex<Box<dyn SandboxDyn>>>,
    pub tools: Arc<Vec<Box<dyn ToolDyn>>>,
}

#[derive(Default)]
pub struct SandboxAggregate {
    config: Option<Config>,
}

impl Aggregate for SandboxAggregate {
    const TYPE: &'static str = "sandbox";
    type Command = Command;
    type Event = Event;
    type Error = Error;
    type Services = SandboxServices;

    async fn handle(
        &self,
        cmd: Self::Command,
        services: &Self::Services,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match cmd {
            Command::Seed {
                template_path,
                base_path,
            } => {
                let template_path_obj = Path::new(&template_path);
                if !template_path_obj.exists() {
                    return Err(Error::TemplateError(format!(
                        "Template path does not exist: {:?}",
                        template_path_obj
                    )));
                }
                let tf = collect_template_files(template_path_obj, &base_path)
                    .map_err(|e| Error::TemplateError(e.to_string()))?;
                let template_hash = tf.hash.clone();
                let file_count = tf.files.len();
                let mut sandbox = services.sandbox.lock().await;
                write_template_files(&mut sandbox, &tf.files)
                    .await
                    .map_err(|e| Error::TemplateError(e.to_string()))?;
                Ok(vec![Event::SandboxSeeded {
                    template_path,
                    base_path,
                    file_count,
                    template_hash: Some(template_hash),
                }])
            }
            Command::RunTools { response } => {
                let mut events = Vec::new();
                let mut sandbox = services.sandbox.lock().await;

                for content in response.choice.iter() {
                    if let rig::message::AssistantContent::ToolCall(call) = content {
                        let tool = services
                            .tools
                            .iter()
                            .find(|t| t.name() == call.function.name);
                        let result = match tool {
                            Some(tool) => {
                                let args = call.function.arguments.clone();
                                let tool_result = tool
                                    .call(args, &mut sandbox)
                                    .await
                                    .map_err(|e| Error::ToolError(e.to_string()))?;

                                if call.function.name == "done" && tool_result.is_ok() {
                                    tracing::info!(
                                        "Task completed successfully, emitting TaskCompleted event"
                                    );
                                    events.push(Event::TaskCompleted { success: true });
                                }
                                tool_result
                            }
                            None => {
                                let error = format!("{} not found", call.function.name);
                                Err(serde_json::json!(error))
                            }
                        };
                        let typed_result = TypedToolResult {
                            tool_name: match call.function.name.as_str() {
                                "done" => ToolKind::Done,
                                other => ToolKind::Other(other.to_string()),
                            },
                            result: call.to_result(result),
                        };
                        if events.is_empty() || !matches!(events.last(), Some(Event::ToolResult(_)))
                        {
                            events.push(Event::ToolResult(vec![typed_result]));
                        } else if let Some(Event::ToolResult(results)) = events.last_mut() {
                            results.push(typed_result);
                        }
                    }
                }

                Ok(events)
            }
        }
    }

    fn apply(&mut self, event: Self::Event) {
        match event {
            Event::SandboxSeeded {
                template_path,
                base_path,
                file_count,
                template_hash,
            } => {
                self.config = Some(Config {
                    template_path,
                    base_path,
                    file_count,
                    template_hash,
                });
            }
            _ => {}
        }
    }
}

pub struct ToolCallback<ES: EventStore> {
    handler: Handler<SandboxAggregate, ES>,
    recipient: Option<String>,
}

impl<ES: EventStore> ToolCallback<ES> {
    pub fn new(handler: Handler<SandboxAggregate, ES>, recipient: Option<String>) -> Self {
        Self { handler, recipient }
    }
}

impl<ES: EventStore> Callback<SandboxAggregate> for ToolCallback<ES> {
    async fn process(&mut self, event: &Envelope<SandboxAggregate>) -> Result<()> {
        match &event.data {
            Event::AgentMessage {
                response,
                recipient,
            } if response.finish_reason == FinishReason::ToolUse
                && recipient.as_ref() == self.recipient.as_ref() =>
            {
                self.handler
                    .execute(
                        &event.aggregate_id,
                        Command::RunTools {
                            response: response.clone(),
                        },
                    )
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }
}
