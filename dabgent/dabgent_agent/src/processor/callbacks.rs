use crate::metadata::{AgentExtra, AgentMetadata, WorkerContext};
use crate::processor::sandbox::{self, Sandbox};
use crate::processor::thread::{self, Thread};
use crate::processor::worker::{self, Worker};
use dabgent_mq::{Callback, Envelope, EventStore, Handler};
use eyre::Result;

// Watches Worker events and orchestrates Thread and Sandbox
pub struct WorkerOrchestrator<ES: EventStore> {
    thread_handler: Handler<Thread, ES>,
    sandbox_handler: Handler<Sandbox, ES>,
}

impl<ES: EventStore> WorkerOrchestrator<ES> {
    pub fn new(thread_handler: Handler<Thread, ES>, sandbox_handler: Handler<Sandbox, ES>) -> Self {
        Self {
            thread_handler,
            sandbox_handler,
        }
    }
}

impl<ES: EventStore> Callback<Worker> for WorkerOrchestrator<ES> {
    async fn process(&mut self, envelope: &Envelope<Worker>) -> Result<()> {
        let agent_meta: AgentMetadata = envelope.metadata.clone().try_into()?;

        match &envelope.data {
            worker::Event::Started {
                thread_id,
                sandbox_id,
                workflow_id,
            } => {
                let context = WorkerContext {
                    worker_id: envelope.aggregate_id.clone(),
                    thread_id: thread_id.clone(),
                    sandbox_id: sandbox_id.clone(),
                };

                let metadata = AgentMetadata::new()
                    .with_correlation(*workflow_id)
                    .with_worker_context(context);

                // Initialize thread
                self.thread_handler
                    .execute_with_metadata(
                        thread_id,
                        thread::Command::Setup {
                            model: "claude-3-5-sonnet-20241022".to_string(),
                            temperature: 0.7,
                            max_tokens: 4096,
                            preamble: None,
                            tools: None,
                        },
                        metadata.into(),
                    )
                    .await?;
            }

            worker::Event::ThreadMessageRequested(content) => {
                if let Some(AgentExtra::Worker(ctx)) = &agent_meta.extra {
                    self.thread_handler
                        .execute_with_metadata(
                            &ctx.thread_id,
                            thread::Command::User(content.clone()),
                            envelope.metadata.clone(),
                        )
                        .await?;
                }
            }

            worker::Event::ThreadCompletionRequested => {
                if let Some(AgentExtra::Worker(ctx)) = &agent_meta.extra {
                    self.thread_handler
                        .execute_with_metadata(
                            &ctx.thread_id,
                            thread::Command::Completion,
                            envelope.metadata.clone(),
                        )
                        .await?;
                }
            }

            worker::Event::ToolExecutionRequested { sandbox_id, calls } => {
                self.sandbox_handler
                    .execute_with_metadata(
                        sandbox_id,
                        sandbox::Command::RequestTools(calls.clone()),
                        envelope.metadata.clone(),
                    )
                    .await?;
            }

            _ => {}
        }

        Ok(())
    }
}

// Watches Thread events and notifies Worker
pub struct ThreadWatcher<ES: EventStore> {
    worker_handler: Handler<Worker, ES>,
}

impl<ES: EventStore> ThreadWatcher<ES> {
    pub fn new(worker_handler: Handler<Worker, ES>) -> Self {
        Self { worker_handler }
    }
}

impl<ES: EventStore> Callback<Thread> for ThreadWatcher<ES> {
    async fn process(&mut self, envelope: &Envelope<Thread>) -> Result<()> {
        if let thread::Event::AgentMessage(response) = &envelope.data {
            let agent_meta: AgentMetadata = envelope.metadata.clone().try_into()?;

            if let Some(AgentExtra::Worker(ctx)) = agent_meta.extra {
                self.worker_handler
                    .execute_with_metadata(
                        &ctx.worker_id,
                        worker::Command::OnThreadResponse(response.clone()),
                        envelope.metadata.clone(),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}

// Watches Sandbox events and notifies Worker
pub struct SandboxWatcher<ES: EventStore> {
    worker_handler: Handler<Worker, ES>,
}

impl<ES: EventStore> SandboxWatcher<ES> {
    pub fn new(worker_handler: Handler<Worker, ES>) -> Self {
        Self { worker_handler }
    }
}

impl<ES: EventStore> Callback<Sandbox> for SandboxWatcher<ES> {
    async fn process(&mut self, envelope: &Envelope<Sandbox>) -> Result<()> {
        if let sandbox::Event::ToolsExecuted(results) = &envelope.data {
            let agent_meta: AgentMetadata = envelope.metadata.clone().try_into()?;

            if let Some(AgentExtra::Worker(ctx)) = agent_meta.extra {
                self.worker_handler
                    .execute_with_metadata(
                        &ctx.worker_id,
                        worker::Command::OnToolsExecuted(results.clone()),
                        envelope.metadata.clone(),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}

