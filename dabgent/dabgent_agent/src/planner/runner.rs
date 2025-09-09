//! Minimal planner runner following dabgent patterns

use crate::agent::Worker;
use crate::handler::Handler;
use crate::llm::LLMClient;
use crate::planner::{Planner, Command, Event};
use crate::thread::Event as ThreadEvent;
use crate::toolbox::ToolDyn;
use dabgent_mq::EventStore;
use dabgent_mq::db::Query;
use eyre::Result;
use tokio_stream::StreamExt;
use uuid::Uuid;

/// Run planning and execution in dabgent style
pub async fn run<T, E>(
    llm: T,
    store: E,
    preamble: String,
    tools: Vec<Box<dyn ToolDyn>>,
    input: String,
) -> Result<()>
where
    T: LLMClient + Clone + Send + 'static,
    E: EventStore + Clone + Send + 'static,
{
    let id = Uuid::new_v4().to_string();
    
    // Initialize
    let events = Planner::new().process(Command::Initialize {
        user_input: input,
        attachments: vec![],
    })?;
    
    for event in events {
        store.push_event("plan", &id, &event, &Default::default()).await?;
    }
    
    // Spawn worker
    let worker_store = store.clone();
    let worker_id = id.clone();
    tokio::spawn(async move {
        Worker::new(llm, worker_store, preamble, tools)
            .run("plan", &worker_id)
            .await
    });
    
    // Monitor
    let mut rx = store.subscribe::<Event>(&Query {
        stream_id: "plan".to_owned(),
        event_type: Some("PlanningCompleted".to_owned()),
        aggregate_id: Some(id),
    })?;
    
    if let Some(Ok(Event::PlanningCompleted { summary })) = rx.next().await {
        tracing::info!("Done: {}", summary);
    }
    
    Ok(())
}
