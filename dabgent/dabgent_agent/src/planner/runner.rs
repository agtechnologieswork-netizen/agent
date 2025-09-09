//! Minimal planner runner

use crate::agent::Worker;
use crate::handler::Handler;
use crate::llm::LLMClient;
use crate::planner::{Planner, Command, Event};
use crate::toolbox::ToolDyn;
use dabgent_mq::EventStore;
use dabgent_mq::db::Query;
use eyre::Result;
use tokio::time::{timeout, Duration};
use tokio_stream::StreamExt;
use uuid::Uuid;

/// Run planning with default 5 minute timeout
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
    run_with_timeout(llm, store, preamble, tools, input, 300).await
}

/// Run planning with custom timeout in seconds
pub async fn run_with_timeout<T, E>(
    llm: T,
    store: E,
    preamble: String,
    tools: Vec<Box<dyn ToolDyn>>,
    input: String,
    timeout_secs: u64,
) -> Result<()>
where
    T: LLMClient + Clone + Send + 'static,
    E: EventStore + Clone + Send + 'static,
{
    let fut = async {
        let id = Uuid::new_v4().to_string();
        
        // Initialize planner
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
        
        // Wait for completion
        let mut rx = store.subscribe::<Event>(&Query {
            stream_id: "plan".to_owned(),
            event_type: Some("PlanningCompleted".to_owned()),
            aggregate_id: Some(id),
        })?;
        
        if let Some(Ok(Event::PlanningCompleted { summary })) = rx.next().await {
            tracing::info!("Done: {}", summary);
        }
        
        Ok::<(), eyre::Error>(())
    };
    
    timeout(Duration::from_secs(timeout_secs), fut)
        .await
        .map_err(|_| eyre::eyre!("Timeout after {} seconds", timeout_secs))?
}