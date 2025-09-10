use dabgent_agent::handler::Handler;
use dabgent_mq::models::Event;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChatEvent {
    UserMessage { content: String },
    AgentMessage { content: String },
}

impl Event for ChatEvent {
    const EVENT_VERSION: &'static str = "1.0";

    fn event_type(&self) -> &'static str {
        match self {
            ChatEvent::UserMessage { .. } => "user_message",
            ChatEvent::AgentMessage { .. } => "agent_message",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChatCommand {
    SendMessage(String),
    AgentRespond(String),
}

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("Cannot send message while agent is processing")]
    AgentProcessing,
    #[error("No user message to respond to")]
    NoUserMessage,
    #[error("Agent already responded")]
    AlreadyResponded,
}

#[derive(Debug, Clone, Default)]
pub struct ChatSession {
    messages: Vec<ChatEvent>,
    waiting_for_agent: bool,
}

impl ChatSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn messages(&self) -> &[ChatEvent] {
        &self.messages
    }

    pub fn can_send_message(&self) -> bool {
        !self.waiting_for_agent
    }

    pub fn last_user_message(&self) -> Option<&ChatEvent> {
        self.messages
            .iter()
            .rev()
            .find(|e| matches!(e, ChatEvent::UserMessage { .. }))
    }
}

impl Handler for ChatSession {
    type Command = ChatCommand;
    type Event = ChatEvent;
    type Error = ChatError;

    fn process(&mut self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            ChatCommand::SendMessage(content) => {
                if self.waiting_for_agent {
                    return Err(ChatError::AgentProcessing);
                }
                let event = ChatEvent::UserMessage { content };
                self.messages.push(event.clone());
                self.waiting_for_agent = true;
                Ok(vec![event])
            }
            ChatCommand::AgentRespond(content) => {
                if !self.waiting_for_agent {
                    return Err(ChatError::NoUserMessage);
                }
                let event = ChatEvent::AgentMessage { content };
                self.messages.push(event.clone());
                self.waiting_for_agent = false;
                Ok(vec![event])
            }
        }
    }

    fn fold(events: &[Self::Event]) -> Self {
        let mut session = Self::new();
        for event in events {
            session.messages.push(event.clone());
            match event {
                ChatEvent::UserMessage { .. } => {
                    session.waiting_for_agent = true;
                }
                ChatEvent::AgentMessage { .. } => {
                    session.waiting_for_agent = false;
                }
            }
        }
        session
    }
}
