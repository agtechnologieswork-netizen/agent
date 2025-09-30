use crate::toolbox::{ToolCallExt, ToolDyn};
use dabgent_mq::{Aggregate, Callback, Envelope, Event as MQEvent, EventStore, Metadata};
use dabgent_sandbox::{DaggerSandbox, SandboxHandle};
use eyre::Result;
use rig::message::{ToolCall, ToolResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    CreateFromDirectory {
        host_dir: String,
        dockerfile: String,
    },
    QueueTools(Vec<ToolCall>),
    Execute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    SandboxCreated,
    ToolsRequested(Vec<ToolCall>),
    ToolsExecuted(Vec<ToolResult>),
}

impl MQEvent for Event {
    fn event_version(&self) -> String {
        "1.0".to_owned()
    }

    fn event_type(&self) -> String {
        match self {
            Event::SandboxCreated => "sandbox_created",
            Event::ToolsRequested(..) => "tools_requested",
            Event::ToolsExecuted(..) => "tools_executed",
        }
        .to_owned()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Sandbox not initialized")]
    NotInitialized,
    #[error("No tools requested")]
    NoToolsRequested,
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),
    #[error("Internal: {0}")]
    InternalError(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sandbox {
    pub initialized: bool,
    pub pending_calls: Vec<ToolCall>,
}

impl Aggregate for Sandbox {
    const TYPE: &'static str = "sandbox";
    type Command = Command;
    type Event = Event;
    type Error = Error;
    type Services = SandboxServicesWithId;

    async fn handle(
        &self,
        cmd: Self::Command,
        services: &Self::Services,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match cmd {
            Command::CreateFromDirectory {
                host_dir,
                dockerfile,
            } => {
                services.create_sandbox(&host_dir, &dockerfile).await?;
                Ok(vec![Event::SandboxCreated])
            }
            Command::QueueTools(calls) => {
                if !self.initialized {
                    return Err(Error::NotInitialized);
                }
                Ok(vec![Event::ToolsRequested(calls)])
            }
            Command::Execute => {
                if !self.initialized {
                    return Err(Error::NotInitialized);
                }
                if self.pending_calls.is_empty() {
                    return Err(Error::NoToolsRequested);
                }

                let mut results = Vec::new();
                let mut sandbox = services.get_sandbox().await?;
                for call in &self.pending_calls {
                    let tool = services
                        .tools
                        .iter()
                        .find(|t| t.name() == call.function.name)
                        .ok_or_else(|| Error::ToolNotFound(call.function.name.clone()))?;
                    let output = tool
                        .call(call.function.arguments.clone(), &mut sandbox)
                        .await
                        .map_err(|e| Error::ToolExecutionFailed(e.to_string()))?;
                    results.push(call.to_result(output));
                }
                services.set_sandbox(sandbox).await?;

                Ok(vec![Event::ToolsExecuted(results)])
            }
        }
    }

    fn apply(&mut self, event: Self::Event) {
        match event {
            Event::SandboxCreated => {
                self.initialized = true;
            }
            Event::ToolsRequested(calls) => {
                self.pending_calls.extend(calls);
            }
            Event::ToolsExecuted(_) => {
                self.pending_calls.clear();
            }
        }
    }
}

#[derive(Clone)]
pub struct SandboxHandler<ES: EventStore> {
    store: ES,
    services: SandboxServices,
}

impl<ES: EventStore> SandboxHandler<ES> {
    pub fn new(store: ES, services: SandboxServices) -> Self {
        Self { store, services }
    }

    pub async fn execute(&self, aggregate_id: &str, cmd: Command) -> eyre::Result<()> {
        self.execute_with_metadata(aggregate_id, cmd, Default::default())
            .await
    }

    pub async fn execute_with_metadata(
        &self,
        aggregate_id: &str,
        cmd: Command,
        metadata: Metadata,
    ) -> eyre::Result<()> {
        let ctx = self.store.load_aggregate::<Sandbox>(aggregate_id).await?;
        let services = self.services.with_id(aggregate_id.to_string());
        let events = ctx.aggregate.handle(cmd, &services).await?;
        self.store.commit(events, metadata, ctx).await?;
        Ok(())
    }
}

pub struct ExecutionCallback<ES: EventStore> {
    handler: SandboxHandler<ES>,
}

impl<ES: EventStore> ExecutionCallback<ES> {
    pub fn new(handler: SandboxHandler<ES>) -> Self {
        Self { handler }
    }
}

impl<ES: EventStore> Callback<Sandbox> for ExecutionCallback<ES> {
    async fn process(&mut self, envelope: &Envelope<Sandbox>) -> Result<()> {
        if matches!(envelope.data, Event::ToolsRequested(..)) {
            self.handler
                .execute_with_metadata(
                    &envelope.aggregate_id,
                    Command::Execute,
                    envelope.metadata.clone(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct SandboxServices {
    handle: SandboxHandle,
    tools: Arc<Vec<Box<dyn ToolDyn>>>,
}

impl SandboxServices {
    pub fn new(handle: SandboxHandle, tools: Vec<Box<dyn ToolDyn>>) -> Self {
        Self {
            handle,
            tools: Arc::new(tools),
        }
    }

    fn with_id(&self, id: String) -> SandboxServicesWithId {
        SandboxServicesWithId {
            id,
            handle: self.handle.clone(),
            tools: self.tools.clone(),
        }
    }
}

pub struct SandboxServicesWithId {
    id: String,
    handle: SandboxHandle,
    tools: Arc<Vec<Box<dyn ToolDyn>>>,
}

impl SandboxServicesWithId {
    async fn create_sandbox(&self, host_dir: &str, dockerfile: &str) -> Result<(), Error> {
        self.handle
            .create_from_directory(&self.id, host_dir, dockerfile)
            .await
            .map_err(|err| Error::InternalError(err.to_string()))
    }

    async fn get_sandbox(&self) -> Result<DaggerSandbox, Error> {
        self.handle
            .get(&self.id)
            .await
            .map_err(|err| Error::InternalError(err.to_string()))?
            .ok_or(Error::NotInitialized)
    }

    async fn set_sandbox(&self, sandbox: DaggerSandbox) -> Result<(), Error> {
        self.handle
            .set(&self.id, sandbox)
            .await
            .map_err(|err| Error::InternalError(err.to_string()))
    }
}
