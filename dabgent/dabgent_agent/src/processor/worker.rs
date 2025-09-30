use crate::llm::CompletionResponse;
use dabgent_mq::{Aggregate, Event as MQEvent};
use rig::message::{ToolCall, ToolResult, UserContent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Start {
        message: String,
        thread_id: String,
        sandbox_id: String,
    },
    OnThreadResponse(CompletionResponse),
    OnToolsExecuted(Vec<ToolResult>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Started {
        thread_id: String,
        sandbox_id: String,
    },
    CompletionRequested {
        thread_id: String,
        content: rig::OneOrMany<rig::message::UserContent>,
    },
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
            Event::CompletionRequested { .. } => "completion_requested",
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
    #[error("Unexpected tool result with id: {0}")]
    UnexpectedToolId(String),
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
    pub thread_id: Option<String>,
    pub sandbox_id: Option<String>,
    pub state: Option<WorkerState>,
    pub pending_calls: HashMap<String, Option<ToolResult>>,
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
            Command::Start {
                message,
                thread_id,
                sandbox_id,
            } => Ok(vec![
                Event::Started {
                    thread_id: thread_id.clone(),
                    sandbox_id,
                },
                Event::CompletionRequested {
                    thread_id,
                    content: rig::OneOrMany::one(UserContent::text(message)),
                },
            ]),
            Command::OnThreadResponse(response) => match response.tool_calls() {
                Some(calls) => Ok(vec![Event::ToolExecutionRequested {
                    sandbox_id: self.maybe_sandbox_id()?,
                    calls,
                }]),
                None => Ok(vec![Event::Completed]),
            },
            Command::OnToolsExecuted(results) => {
                let mut events = vec![Event::ToolsCompleted(results.clone())];
                if let Some(completed) = self.try_complete_calls(&results)? {
                    let content = completed.into_iter().map(UserContent::ToolResult);
                    let content = rig::OneOrMany::many(content).expect("At least one tool result");
                    events.push(Event::CompletionRequested {
                        thread_id: self.maybe_thread_id()?,
                        content,
                    });
                }
                Ok(events)
            }
        }
    }

    fn apply(&mut self, event: Self::Event) {
        match event {
            Event::Started {
                thread_id,
                sandbox_id,
            } => {
                self.thread_id = Some(thread_id);
                self.sandbox_id = Some(sandbox_id);
                self.state = Some(WorkerState::Idle);
            }
            Event::CompletionRequested { .. } => {
                self.pending_calls.clear();
                self.state = Some(WorkerState::AwaitingThreadResponse);
            }
            Event::ToolExecutionRequested { calls, .. } => {
                for call in calls.into_iter() {
                    self.pending_calls.insert(call.id, None);
                }
                self.state = Some(WorkerState::ExecutingTools);
            }
            Event::ToolsCompleted(results) => {
                for result in results.into_iter() {
                    self.pending_calls.insert(result.id.clone(), Some(result));
                }
            }
            Event::Completed => {
                self.state = Some(WorkerState::Completed);
            }
        }
    }
}

impl Worker {
    fn try_complete_calls(&self, results: &[ToolResult]) -> Result<Option<Vec<ToolResult>>, Error> {
        let mut completed: HashMap<_, _> = self
            .pending_calls
            .iter()
            .filter_map(|(id, call)| call.as_ref().map(|call| (id.clone(), call)))
            .collect();
        for call in results.iter() {
            if completed.contains_key(&call.id) || !self.pending_calls.contains_key(&call.id) {
                return Err(Error::UnexpectedToolId(call.id.clone()));
            }
            completed.insert(call.id.clone(), call);
        }
        if completed.len() != self.pending_calls.len() {
            return Ok(None);
        }
        Ok(Some(completed.into_values().cloned().collect()))
    }

    fn maybe_sandbox_id(&self) -> Result<String, Error> {
        self.sandbox_id.clone().ok_or(Error::InvalidState)
    }

    fn maybe_thread_id(&self) -> Result<String, Error> {
        self.thread_id.clone().ok_or(Error::InvalidState)
    }
}
