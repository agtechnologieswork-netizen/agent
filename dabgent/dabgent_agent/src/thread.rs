use crate::llm::CompletionResponse;
use crate::{Event, Handler};
use rig::completion::Message;
use serde::{Deserialize, Serialize};

impl Handler for Thread {
    type Command = Command;
    type Event = Event;
    type Error = Error;

    fn process(&mut self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        match (&self.state, command) {
            (State::None | State::Agent, Command::Prompt(prompt)) => {
                Ok(vec![Event::Prompted(prompt)])
            }
            (State::User | State::Tool, Command::Completion(response)) => {
                Ok(vec![Event::LlmCompleted(response)])
            }
            (State::Agent, Command::Tool(response)) => Ok(vec![Event::ToolCompleted(response)]),
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
                Event::ToolCompleted(content) => {
                    thread.state = match thread.is_done(content) {
                        true => State::Done,
                        false => State::Tool,
                    };
                    thread.messages.push(rig::message::Message::User {
                        content: content.clone(),
                    });
                }
                _ => {}
            }
        }
        thread
    }
}

impl Thread {
    pub fn is_done(&self, content: &rig::OneOrMany<rig::message::UserContent>) -> bool {
        let Some(done_id) = &self.done_call_id else {
            return false;
        };
        content.iter().any(|item| {
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
    Tool(rig::OneOrMany<rig::message::UserContent>),
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Agent error: {0}")]
    Other(String),
}
