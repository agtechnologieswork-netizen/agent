#![cfg(feature = "mq")]

use dabgent_mq::db::{sqlite::SqliteStore, EventStore, Query, Metadata};
use meta_agent::planner::{handler::{Handler, Command, Event}, Planner};

#[tokio::test]
async fn test_subscription_receives_events() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;

    let aggregate_id = "session-sub-1";
    let query = Query {
        stream_id: "planner".to_string(),
        event_type: Some("PlannerEvent".to_string()),
        aggregate_id: Some(aggregate_id.to_string()),
    };

    // Subscribe before pushing events
    let mut stream = store.subscribe::<Event>(&query).expect("subscribe");

    // Emit and persist one event
    let mut planner = Planner::new();
    let events = planner.process(Command::Initialize { user_input: "Step 1".into(), attachments: vec![] }).unwrap();
    for ev in &events {
        store.push_event("planner", aggregate_id, ev, &Metadata::default()).await.unwrap();
    }

    // Ensure we can receive at least one event via subscription (with timeout)
    let received = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        stream.next().await
    })
    .await
    .expect("timeout waiting for event")
    .expect("stream closed")
    .expect("deserialize event");

    // Confirm type matches one of our events
    match received {
        Event::TasksPlanned { .. } | Event::TaskDispatched { .. } | Event::TaskStatusUpdated { .. }
        | Event::ClarificationRequested { .. } | Event::ClarificationReceived { .. }
        | Event::ContextCompacted { .. } | Event::PlanningCompleted { .. } => {}
    }
}


