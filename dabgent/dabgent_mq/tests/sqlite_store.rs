use dabgent_mq::db::{sqlite::SqliteStore, *};
use dabgent_mq::models::Event;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestEvent(usize);

impl Event for TestEvent {
    const EVENT_VERSION: &'static str = "1.0";
    fn event_type() -> &'static str {
        "TestEvent"
    }
}

async fn setup_test_store() -> SqliteStore {
    let pool = SqlitePool::connect(":memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}

#[tokio::test]
async fn test_push_and_load_events() {
    let store = setup_test_store().await;
    let stream_id = "test-stream";
    let aggregate_id = "test-aggregate";

    let event1 = TestEvent(0);
    let event2 = TestEvent(1);
    let metadata = Metadata::default();
    store
        .push_event(stream_id, aggregate_id, &event1, &metadata)
        .await
        .expect("Failed to push first event");
    store
        .push_event(stream_id, aggregate_id, &event2, &metadata)
        .await
        .expect("Failed to push second event");

    let query = Query {
        stream_id: stream_id.to_string(),
        event_type: None,
        aggregate_id: None,
    };

    let loaded_events: Vec<TestEvent> = store
        .load_events(&query)
        .await
        .expect("Failed to load events");

    assert_eq!(loaded_events.len(), 2);
    assert_eq!(loaded_events[0], event1);
    assert_eq!(loaded_events[1], event2);
}

#[tokio::test]
async fn test_load_events_by_aggregate_id() {
    let store = setup_test_store().await;
    let stream_id = "test-stream";

    let event1 = TestEvent(0);
    let event2 = TestEvent(1);
    let metadata = Metadata::default();

    store
        .push_event(stream_id, "aggregate1", &event1, &metadata)
        .await
        .expect("Failed to push first event");

    store
        .push_event(stream_id, "aggregate2", &event2, &metadata)
        .await
        .expect("Failed to push second event");

    let query = Query {
        stream_id: stream_id.to_string(),
        event_type: None,
        aggregate_id: Some("aggregate1".to_string()),
    };

    let loaded_events: Vec<TestEvent> = store
        .load_events(&query)
        .await
        .expect("Failed to load events");

    assert_eq!(loaded_events.len(), 1);
    assert_eq!(loaded_events[0], event1);
}

#[tokio::test]
async fn test_subscription() {
    let store = setup_test_store().await;
    let stream_id = "test-stream";
    let aggregate_id = "test-aggregate";

    let query = Query {
        stream_id: stream_id.to_string(),
        event_type: None,
        aggregate_id: None,
    };

    let mut receiver = store
        .subscribe::<TestEvent>(&query)
        .expect("Failed to subscribe");

    let event1 = TestEvent(0);
    let metadata = Metadata::default();
    store
        .push_event(&stream_id, &aggregate_id, &event1, &metadata)
        .await
        .expect("Failed to push event");

    let received = tokio::time::timeout(tokio::time::Duration::from_secs(2), receiver.recv())
        .await
        .expect("Timeout waiting for event")
        .expect("Failed to receive event");

    assert_eq!(received, event1);
}

#[tokio::test]
async fn test_multiple_subscribers() {
    let store = setup_test_store().await;
    let stream_id = "test-stream";
    let aggregate_id = "test-aggregate";

    let query = Query {
        stream_id: stream_id.to_string(),
        event_type: None,
        aggregate_id: None,
    };

    // Create two subscribers
    let mut receiver1 = store
        .subscribe::<TestEvent>(&query)
        .expect("Failed to subscribe");

    let mut receiver2 = store
        .subscribe::<TestEvent>(&query)
        .expect("Failed to subscribe");

    let event = TestEvent(0);
    let metadata = Metadata::default();
    store
        .push_event(&stream_id, &aggregate_id, &event, &metadata)
        .await
        .expect("Failed to push event");

    // Both receivers should get the event
    let received1 = tokio::time::timeout(tokio::time::Duration::from_secs(2), receiver1.recv())
        .await
        .expect("Timeout waiting for event on receiver1")
        .expect("Failed to receive event on receiver1");

    let received2 = tokio::time::timeout(tokio::time::Duration::from_secs(2), receiver2.recv())
        .await
        .expect("Timeout waiting for event on receiver2")
        .expect("Failed to receive event on receiver2");

    assert_eq!(received1, event);
    assert_eq!(received2, event);
}
