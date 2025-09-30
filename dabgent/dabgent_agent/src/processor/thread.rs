use crate::llm::{Completion, CompletionResponse, LLMClientDyn};
use dabgent_mq::Event as MQEvent;
use dabgent_mq::{Aggregate, Callback, Envelope, EventStore, Handler};
use eyre::Result;
use rig::completion::ToolDefinition;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Setup {
        model: String,
        temperature: f64,
        max_tokens: u64,
        preamble: Option<String>,
        tools: Option<Vec<ToolDefinition>>,
    },
    User(rig::OneOrMany<rig::message::UserContent>),
    Completion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    LLMConfig {
        model: String,
        temperature: f64,
        max_tokens: u64,
        preamble: Option<String>,
        tools: Option<Vec<rig::completion::ToolDefinition>>,
    },
    AgentMessage(CompletionResponse),
    UserMessage(rig::OneOrMany<rig::message::UserContent>),
}

impl MQEvent for Event {
    fn event_version(&self) -> String {
        "1.0".to_owned()
    }

    fn event_type(&self) -> String {
        match self {
            Event::LLMConfig { .. } => "llm_config",
            Event::AgentMessage { .. } => "agent_message",
            Event::UserMessage { .. } => "user_message",
        }
        .to_owned()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Model is not configured")]
    Uninitialized,
    #[error("Wrong turn")]
    WrongTurn,
    #[error("LLM call error")]
    LLMCall,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Thread {
    pub model: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u64>,
    pub preamble: Option<String>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub messages: Vec<rig::completion::Message>,
}

impl Aggregate for Thread {
    const TYPE: &'static str = "thread";
    type Command = Command;
    type Event = Event;
    type Error = Error;
    type Services = Arc<dyn LLMClientDyn>;

    async fn handle(
        &self,
        cmd: Self::Command,
        services: &Self::Services,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match cmd {
            Command::Setup {
                model,
                temperature,
                max_tokens,
                preamble,
                tools,
            } => Ok(vec![Event::LLMConfig {
                model,
                temperature: temperature,
                max_tokens: max_tokens,
                preamble,
                tools,
            }]),
            Command::User(content) => self.handle_user(&content).await,
            Command::Completion => self.handle_completion(services).await,
        }
    }

    fn apply(&mut self, event: Self::Event) {
        match event {
            Event::LLMConfig {
                model,
                temperature,
                max_tokens,
                preamble,
                tools,
            } => {
                self.model = Some(model);
                self.temperature = Some(temperature);
                self.max_tokens = Some(max_tokens);
                self.preamble = preamble;
                self.tools = tools;
            }
            Event::AgentMessage(response) => {
                self.messages.push(response.message());
            }
            Event::UserMessage(content) => {
                self.messages.push(rig::completion::Message::User {
                    content: content.clone(),
                });
            }
        }
    }
}

impl Thread {
    pub async fn handle_completion(
        &self,
        llm: &Arc<dyn LLMClientDyn>,
    ) -> Result<Vec<Event>, Error> {
        if self.model.is_none() || self.temperature.is_none() || self.max_tokens.is_none() {
            return Err(Error::Uninitialized);
        }
        match self.messages.last() {
            Some(rig::completion::Message::User { .. }) => {}
            _ => return Err(Error::WrongTurn),
        }
        let mut history = self.messages.clone();
        let message = history.pop().expect("No messages");
        let mut completion = Completion::new(self.model.clone().unwrap(), message)
            .history(history)
            .temperature(self.temperature.unwrap())
            .max_tokens(self.max_tokens.unwrap());
        if let Some(preamble) = &self.preamble {
            completion = completion.preamble(preamble.clone());
        }
        if let Some(ref tools) = self.tools {
            completion = completion.tools(tools.clone());
        }
        match llm.completion(completion).await {
            Ok(response) => Ok(vec![Event::AgentMessage(response)]),
            Err(_) => Err(Error::LLMCall),
        }
    }

    async fn handle_user(
        &self,
        content: &rig::OneOrMany<rig::message::UserContent>,
    ) -> Result<Vec<Event>, Error> {
        if self.model.is_none() || self.temperature.is_none() || self.max_tokens.is_none() {
            return Err(Error::Uninitialized);
        }
        match self.messages.last() {
            None | Some(rig::completion::Message::Assistant { .. }) => {}
            _ => return Err(Error::WrongTurn),
        }
        Ok(vec![Event::UserMessage(content.clone())])
    }
}

pub struct CompletionCallback<ES: EventStore> {
    handler: Handler<Thread, ES>,
}

impl<ES: EventStore> CompletionCallback<ES> {
    pub fn new(handler: Handler<Thread, ES>) -> Self {
        Self { handler }
    }
}

impl<ES: EventStore> Callback<Thread> for CompletionCallback<ES> {
    async fn process(&mut self, envelope: &Envelope<Thread>) -> Result<()> {
        if matches!(envelope.data, Event::UserMessage(..)) {
            self.handler
                .execute_with_metadata(
                    &envelope.aggregate_id,
                    Command::Completion,
                    envelope.metadata.clone(),
                )
                .await?;
        }
        Ok(())
    }
}
