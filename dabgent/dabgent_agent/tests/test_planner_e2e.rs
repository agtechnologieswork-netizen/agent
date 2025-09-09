//! End-to-end test for planner integration

use dabgent_agent::handler::Handler;
use dabgent_agent::planner::{Planner, Command, Event};
use dabgent_mq::db::{sqlite::SqliteStore, Query};
use dabgent_mq::EventStore;

async fn setup_store() -> SqliteStore {
    let pool = sqlx::SqlitePool::connect(":memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}

#[tokio::test]
async fn test_planner_basic_flow() {
    let store = setup_store().await;
    let mut planner = Planner::new();
    
    // Initialize with a simple task
    let command = Command::Initialize {
        user_input: "Write a function to add two numbers".to_string(),
    };
    
    // Process should succeed
    let events = planner.process(command);
    assert!(events.is_ok(), "Should process initialize command");
    
    let events = events.unwrap();
    assert!(!events.is_empty(), "Should generate events");
    
    // Persist events
    for event in &events {
        store.push_event("test", "plan-1", event, &Default::default())
            .await
            .unwrap();
    }
    
    // Load events back
    let query = Query {
        stream_id: "test".to_owned(),
        event_type: None,
        aggregate_id: Some("plan-1".to_owned()),
    };
    
    let loaded = store.load_events::<Event>(&query, None).await.unwrap();
    assert_eq!(loaded.len(), events.len(), "All events should be persisted");
    
    // Reconstruct from events
    let _ = Planner::fold(&loaded);
}

#[tokio::test]
async fn test_planner_continue() {
    let mut planner = Planner::new();
    
    // Initialize
    let _ = planner.process(Command::Initialize {
        user_input: "Build a web application".to_string(),
    }).unwrap();
    
    // Continue command should work
    let result = planner.process(Command::Continue);
    assert!(result.is_ok(), "Continue should work");
}

// Attachment-related test removed as attachments are out of MVP scope