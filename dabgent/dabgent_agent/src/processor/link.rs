use super::agent::{Agent, AgentState, Command, Event, EventHandler, HandlerAdapter, Runtime};
use dabgent_mq::{Envelope, EventQueue, EventStore, Handler};
use eyre::Result;

/// Link trait for bidirectional communication between two agents.
pub trait Link<ES: EventStore>: Send + Sync {
    type RuntimeA: Agent;
    type RuntimeB: Agent;

    fn forward(
        &self,
        a_id: &str,
        event: &Event<<Self::RuntimeA as Agent>::AgentEvent>,
        handler: &Handler<AgentState<Self::RuntimeA>, ES>,
    ) -> impl Future<Output = Option<(String, Command<<Self::RuntimeB as Agent>::AgentCommand>)>> + Send;

    fn backward(
        &self,
        b_id: &str,
        event: &Event<<Self::RuntimeB as Agent>::AgentEvent>,
        handler: &Handler<AgentState<Self::RuntimeB>, ES>,
    ) -> impl Future<Output = Option<(String, Command<<Self::RuntimeA as Agent>::AgentCommand>)>> + Send;
}

struct ForwardLinkHandler<ES, L>
where
    ES: EventStore,
    L: Link<ES>,
{
    handler_b: Handler<AgentState<L::RuntimeB>, ES>,
    link: L,
}

impl<ES, L> EventHandler<L::RuntimeA, ES> for ForwardLinkHandler<ES, L>
where
    ES: EventStore,
    L: Link<ES>,
{
    async fn process(
        &mut self,
        handler: &Handler<AgentState<L::RuntimeA>, ES>,
        event: &Envelope<AgentState<L::RuntimeA>>,
    ) -> Result<()> {
        if let Some((aggregate_id, command)) = self
            .link
            .forward(&event.aggregate_id, &event.data, handler)
            .await
        {
            self.handler_b
                .execute_with_metadata(&aggregate_id, command, event.metadata.clone())
                .await?;
        }
        Ok(())
    }
}

struct BackwardLinkHandler<ES, L>
where
    ES: EventStore,
    L: Link<ES>,
{
    handler_a: Handler<AgentState<L::RuntimeA>, ES>,
    link: L,
}

impl<ES, L> EventHandler<L::RuntimeB, ES> for BackwardLinkHandler<ES, L>
where
    ES: EventStore,
    L: Link<ES>,
{
    async fn process(
        &mut self,
        handler: &Handler<AgentState<L::RuntimeB>, ES>,
        event: &Envelope<AgentState<L::RuntimeB>>,
    ) -> Result<()> {
        if let Some((aggregate_id, command)) = self
            .link
            .backward(&event.aggregate_id, &event.data, handler)
            .await
        {
            self.handler_a
                .execute_with_metadata(&aggregate_id, command, event.metadata.clone())
                .await?;
        }
        Ok(())
    }
}

pub fn link_runtimes<ES, L>(
    runtime_a: &mut Runtime<L::RuntimeA, ES>,
    runtime_b: &mut Runtime<L::RuntimeB, ES>,
    link: L,
) where
    ES: EventQueue + 'static,
    L: Link<ES> + Clone + 'static,
    <L::RuntimeA as Agent>::Services: Clone + 'static,
    <L::RuntimeB as Agent>::Services: Clone + 'static,
{
    let forward_handler = ForwardLinkHandler {
        handler_b: runtime_b.handler.clone(),
        link: link.clone(),
    };

    let backward_handler = BackwardLinkHandler {
        handler_a: runtime_a.handler.clone(),
        link,
    };

    runtime_a.listener.register(HandlerAdapter::new(
        runtime_a.handler.clone(),
        forward_handler,
    ));
    runtime_b.listener.register(HandlerAdapter::new(
        runtime_b.handler.clone(),
        backward_handler,
    ));
}
