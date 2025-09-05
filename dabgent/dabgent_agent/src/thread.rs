use crate::{handler::Handler, llm::CompletionResponse};
use rig::completion::Message;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub state: State,
}

impl Thread {
    pub fn new() -> Self {
        Self { state: State::None }
    }
}

impl Default for Thread {
    fn default() -> Self {
        Self::new()
    }
}

impl Handler for Thread {
    type Command = Command;
    type Event = Event;
    type Error = Error;

    fn process(&mut self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        match (&self.state, command) {
            (State::None, Command::Prompt(prompt)) => Ok(vec![Event::Prompted(prompt)]),
            (State::Tool | State::User, Command::Completion(completion)) => {
                let message = completion.message();
                Ok(vec![Event::LlmCompleted(message)])
            }
            (state, command) => Err(Error::Other(format!(
                "Invalid command {:?} for state {:?}",
                command, state
            ))),
        }
    }

    fn fold(events: &[Self::Event]) -> Self {
        let mut thread = Self::new();
        for event in events {
            match event {
                Event::Prompted(prompt) => {
                    thread.prompt = Some(prompt.clone());
                }
                _ => unimplemented!(),
            }
        }
        thread
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Prompt(String),
    Completion(CompletionResponse),
    Tool(Message),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Prompted(String),
    LlmCompleted(Message),
    ToolCompleted(Message),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Agent error: {0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum State {
    None,
    User,
    Tool,
    Done,
    Fail(String),
}
