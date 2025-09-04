//! DabGent MQ integration examples demonstrating production patterns
//! This module shows how to use the planner with DabGent MQ for:
//! - Event persistence with metadata
//! - Real-time event streaming
//! - Correlation/causation tracking
//! - Event replay and debugging

#![cfg(feature = "mq")]

use crate::planner::{handler::{Command, Event, Handler}, Planner};
use dabgent_mq::db::{EventStore, Query, Metadata, sqlite::SqliteStore};
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Production-ready planner session with DabGent MQ
pub struct PlannerSession {
    planner: Arc<Mutex<Planner>>,
    store: SqliteStore,
    session_id: String,
    correlation_id: Uuid,
}

impl PlannerSession {
    /// Create a new planner session with SQLite event store
    pub async fn new(db_path: &str, session_id: String) -> eyre::Result<Self> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect(db_path)
            .await?;
        
        let store = SqliteStore::new(pool);
        store.migrate().await;
        
        Ok(Self {
            planner: Arc::new(Mutex::new(Planner::new())),
            store,
            session_id,
            correlation_id: Uuid::new_v4(),
        })
    }
    
    /// Process a command with full event persistence and metadata
    pub async fn process_command(&self, command: Command) -> eyre::Result<Vec<Event>> {
        let causation_id = Uuid::new_v4();
        
        // Process command
        let mut planner = self.planner.lock().await;
        let events = planner.process(command)?;
        drop(planner); // Release lock early
        
        // Persist events with rich metadata
        for event in &events {
            let metadata = Metadata {
                correlation_id: Some(self.correlation_id),
                causation_id: Some(causation_id),
                extra: Some(serde_json::json!({
                    "variant": event.variant_type(),
                    "timestamp": chrono::Utc::now(),
                })),
            };
            
            self.store.push_event(
                "planner",
                &self.session_id,
                event,
                &metadata
            ).await?;
        }
        
        Ok(events)
    }
    
    /// Restore planner state from event history
    pub async fn restore_from_history(&self) -> eyre::Result<Planner> {
        let query = Query {
            stream_id: "planner".to_string(),
            aggregate_id: Some(self.session_id.clone()),
            event_type: None,
        };
        
        let events: Vec<Event> = self.store.load_events(&query, None).await?;
        Ok(Planner::fold(&events))
    }
    
    /// Time-travel to specific sequence number
    pub async fn restore_at_sequence(&self, sequence: i64) -> eyre::Result<Planner> {
        let query = Query {
            stream_id: "planner".to_string(),
            aggregate_id: Some(self.session_id.clone()),
            event_type: None,
        };
        
        let events: Vec<Event> = self.store.load_events(&query, Some(sequence)).await?;
        Ok(Planner::fold(&events))
    }
}

/// Executor subscription pattern for reactive processing
pub struct ExecutorSubscription {
    store: SqliteStore,
}

impl ExecutorSubscription {
    pub fn new(store: SqliteStore) -> Self {
        Self { store }
    }
    
    /// Subscribe to task dispatch events and route to executors
    pub async fn start_task_router(&self) -> eyre::Result<()> {
        let query = Query {
            stream_id: "planner".to_string(),
            event_type: Some("PlannerEvent".to_string()), // Base type
            aggregate_id: None, // All sessions
        };
        
        let mut stream = self.store.subscribe::<Event>(&query)?;
        
        while let Some(event_result) = stream.next().await {
            let event = event_result?;
            
            // Route based on event variant
            match event {
                Event::TaskDispatched { task_id, command } => {
                    self.route_to_executor(task_id, command).await?;
                }
                Event::ClarificationRequested { task_id, question } => {
                    self.route_to_ui(task_id, question).await?;
                }
                _ => {
                    // Other events can trigger monitoring, metrics, etc.
                }
            }
        }
        
        Ok(())
    }
    
    async fn route_to_executor(&self, _task_id: u64, command: crate::planner::types::PlannerCmd) -> eyre::Result<()> {
        use crate::planner::types::{NodeKind, PlannerCmd};
        
        // Route based on NodeKind for specialized executors
        match command {
            PlannerCmd::ExecuteTask { node_id, kind, parameters } => {
                match kind {
                    NodeKind::ToolCall => {
                        // Route to ToolCallExecutor
                        println!("→ ToolCallExecutor: task {} with {}", node_id, parameters);
                    }
                    NodeKind::Processing => {
                        // Route to ProcessingExecutor  
                        println!("→ ProcessingExecutor: task {} with {}", node_id, parameters);
                    }
                    NodeKind::Clarification => {
                        // Route to ClarificationExecutor
                        println!("→ ClarificationExecutor: task {} with {}", node_id, parameters);
                    }
                }
            }
            _ => {}
        }
        
        Ok(())
    }
    
    async fn route_to_ui(&self, task_id: u64, question: String) -> eyre::Result<()> {
        println!("→ UI: Clarification needed for task {}: {}", task_id, question);
        Ok(())
    }
}

/// Example: Complete flow with DabGent MQ
pub async fn production_flow_example() -> eyre::Result<()> {
    // Initialize session
    let session = PlannerSession::new("sqlite::memory:", "prod-session-1".to_string()).await?;
    
    // Process initial command
    let events = session.process_command(Command::Initialize {
        user_input: "Analyze the codebase and run tests".to_string(),
        attachments: vec![],
    }).await?;
    
    println!("Initial events: {} emitted", events.len());
    
    // Simulate task completion
    let events = session.process_command(Command::HandleExecutorEvent(
        crate::planner::types::ExecutorEvent::TaskCompleted {
            node_id: 1,
            result: "Analysis complete".to_string(),
        }
    )).await?;
    
    println!("Task completion events: {} emitted", events.len());
    
    // Restore state from history
    let restored = session.restore_from_history().await?;
    println!("State restored: {} tasks", restored.state().tasks.len());
    
    Ok(())
}

/// Example: Fan-out pattern for monitoring
pub async fn monitoring_subscription_example(store: SqliteStore) -> eyre::Result<()> {
    // Multiple subscribers for different purposes
    
    // 1. Metrics collector
    let metrics_query = Query {
        stream_id: "planner".to_string(),
        event_type: Some("PlannerEvent".to_string()),
        aggregate_id: None,
    };
    
    let store_metrics = store.clone();
    tokio::spawn(async move {
        let mut stream = store_metrics.subscribe::<Event>(&metrics_query).unwrap();
        while let Some(Ok(event)) = stream.next().await {
            // Collect metrics
            match event {
                Event::TaskStatusUpdated { status, .. } => {
                    println!("[Metrics] Task status: {:?}", status);
                }
                Event::PlanningCompleted { .. } => {
                    println!("[Metrics] Planning completed");
                }
                _ => {}
            }
        }
    });
    
    // 2. Audit logger
    let audit_query = Query {
        stream_id: "planner".to_string(),
        event_type: None, // All events
        aggregate_id: None,
    };
    
    let store_audit = store.clone();
    tokio::spawn(async move {
        let mut stream = store_audit.subscribe::<Event>(&audit_query).unwrap();
        while let Some(Ok(event)) = stream.next().await {
            println!("[Audit] Event: {}", event.variant_type());
        }
    });
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_planner_session() {
        let session = PlannerSession::new(
            "sqlite::memory:",
            "test-session".to_string()
        ).await.unwrap();
        
        let events = session.process_command(Command::Initialize {
            user_input: "Test task".to_string(),
            attachments: vec![],
        }).await.unwrap();
        
        assert!(!events.is_empty());
        
        // Verify we can restore
        let restored = session.restore_from_history().await.unwrap();
        assert!(!restored.state().tasks.is_empty());
    }
}
