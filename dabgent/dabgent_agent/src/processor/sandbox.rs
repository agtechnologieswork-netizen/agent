use crate::toolbox::{ToolCallExt, ToolDyn};
use dabgent_mq::{Aggregate, Callback, Envelope, Event as MQEvent, EventStore, Handler};
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use rig::message::{ToolCall, ToolResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    QueueTools(Vec<ToolCall>),
    Execute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    ToolsRequested(Vec<ToolCall>),
    ToolsExecuted(Vec<ToolResult>),
}

impl MQEvent for Event {
    fn event_version(&self) -> String {
        "1.0".to_owned()
    }

    fn event_type(&self) -> String {
        match self {
            Event::ToolsRequested(..) => "tools_requested",
            Event::ToolsExecuted(..) => "tools_executed",
        }
        .to_owned()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No tools requested")]
    NoToolsRequested,
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),
}

pub struct SandboxServices {
    sandbox: Mutex<Box<dyn SandboxDyn>>,
    pub tools: Vec<Box<dyn ToolDyn>>,
}

impl SandboxServices {
    pub fn new(sandbox: Box<dyn SandboxDyn>, tools: Vec<Box<dyn ToolDyn>>) -> Self {
        Self {
            sandbox: Mutex::new(sandbox),
            tools,
        }
    }

    async fn get_sandbox_fork(&self) -> Result<Box<dyn SandboxDyn>, Error> {
        self.sandbox
            .lock()
            .await
            .fork()
            .await
            .map_err(|e| Error::ToolExecutionFailed(e.to_string()))
    }

    async fn set_sandbox(&self, sandbox: Box<dyn SandboxDyn>) {
        *self.sandbox.lock().await = sandbox;
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sandbox {
    pub pending_calls: Vec<ToolCall>,
}

impl Aggregate for Sandbox {
    const TYPE: &'static str = "sandbox";
    type Command = Command;
    type Event = Event;
    type Error = Error;
    type Services = Arc<SandboxServices>;

    async fn handle(
        &self,
        cmd: Self::Command,
        services: &Self::Services,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match cmd {
            Command::QueueTools(calls) => Ok(vec![Event::ToolsRequested(calls)]),
            Command::Execute => {
                if self.pending_calls.is_empty() {
                    return Err(Error::NoToolsRequested);
                }

                let mut results = Vec::new();
                let mut sandbox = services.get_sandbox_fork().await?;

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
                services.set_sandbox(sandbox).await;

                Ok(vec![Event::ToolsExecuted(results)])
            }
        }
    }

    fn apply(&mut self, event: Self::Event) {
        match event {
            Event::ToolsRequested(calls) => {
                self.pending_calls.extend(calls);
            }
            Event::ToolsExecuted(_) => {
                self.pending_calls.clear();
            }
        }
    }
}

pub struct ExecutionCallback<ES: EventStore> {
    handler: Handler<Sandbox, ES>,
}

impl<ES: EventStore> ExecutionCallback<ES> {
    pub fn new(handler: Handler<Sandbox, ES>) -> Self {
        Self { handler }
    }
}

impl<ES: EventStore> Callback<Sandbox> for ExecutionCallback<ES> {
    async fn process(&mut self, event: &Envelope<Sandbox>) -> Result<()> {
        if matches!(event.data, Event::ToolsRequested(..)) {
            self.handler
                .execute(&event.aggregate_id, Command::Execute)
                .await?;
        }
        Ok(())
    }
}
