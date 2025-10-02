use super::agent::{Agent, AgentState, EventHandler};
use dabgent_mq::{Aggregate, Callback, Envelope, EventStore, Handler};
use eyre::Result;

pub struct LogHandler;

impl<T: Aggregate + std::fmt::Debug> Callback<T> for LogHandler {
    async fn process(&mut self, envelope: &Envelope<T>) -> Result<()> {
        tracing::info!(aggregate = T::TYPE, envelope = ?envelope, "event");
        Ok(())
    }
}

impl<A: Agent, ES: EventStore> EventHandler<A, ES> for LogHandler
where
    AgentState<A>: std::fmt::Debug,
{
    async fn process(
        &mut self,
        _handler: &Handler<AgentState<A>, ES>,
        event: &Envelope<AgentState<A>>,
    ) -> Result<()> {
        tracing::info!(agent = A::TYPE, envelope = ?event, "event");
        Ok(())
    }
}
