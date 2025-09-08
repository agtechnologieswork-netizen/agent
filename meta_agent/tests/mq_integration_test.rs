#![cfg(feature = "mq")]

use meta_agent::planner::{handler::{Handler, Command, Event}, Planner};
use dabgent_mq::db::{sqlite::SqliteStore, EventStore, Query, Metadata};

#[tokio::test]
async fn test_planner_events_persist_and_replay() {
    // in-memory sqlite pool
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite::memory:")
        .await
        .expect("pool");

    let store = SqliteStore::new(pool);
    store.migrate().await;

    // run planner
    let mut planner = Planner::new();
    let events = planner.process(Command::Initialize {
        user_input: "Task A\nTask B".to_string(),
        attachments: vec![],
    }).expect("planner init");

    // persist emitted events
    let aggregate_id = "session-1";
    for ev in &events {
        store.push_event("planner", aggregate_id, ev, &Metadata::default()).await.expect("push");
    }

    // load and fold to reconstruct state
    let query = Query { stream_id: "planner".into(), event_type: None, aggregate_id: Some(aggregate_id.into()) };
    let loaded: Vec<Event> = store.load_events(&query, None).await.expect("load");
    let restored = Planner::fold(&loaded);

    // basic assertions
    assert_eq!(restored.state().tasks.len(), planner.state().tasks.len());
    assert_eq!(restored.events().len(), planner.events().len());
}


