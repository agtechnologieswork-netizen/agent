use super::agent::{Agent, AgentState, Event, EventHandler};
use dabgent_mq::{Aggregate, Callback, Envelope, Event as MQEvent, EventStore, Handler};
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
        // Check if this is a Finished event and print a clear completion message
        if let Event::Agent(agent_event) = &event.data {
            if agent_event.event_type() == "finished" {
                eprintln!("\n========================================");
                eprintln!("✓ WORKFLOW COMPLETED SUCCESSFULLY");
                eprintln!("========================================\n");
            }
        }
        tracing::info!(agent = A::TYPE, envelope = ?event, "event");
        Ok(())
    }
}
