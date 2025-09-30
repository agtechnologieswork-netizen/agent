use crate::llm::CompletionResponse;
use dabgent_mq::{Aggregate, Event as MQEvent};
use rig::message::{ToolCall, ToolResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Start {
        initial_message: String,
    },
    OnThreadResponse(CompletionResponse),
    OnToolsExecuted(Vec<ToolResult>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Started {
        workflow_id: Uuid,
        thread_id: String,
        sandbox_id: String,
    },
    ThreadMessageRequested(rig::OneOrMany<rig::message::UserContent>),
    ThreadCompletionRequested,
    ThreadResponseReceived(CompletionResponse),
    ToolExecutionRequested {
        sandbox_id: String,
        calls: Vec<ToolCall>,
    },
    ToolsCompleted(Vec<ToolResult>),
    Completed,
}

impl MQEvent for Event {
    fn event_version(&self) -> String {
        "1.0".to_owned()
    }

    fn event_type(&self) -> String {
        match self {
            Event::Started { .. } => "started",
            Event::ThreadMessageRequested(..) => "thread_message_requested",
            Event::ThreadCompletionRequested => "thread_completion_requested",
            Event::ThreadResponseReceived(..) => "thread_response_received",
            Event::ToolExecutionRequested { .. } => "tool_execution_requested",
            Event::ToolsCompleted(..) => "tools_completed",
            Event::Completed => "completed",
        }
        .to_owned()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid state transition")]
    InvalidState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerState {
    Idle,
    AwaitingThreadResponse,
    ExecutingTools,
    Completed,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Worker {
    pub workflow_id: Option<Uuid>,
    pub thread_id: Option<String>,
    pub sandbox_id: Option<String>,
    pub state: Option<WorkerState>,
}

impl Aggregate for Worker {
    const TYPE: &'static str = "worker";
    type Command = Command;
    type Event = Event;
    type Error = Error;
    type Services = ();

    async fn handle(
        &self,
        cmd: Self::Command,
        _services: &Self::Services,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match cmd {
            Command::Start { initial_message } => {
                let workflow_id = Uuid::new_v4();
                let thread_id = format!("thread-{}", workflow_id);
                let sandbox_id = format!("sandbox-{}", workflow_id);

                Ok(vec![
                    Event::Started {
                        workflow_id,
                        thread_id,
                        sandbox_id,
                    },
                    Event::ThreadMessageRequested(rig::OneOrMany::one(
                        rig::message::UserContent::Text(initial_message.into()),
                    )),
                    Event::ThreadCompletionRequested,
                ])
            }
            Command::OnThreadResponse(response) => {
                use crate::llm::FinishReason;

                let tool_calls: Vec<ToolCall> = response
                    .choice
                    .iter()
                    .filter_map(|content| {
                        if let rig::message::AssistantContent::ToolCall(call) = content {
                            Some(call.clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                let mut events = vec![Event::ThreadResponseReceived(response.clone())];

                if !tool_calls.is_empty() {
                    events.push(Event::ToolExecutionRequested {
                        sandbox_id: self.sandbox_id.clone().unwrap(),
                        calls: tool_calls,
                    });
                } else if matches!(response.finish_reason, FinishReason::Stop) {
                    events.push(Event::Completed);
                }

                Ok(events)
            }
            Command::OnToolsExecuted(results) => {
                let user_contents: Vec<rig::message::UserContent> = results
                    .iter()
                    .map(|r| rig::message::UserContent::ToolResult(r.clone()))
                    .collect();

                let content = rig::OneOrMany::many(user_contents)
                    .map_err(|_| Error::InvalidState)?;

                Ok(vec![
                    Event::ToolsCompleted(results.clone()),
                    Event::ThreadMessageRequested(content),
                    Event::ThreadCompletionRequested,
                ])
            }
        }
    }

    fn apply(&mut self, event: Self::Event) {
        match event {
            Event::Started {
                workflow_id,
                thread_id,
                sandbox_id,
            } => {
                self.workflow_id = Some(workflow_id);
                self.thread_id = Some(thread_id);
                self.sandbox_id = Some(sandbox_id);
                self.state = Some(WorkerState::Idle);
            }
            Event::ThreadCompletionRequested => {
                self.state = Some(WorkerState::AwaitingThreadResponse);
            }
            Event::ToolExecutionRequested { .. } => {
                self.state = Some(WorkerState::ExecutingTools);
            }
            Event::Completed => {
                self.state = Some(WorkerState::Completed);
            }
            _ => {}
        }
    }
}