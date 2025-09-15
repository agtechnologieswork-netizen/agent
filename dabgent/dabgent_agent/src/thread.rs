use crate::{handler::Handler, llm::CompletionResponse};
use rig::completion::Message;
use serde::{Deserialize, Serialize};

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
            (State::Agent, Command::Tool(response)) => Ok(vec![Event::ToolCompleted(response)]),
            // FixMe: handle large error with compact somewhere here
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
                    // Raw tool results don't update the thread state directly
                    // They will be processed by CompactWorker and converted to ToolCompleted
                }
                Event::ToolCompleted(response) => {
                    thread.state = match thread.is_done(response) {
                        true => State::Done,
                        false => State::Tool,
                    };
                    thread.messages.push(response.message());
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
}

impl dabgent_mq::Event for Event {
    const EVENT_VERSION: &'static str = "1.0";

    fn event_type(&self) -> &'static str {
        match self {
            Event::Prompted(..) => "prompted",
            Event::LlmCompleted(..) => "llm_completed",
            Event::ToolCompletedRaw(..) => "tool_completed_raw",
            Event::ToolCompleted(..) => "tool_completed",
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

#[cfg(test)]
mod tests {
    use super::*;
    use dabgent_mq::Event as EventTrait;

    #[test]
    fn test_thread_fold_with_raw_events() {
        // Create a ToolResponse for testing
        let tool_response = ToolResponse {
            content: rig::OneOrMany::one(rig::message::UserContent::text("test result".to_string())),
        };

        // Create events including ToolCompletedRaw
        let events = vec![
            Event::Prompted("Test prompt".to_string()),
            Event::ToolCompletedRaw(tool_response.clone()),
            Event::ToolCompleted(tool_response),
        ];

        // Test that Thread::fold handles the events correctly
        let thread = Thread::fold(&events);
        
        // After folding, thread should be in Tool state (from ToolCompleted)
        // and should have 2 messages (Prompted + ToolCompleted, ToolCompletedRaw is ignored)
        assert!(matches!(thread.state, State::Tool));
        assert_eq!(thread.messages.len(), 2);
    }

    #[test]
    fn test_event_type_mapping() {
        let events = [
            Event::Prompted("test".to_string()),
            Event::LlmCompleted(crate::llm::CompletionResponse { 
                choice: rig::OneOrMany::one(rig::message::AssistantContent::Text(
                    rig::message::Text { text: "response".to_string() }
                )),
                finish_reason: crate::llm::FinishReason::Stop,
                output_tokens: 10,
            }),
            Event::ToolCompletedRaw(ToolResponse {
                content: rig::OneOrMany::one(rig::message::UserContent::text("raw".to_string())),
            }),
            Event::ToolCompleted(ToolResponse {
                content: rig::OneOrMany::one(rig::message::UserContent::text("processed".to_string())),
            }),
        ];

        // Test that event types map correctly
        assert_eq!(events[0].event_type(), "prompted");
        assert_eq!(events[1].event_type(), "llm_completed");
        assert_eq!(events[2].event_type(), "tool_completed_raw");
        assert_eq!(events[3].event_type(), "tool_completed");
    }
}
