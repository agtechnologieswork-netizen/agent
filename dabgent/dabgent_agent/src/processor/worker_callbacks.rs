use crate::metadata::{AgentExtra, AgentMetadata};
use crate::processor::sandbox::{self, Sandbox, SandboxHandler};
use crate::processor::thread::{self, Thread};
use crate::processor::worker::{self, Worker};
use dabgent_mq::{Callback, Envelope, EventStore, Handler};
use eyre::Result;

#[derive(Clone)]
pub struct WorkerOrchestrator<ES: EventStore> {
    thread_handler: Handler<Thread, ES>,
    sandbox_handler: SandboxHandler<ES>,
}

impl<ES: EventStore> WorkerOrchestrator<ES> {
    pub fn new(thread_handler: Handler<Thread, ES>, sandbox_handler: SandboxHandler<ES>) -> Self {
        Self {
            thread_handler,
            sandbox_handler,
        }
    }
}

impl<ES: EventStore> Callback<Worker> for WorkerOrchestrator<ES> {
    async fn process(&mut self, envelope: &Envelope<Worker>) -> Result<()> {
        let meta = AgentMetadata::try_from(&envelope.metadata)?;
        let meta = meta.with_extra(AgentExtra::new_worker(envelope.aggregate_id.clone()));
        match &envelope.data {
            worker::Event::CompletionRequested { thread_id, content } => {
                self.thread_handler
                    .execute_with_metadata(
                        &thread_id,
                        thread::Command::User(content.clone()),
                        meta.clone().into(),
                    )
                    .await?;
            }
            worker::Event::ToolExecutionRequested { sandbox_id, calls } => {
                self.sandbox_handler
                    .execute_with_metadata(
                        sandbox_id,
                        sandbox::Command::QueueToolsFiltered(calls.clone()),
                        meta.clone().into(),
                    )
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }
}

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
            let meta = AgentMetadata::try_from(&envelope.metadata)?;
            if let Some(AgentExtra::Worker { aggregate_id }) = meta.extra {
                self.worker_handler
                    .execute_with_metadata(
                        &aggregate_id,
                        worker::Command::OnThreadResponse(response.clone()),
                        envelope.metadata.clone(),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}

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
            let meta = AgentMetadata::try_from(&envelope.metadata)?;
            if let Some(AgentExtra::Worker { aggregate_id }) = meta.extra {
                self.worker_handler
                    .execute_with_metadata(
                        &aggregate_id,
                        worker::Command::OnToolsExecuted(results.clone()),
                        envelope.metadata.clone(),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}
