use crate::llm::{Completion, CompletionResponse, LLMClient, WithRetryExt};
use crate::{Aggregate, Event, Processor};
use dabgent_mq::{EventDb, EventStore, Query};
use eyre::Result;
use rig::completion::ToolDefinition;
use serde::{Deserialize, Serialize};

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
    Agent(CompletionResponse),
    User(rig::OneOrMany<rig::message::UserContent>),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Model is not configured")]
    Uninitialized,
    #[error("Wrong turn")]
    WrongTurn,
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
    pub is_completed: bool,
}

impl Aggregate for Thread {
    type Command = Command;
    type Event = Event;
    type Error = Error;

    fn process(&mut self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        let events = match command {
            Command::Setup { .. } => self.handle_setup(command)?,
            Command::Agent(..) => self.handle_agent(command)?,
            Command::User(..) => self.handle_user(command)?,
        };
        for event in events.iter() {
            self.apply(&event);
        }
        Ok(events)
    }

    fn apply(&mut self, event: &Self::Event) {
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
                self.model = Some(model.clone());
                self.temperature = Some(temperature.clone());
                self.max_tokens = Some(max_tokens.clone());
                self.preamble = preamble.clone();
                self.tools = tools.clone();
                self.recipient = recipient.clone();
            }
            Event::AgentMessage { response, .. } => {
                self.messages.push(response.message());
            }
            Event::UserMessage(content) => {
                self.messages.push(rig::completion::Message::User {
                    content: content.clone(),
                });
            }
            Event::ToolResult(tool_results) => {
                // Check if this is a done tool result - if so, don't convert to user message
                let is_done_tool = tool_results.iter().any(|tr| matches!(tr.tool_name, crate::event::ToolKind::Done));
                tracing::debug!("Thread applying ToolResult. Done tool: {}, Tool count: {}", is_done_tool, tool_results.len());

                if !is_done_tool {
                    // Convert tool results to User message with ToolResult content
                    let tool_contents: Vec<rig::message::UserContent> = tool_results
                        .iter()
                        .map(|typed_result| {
                            // Convert TypedToolResult to ToolResult
                            let tool_result = rig::message::ToolResult {
                                id: typed_result.result.id.clone(),
                                content: typed_result.result.content.clone(),
                                call_id: None, // Add call_id if available
                            };
                            rig::message::UserContent::ToolResult(tool_result)
                        })
                        .collect();

                    if !tool_contents.is_empty() {
                        self.messages.push(rig::completion::Message::User {
                            content: rig::OneOrMany::many(tool_contents).unwrap(),
                        });
                    }
                }
            }
            Event::TaskCompleted { .. } => {
                self.is_completed = true;
            }
            _ => {}
        }
    }
}

impl Thread {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_setup(&self, command: Command) -> Result<Vec<Event>, Error> {
        match command {
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
            _ => unreachable!(),
        }
    }

    pub fn handle_user(&self, command: Command) -> Result<Vec<Event>, Error> {
        if self.model.is_none() || self.temperature.is_none() || self.max_tokens.is_none() {
            return Err(Error::Uninitialized);
        }
        match command {
            Command::User(content) => match self.messages.last() {
                None | Some(rig::completion::Message::Assistant { .. }) => {
                    Ok(vec![Event::UserMessage(content)])
                }
                _ => {
                    tracing::warn!("Rejecting UserMessage - last message is not Assistant. Last: {:?}",
                        self.messages.last().map(|m| match m {
                            rig::completion::Message::User { .. } => "User",
                            rig::completion::Message::Assistant { .. } => "Assistant",
                        }));
                    Err(Error::WrongTurn)
                }
            },
            _ => unreachable!(),
        }
    }

    pub fn handle_agent(&self, command: Command) -> Result<Vec<Event>, Error> {
        match command {
            Command::Agent(response) => match self.messages.last() {
                Some(rig::completion::Message::User { .. }) => Ok(vec![Event::AgentMessage {
                    response,
                    recipient: self.recipient.clone(),
                }]),
                _ => Err(Error::WrongTurn),
            },
            _ => unreachable!(),
        }
    }
}

pub struct ThreadProcessor<T: LLMClient, E: EventStore> {
    llm: T,
    event_store: E,
    recipient_filter: Option<String>,
}

impl<T: LLMClient, E: EventStore> Processor<Event> for ThreadProcessor<T, E> {
    async fn run(&mut self, event: &EventDb<Event>) -> eyre::Result<()> {
        let query = Query::stream(&event.stream_id).aggregate(&event.aggregate_id);
        match &event.data {
            Event::UserMessage(..) | Event::ToolResult(..) => {
                tracing::info!("ThreadProcessor processing event for aggregate {}: {:?}",
                    event.aggregate_id,
                    match &event.data {
                        Event::UserMessage(_) => "UserMessage",
                        Event::ToolResult(_) => "ToolResult",
                        _ => "Other"
                    });
                let events = self.event_store.load_events::<Event>(&query, None).await?;
                let mut thread = Thread::fold(&events);

                // Check recipient filter
                if let Some(ref filter) = self.recipient_filter {
                    tracing::debug!("ThreadProcessor checking recipient. Thread recipient: {:?}, Filter: {}",
                        thread.recipient, filter);
                    if let Some(ref thread_recipient) = thread.recipient {
                        // Check if the thread's recipient matches our filter
                        // Support prefix matching for patterns like "task-*"
                        if filter.ends_with("*") {
                            let prefix = &filter[..filter.len() - 1];
                            if !thread_recipient.starts_with(prefix) {
                                tracing::debug!("Skipping thread with recipient {} (filter: {})", thread_recipient, filter);
                                return Ok(());
                            }
                        } else if thread_recipient != filter {
                            tracing::debug!("Skipping thread with recipient {} (filter: {})", thread_recipient, filter);
                            return Ok(());
                        }
                    } else {
                        // Thread has no recipient but we have a filter - skip
                        tracing::debug!("Skipping thread with no recipient (filter: {})", filter);
                        return Ok(());
                    }
                }

                tracing::info!("ThreadProcessor recipient check passed for aggregate {}", event.aggregate_id);

                // Don't process if thread is already completed
                if thread.is_completed {
                    tracing::info!("Thread {} is completed, skipping processing", event.aggregate_id);
                    return Ok(());
                }

                tracing::debug!("Thread {} - Last message type: {:?}", event.aggregate_id,
                    thread.messages.last().map(|m| match m {
                        rig::completion::Message::User { .. } => "User",
                        rig::completion::Message::Assistant { .. } => "Assistant",
                    }));
                let completion = self.completion(&thread).await?;
                tracing::info!("ThreadProcessor generated completion for aggregate {}", event.aggregate_id);
                match thread.process(Command::Agent(completion.clone())) {
                    Ok(new_events) => {
                        tracing::info!("ThreadProcessor processed {} new events for aggregate {}", new_events.len(), event.aggregate_id);
                        for new_event in new_events.iter() {
                            self.event_store
                                .push_event(
                                    &event.stream_id,
                                    &event.aggregate_id,
                                    new_event,
                                    &Default::default(),
                                )
                                .await?;
                        }
                    }
                    Err(e) => {
                        tracing::error!("ThreadProcessor failed to process command for aggregate {}: {:?}", event.aggregate_id, e);
                        return Err(eyre::eyre!("Failed to process command: {:?}", e));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl<T: LLMClient, E: EventStore> ThreadProcessor<T, E> {
    pub fn new(llm: T, event_store: E) -> Self {
        Self {
            llm,
            event_store,
            recipient_filter: None,
        }
    }

    pub fn with_recipient_filter(mut self, filter: String) -> Self {
        self.recipient_filter = Some(filter);
        self
    }

    pub async fn completion(&self, thread: &Thread) -> Result<CompletionResponse> {
        let mut history = thread.messages.clone();
        let message = history.pop().expect("No messages");
        let mut completion = Completion::new(thread.model.clone().unwrap(), message)
            .history(history)
            .temperature(thread.temperature.unwrap())
            .max_tokens(thread.max_tokens.unwrap());
        if let Some(preamble) = &thread.preamble {
            completion = completion.preamble(preamble.clone());
        }
        if let Some(ref tools) = thread.tools {
            completion = completion.tools(tools.clone());
        }
        let llm = self.llm.clone().with_retry();
        llm.completion(completion).await
    }
}
