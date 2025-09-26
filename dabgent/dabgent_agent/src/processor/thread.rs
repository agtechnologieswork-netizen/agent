use crate::Event;
use crate::llm::{Completion, LLMClientDyn};
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
        recipient: Option<String>,
    },
    Completion,
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
    pub recipient: Option<String>,
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
                recipient,
            } => Ok(vec![Event::LLMConfig {
                model,
                temperature: temperature,
                max_tokens: max_tokens,
                preamble,
                tools,
                recipient,
                parent: None,
            }]),
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
                recipient,
                parent: _,
            } => {
                self.model = Some(model);
                self.temperature = Some(temperature);
                self.max_tokens = Some(max_tokens);
                self.preamble = preamble;
                self.tools = tools;
                self.recipient = recipient;
            }
            Event::AgentMessage { response, .. } => {
                self.messages.push(response.message());
            }
            Event::UserMessage(content) => {
                self.messages.push(rig::completion::Message::User {
                    content: content.clone(),
                });
            }
            _ => {}
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
            Ok(response) => Ok(vec![Event::AgentMessage {
                response,
                recipient: self.recipient.clone(),
            }]),
            Err(_) => Err(Error::LLMCall),
        }
    }
}

pub struct CompletionCallback<ES: EventStore> {
    handler: Handler<Thread, ES>,
}

impl<ES: EventStore> Callback<Thread> for CompletionCallback<ES> {
    async fn process(&mut self, event: &Envelope<Thread>) -> Result<()> {
        if matches!(event.data, Event::UserMessage(..)) {
            self.handler
                .execute(&event.aggregate_id, Command::Completion)
                .await?;
        }
        Ok(())
    }
}
