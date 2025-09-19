use super::{Handler, Processor};
use crate::event::Event;
use crate::llm::{Completion, CompletionResponse, LLMClient};
use crate::thread::{Command, State, Thread};
use dabgent_mq::{EventDb, EventStore, Query};
use eyre::Result;
use rig::completion::ToolDefinition;

pub struct ThreadProcessor<T: LLMClient, E: EventStore> {
    llm: T,
    event_store: E,
    model: String,
    preamble: String,
    temperature: f64,
    max_tokens: u64,
    tools: Vec<ToolDefinition>,
}

impl<T: LLMClient, E: EventStore> Processor<Event> for ThreadProcessor<T, E> {
    async fn run(&mut self, event: &EventDb<Event>) -> eyre::Result<()> {
        let query = Query::stream(&event.stream_id).aggregate(&event.aggregate_id);
        match &event.data {
            Event::Prompted(..) | Event::ToolCompleted(..) => {
                let events = self.event_store.load_events::<Event>(&query, None).await?;
                let mut thread = Thread::fold(&events);
                if matches!(thread.state, State::Done) {
                    return Ok(());
                }
                let completion = self.completion(&thread).await?;
                let new_events = thread.process(Command::Completion(completion))?;
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
            _ => {}
        }
        Ok(())
    }
}

impl<T: LLMClient, E: EventStore> ThreadProcessor<T, E> {
    pub fn new(
        llm: T,
        event_store: E,
        model: String,
        preamble: String,
        tools: Vec<ToolDefinition>,
    ) -> Self {
        Self {
            llm,
            event_store,
            model,
            preamble,
            temperature: 1.0,
            max_tokens: 8192,
            tools,
        }
    }

    pub async fn completion(&self, thread: &Thread) -> Result<CompletionResponse> {
        let mut history = thread.messages.clone();
        let message = history.pop().expect("No messages");
        let completion = Completion::new(self.model.clone(), message)
            .history(history)
            .preamble(self.preamble.clone())
            .tools(self.tools.clone())
            .temperature(self.temperature)
            .max_tokens(self.max_tokens);
        self.llm.completion(completion).await
    }

    pub fn temperature(mut self, temperature: f64) -> Self {
        self.temperature = temperature;
        self
    }

    pub fn max_tokens(mut self, max_tokens: u64) -> Self {
        self.max_tokens = max_tokens;
        self
    }
}
