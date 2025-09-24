use crate::event::Event;
use crate::llm::LLMClient;
use crate::processor::{
    Aggregate, Pipeline as ProcessorPipeline, Processor, ThreadProcessor, ToolProcessor,
    thread::{self},
};
use crate::toolbox::ToolDyn;
use dabgent_mq::{
    EventStore,
    db::{Metadata, Query},
};
use dabgent_sandbox::SandboxDyn;
use eyre::{OptionExt, Result};
use std::marker::PhantomData;

const DEFAULT_TEMPERATURE: f64 = 0.0;
const DEFAULT_MAX_TOKENS: u64 = 4_096;

pub struct PipelineBuilder<T, S>
where
    T: LLMClient + 'static,
    S: EventStore,
{
    llm: Option<T>,
    store: Option<S>,
    sandbox: Option<Box<dyn SandboxDyn>>,
    model: Option<String>,
    preamble: Option<String>,
    temperature: Option<f64>,
    max_tokens: Option<u64>,
    recipient: Option<String>,
    tools: Vec<Box<dyn ToolDyn>>,
}

impl<T, S> PipelineBuilder<T, S>
where
    T: LLMClient + 'static,
    S: EventStore,
{
    pub fn new() -> Self {
        Self {
            llm: None,
            store: None,
            sandbox: None,
            model: None,
            preamble: None,
            temperature: None,
            max_tokens: None,
            recipient: None,
            tools: Vec::new(),
        }
    }

    pub fn llm(mut self, llm: T) -> Self {
        self.llm = Some(llm);
        self
    }

    pub fn store(mut self, store: S) -> Self {
        self.store = Some(store);
        self
    }

    pub fn sandbox(mut self, sandbox: Box<dyn SandboxDyn>) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    pub fn model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    pub fn preamble(mut self, preamble: String) -> Self {
        self.preamble = Some(preamble);
        self
    }

    pub fn temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn max_tokens(mut self, max_tokens: u64) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn recipient(mut self, recipient: String) -> Self {
        self.recipient = Some(recipient);
        self
    }

    pub fn tool(mut self, tool: Box<dyn ToolDyn>) -> Self {
        self.tools.push(tool);
        self
    }

    pub fn tools(mut self, tools: Vec<Box<dyn ToolDyn>>) -> Self {
        self.tools.extend(tools);
        self
    }

    pub fn build(self) -> Result<Pipeline<T, S>> {
        let llm = self.llm.ok_or_eyre("LLM Client not provided")?;
        let store = self.store.ok_or_eyre("Event Store not provided")?;
        let sandbox = self.sandbox.ok_or_eyre("Sandbox not provided")?;
        let model = self.model.ok_or_eyre("Model not provided")?;
        let temperature = self.temperature.unwrap_or(DEFAULT_TEMPERATURE);
        let max_tokens = self.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
        let preamble = self.preamble;
        let recipient = self.recipient;

        let tool_definitions = if self.tools.is_empty() {
            Vec::new()
        } else {
            self.tools.iter().map(|tool| tool.definition()).collect()
        };

        let thread_processor = ThreadProcessor::new(llm, store.clone());
        let tool_processor =
            ToolProcessor::new(sandbox, store.clone(), self.tools, recipient.clone());
        let processors = vec![thread_processor.boxed(), tool_processor.boxed()];
        let pipeline = ProcessorPipeline::new(store.clone(), processors);

        Ok(Pipeline {
            store,
            pipeline,
            config: ThreadConfig {
                model,
                preamble,
                temperature,
                max_tokens,
                tools: tool_definitions,
                recipient,
            },
            _marker: PhantomData,
        })
    }
}

struct ThreadConfig {
    model: String,
    preamble: Option<String>,
    temperature: f64,
    max_tokens: u64,
    tools: Vec<rig::completion::ToolDefinition>,
    recipient: Option<String>,
}

pub struct Pipeline<T, S>
where
    T: LLMClient + 'static,
    S: EventStore,
{
    store: S,
    pipeline: ProcessorPipeline<S, Event>,
    config: ThreadConfig,
    _marker: PhantomData<T>,
}

impl<T, S> Pipeline<T, S>
where
    T: LLMClient + 'static,
    S: EventStore,
{
    pub async fn run(self, stream_id: String, aggregate_id: String) -> Result<()> {
        self.initialize_thread(&stream_id, &aggregate_id).await?;
        let pipeline = self.pipeline;
        pipeline.run(stream_id).await
    }

    async fn initialize_thread(&self, stream_id: &str, aggregate_id: &str) -> Result<()> {
        let query = Query::stream(stream_id).aggregate(aggregate_id);
        let events = self.store.load_events::<Event>(&query, None).await?;
        if events
            .iter()
            .any(|event| matches!(event, Event::LLMConfig { .. }))
        {
            return Ok(());
        }

        let mut thread = thread::Thread::default();
        let events = thread.process(thread::Command::Setup {
            model: self.config.model.clone(),
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens,
            preamble: self.config.preamble.clone(),
            tools: if self.config.tools.is_empty() {
                None
            } else {
                Some(self.config.tools.clone())
            },
            recipient: self.config.recipient.clone(),
        })?;
        persist_events(&self.store, stream_id, aggregate_id, events).await
    }
}

async fn persist_events<S: EventStore>(
    store: &S,
    stream_id: &str,
    aggregate_id: &str,
    events: Vec<Event>,
) -> Result<()> {
    let metadata = Metadata::default();
    for event in events.iter() {
        store
            .push_event(stream_id, aggregate_id, event, &metadata)
            .await?;
    }
    Ok(())
}
