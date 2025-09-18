use crate::{handler::Handler, llm::CompletionResponse};
use rig::completion::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

impl Handler for Thread {
    type Command = Command;
    type Event = Event;
    type Error = Error;

    fn process(&mut self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        match (&self.state, command) {
            (State::None | State::User, Command::Prompt(prompt)) => {
                Ok(vec![Event::Prompted(prompt)])
            }
            (State::User | State::Tool, Command::Completion(response)) => {
                Ok(vec![Event::LlmCompleted(response)])
            }
            (State::Agent, Command::Tool(response)) => Ok(vec![Event::ToolCompletedRaw(response)]),
            (state, command) => Err(Error::Other(format!(
                "Invalid command {command:?} for state {state:?}"
            ))),
        }
    }

    fn fold(events: &[Self::Event]) -> Self {
        let mut thread = Self::new();
        for event in events {
            match event {
                Event::Prompted(prompt) => {
                    thread.state = State::User;
                    thread.messages.push(rig::message::Message::user(prompt));
                }
                Event::LlmCompleted(response) => {
                    thread.state = match Thread::has_tool_calls(response) {
                        true => State::Agent,
                        false => State::UserWait,
                    };
                    thread.update_done_call(response);
                    thread.messages.push(response.message());
                }
                Event::ToolCompletedRaw(_) => {
                    // Raw events don't affect thread state - they're processed by CompactWorker
                }
                Event::ToolCompleted(response) => {
                    thread.state = match thread.is_done(response) {
                        true => State::Done,
                        false => State::Tool,
                    };
                    thread.messages.push(response.message());
                }
                Event::ArtifactsCollected(_) => {
                    // This event doesn't affect the thread state, it's just a side effect
                }
            }
        }
        thread
    }
}

impl Thread {
    pub fn is_done(&self, response: &ToolResponse) -> bool {
        let Some(done_id) = &self.done_call_id else {
            return false;
        };
        response.content.iter().any(|item| {
            let rig::message::UserContent::ToolResult(res) = item else {
                return false;
            };
            res.id.eq(done_id) && res.content.iter().any(|tool| {
                matches!(tool, rig::message::ToolResultContent::Text(text) if text.text == "\"success\"")
            })
        })
    }

    pub fn update_done_call(&mut self, response: &CompletionResponse) {
        for item in response.choice.iter() {
            if let rig::message::AssistantContent::ToolCall(call) = item {
                if call.function.name == "done" {
                    self.done_call_id = Some(call.id.clone());
                }
            }
        }
    }

    pub fn has_tool_calls(response: &CompletionResponse) -> bool {
        response
            .choice
            .iter()
            .any(|item| matches!(item, rig::message::AssistantContent::ToolCall(..)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Prompt(String),
    Completion(CompletionResponse),
    Tool(ToolResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Prompted(String),
    LlmCompleted(CompletionResponse),
    ToolCompletedRaw(ToolResponse),
    ToolCompleted(ToolResponse),
    ArtifactsCollected(HashMap<String, String>),
}

impl dabgent_mq::Event for Event {
    const EVENT_VERSION: &'static str = "1.0";

    fn event_type(&self) -> &'static str {
        match self {
            Event::Prompted(..) => "prompted",
            Event::LlmCompleted(..) => "llm_completed",
            Event::ToolCompletedRaw(..) => "tool_completed_raw",
            Event::ToolCompleted(..) => "tool_completed",
            Event::ArtifactsCollected(..) => "artifacts_collected",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum State {
    /// Initial state
    #[default]
    None,
    /// Waiting for user input
    UserWait,
    /// User input received
    User,
    /// Finished agent completion
    Agent,
    /// Finished tool completion
    Tool,
    /// Successfully completed the task
    Done,
    /// Failed to complete the task
    Fail(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Thread {
    pub state: State,
    pub messages: Vec<Message>,
    pub done_call_id: Option<String>,
}

impl Thread {
    pub fn new() -> Self {
        Self {
            state: State::None,
            messages: Vec::new(),
            done_call_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub content: rig::OneOrMany<rig::message::UserContent>,
}

impl ToolResponse {
    pub fn message(&self) -> rig::completion::Message {
        rig::message::Message::User {
            content: self.content.clone(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Agent error: {0}")]
    Other(String),
}
