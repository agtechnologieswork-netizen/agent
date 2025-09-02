use tracing_subscriber;

use dabgent_mq::db::{EventStore, Metadata, Query, sqlite::SqliteStore};
use dabgent_mq::models::Event;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    test_pub_sub().await;
    Ok(())
}

async fn store() -> SqliteStore {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct BenchEvent {
    id: u64,
    payload: String,
}

impl Event for BenchEvent {
    const EVENT_VERSION: &'static str = "1.0";
    fn event_type() -> &'static str {
        "BenchEvent"
    }
}

fn small_event() -> BenchEvent {
    BenchEvent {
        id: 0,
        payload: "Small Event".to_string(),
    }
}

async fn test_pub_sub() {
    use std::sync::Arc;
    use tokio::sync::Barrier;
    const NUM_PUBLISHERS: usize = 2;
    const NUM_SUBSCRIBERS: usize = 2;
    const NUM_EVENTS: usize = 10;

    const STREAM_ID: &str = "bench";
    const AGGREGATE_ID: &str = "bench";

    let store = store().await;
    let barrier = Arc::new(Barrier::new(NUM_PUBLISHERS + NUM_SUBSCRIBERS));
    for pub_id in 0..NUM_PUBLISHERS {
        let store = store.clone();
        let barrier = barrier.clone();
        tokio::spawn(async move {
            tracing::info!("Publisher {} started", pub_id);
            let event = small_event();
            let metadata = Metadata::default();
            for _ in 0..NUM_EVENTS {
                store
                    .push_event(STREAM_ID, AGGREGATE_ID, &event, &metadata)
                    .await
                    .unwrap();
                tracing::info!("Published event for publisher {}", pub_id);
            }
            tracing::info!("Publisher {} finished", pub_id);
            barrier.wait().await;
        });
    }
    for sub_id in 0..NUM_SUBSCRIBERS {
        let store = store.clone();
        let barrier = barrier.clone();
        tokio::spawn(async move {
            let query = Query {
                stream_id: STREAM_ID.to_string(),
                event_type: None,
                aggregate_id: Some(AGGREGATE_ID.to_string()),
            };
            let mut stream = store.subscribe::<BenchEvent>(&query).unwrap();
            tracing::info!("Subscriber {} started", sub_id);
            let mut count = 0;
            while let Some(event) = stream.next().await {
                count += 1;
                tracing::info!(
                    "Subscriber {} received event #{}: {:?}",
                    sub_id,
                    count,
                    event
                );
            }
            tracing::info!("Subscriber {} finished", sub_id);
            barrier.wait().await;
        });
    }
    barrier.wait().await;
}
