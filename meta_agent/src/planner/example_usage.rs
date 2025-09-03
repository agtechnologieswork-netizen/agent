// Example usage of the Handler trait pattern
// This shows how the planner can be used without bus/messaging infrastructure

use crate::planner::{Command, Event, Handler, Planner, PlannerCmd, ExecutorEvent};
#[cfg(feature = "mq")]
use dabgent_mq::db::{EventStore as MqEventStore, Query};
#[cfg(feature = "mq")]
use dabgent_mq::db::sqlite::SqliteStore;
#[cfg(feature = "mq")]
use dabgent_mq::db::Metadata as MqMetadata;

/// Example of how to use the planner in a simple synchronous context
pub fn simple_usage_example() {
    // Create a new planner
    let mut planner = Planner::new();
    
    // Process user input
    let events = planner.process(Command::Initialize {
        user_input: "Analyze the codebase\nRun the tests\nDeploy to production".to_string(),
        attachments: vec![],
    }).expect("Failed to initialize");
    
    #[cfg(feature = "mq")]
    {
        // Directly persist events to DabGent MQ (SQLite)
        // Use an in-memory SQLite pool for demonstration
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(5)
                .connect("sqlite::memory:")
                .await
                .expect("pool");
            let store = SqliteStore::new(pool);
            store.migrate().await;
            let aggregate_id = "planner-1";
            for ev in events {
                let meta = MqMetadata::default();
                store
                    .push_event("planner", aggregate_id, &ev, &meta)
                    .await
                    .expect("push");
            }

            // Reload and fold
            let query = Query {
                stream_id: "planner".to_string(),
                event_type: None,
                aggregate_id: Some(aggregate_id.to_string()),
            };
            let loaded: Vec<Event> = store.load_events(&query, None).await.expect("load");
            let _restored = Planner::fold(&loaded);
        });
    }
    
    // Simulate executor completing a task
    let events = planner.process(Command::HandleExecutorEvent(
        ExecutorEvent::TaskCompleted {
            node_id: 1,
            result: "Analysis complete: 1000 lines of code".to_string(),
        }
    )).expect("Failed to process event");
    
    for event in &events {
        println!("Event emitted: {:?}", event);
    }
}

/// Example of event sourcing - rebuilding state from events
pub fn event_sourcing_example() {
    // Start with some events (e.g., loaded from storage)
    let historical_events = vec![
        Event::TasksPlanned {
            tasks: vec![
                crate::planner::TaskPlan {
                    id: 1,
                    description: "Setup development environment".to_string(),
                    kind: crate::planner::NodeKind::Processing,
                    attachments: vec![],
                },
                crate::planner::TaskPlan {
                    id: 2,
                    description: "Install dependencies".to_string(),
                    kind: crate::planner::NodeKind::ToolCall,
                    attachments: vec![],
                },
            ],
        },
        Event::TaskDispatched {
            task_id: 1,
            command: PlannerCmd::ExecuteTask {
                node_id: 1,
                kind: crate::planner::NodeKind::Processing,
                parameters: "Setup development environment".to_string(),
            },
        },
        Event::TaskStatusUpdated {
            task_id: 1,
            status: crate::planner::TaskStatus::Completed,
            result: Some("Environment ready".to_string()),
        },
    ];
    
    // Rebuild planner state from events
    let planner = Planner::fold(&historical_events);
    
    // The planner now has the complete state
    println!("Planner state restored:");
    println!("  Tasks: {}", planner.state().tasks.len());
    println!("  Cursor: {}", planner.state().cursor);
    println!("  Task 1 status: {:?}", planner.state().get_task(1).map(|t| t.status));
}

/// Example of integrating with an async message bus
pub async fn async_bus_integration_example() {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    
    // Shared planner wrapped for async access
    let planner = Arc::new(Mutex::new(Planner::new()));
    
    // Command handler (receives from bus)
    let command_handler = {
        let planner = Arc::clone(&planner);
        async move |command: Command| {
            let mut planner = planner.lock().await;
            match planner.process(command) {
                Ok(events) => {
                    // Publish events to the bus
                    for event in events {
                        publish_event_to_bus(event).await;
                    }
                }
                Err(e) => {
                    eprintln!("Error processing command: {}", e);
                }
            }
        }
    };
    
    // Executor event handler (receives from bus)
    let executor_handler = {
        let planner = Arc::clone(&planner);
        async move |executor_event: ExecutorEvent| {
            let mut planner = planner.lock().await;
            match planner.process(Command::HandleExecutorEvent(executor_event)) {
                Ok(events) => {
                    for event in events {
                        publish_event_to_bus(event).await;
                    }
                }
                Err(e) => {
                    eprintln!("Error handling executor event: {}", e);
                }
            }
        }
    };
    
    // Usage: bus would call these handlers
    // command_handler(Command::Initialize { ... }).await;
    // executor_handler(ExecutorEvent::TaskCompleted { ... }).await;
}

/// Mock function to represent publishing to a message bus
async fn publish_event_to_bus(event: Event) {
    println!("Publishing to bus: {:?}", event);
    // In reality, this would publish to your actual message bus
}

/// Example of handling clarification flow
pub fn clarification_flow_example() {
    let mut planner = Planner::new();
    
    // Initialize with a question that needs clarification
    planner.process(Command::Initialize {
        user_input: "What database should I use for the project?".to_string(),
        attachments: vec![],
    }).unwrap();
    
    // Planner recognizes this needs clarification and emits appropriate event
    // The executor/UI would show this to the user
    
    // When executor needs clarification from user
    planner.process(Command::HandleExecutorEvent(
        ExecutorEvent::NeedsClarification {
            node_id: 1,
            question: "Do you prefer SQL or NoSQL? What are your requirements?".to_string(),
        }
    )).unwrap();
    
    // User provides clarification
    planner.process(Command::HandleExecutorEvent(
        ExecutorEvent::ClarificationProvided {
            node_id: 1,
            answer: "SQL, PostgreSQL specifically, for ACID compliance".to_string(),
        }
    )).unwrap();
    
    // Task execution continues with the clarification
}

/// Example showing the benefits of the Handler pattern
/// 
/// Benefits:
/// 1. **Separation of Concerns**: Business logic is in the Handler, bus/messaging elsewhere
/// 2. **Event Sourcing**: Complete audit trail and ability to rebuild state
/// 3. **Testability**: Easy to test without mocking bus/messaging infrastructure
/// 4. **Replayability**: Can replay events to debug or analyze behavior
/// 5. **Integration Flexibility**: Can work with any message bus or even synchronously
pub fn handler_pattern_benefits() {
    // The Handler trait provides a clean interface that:
    
    // 1. Can be tested in isolation
    let mut planner = Planner::new();
    let result = planner.process(Command::Initialize {
        user_input: "Test input".to_string(),
        attachments: vec![],
    });
    assert!(result.is_ok());
    
    // 2. Can rebuild state from events (event sourcing)
    let events = result.unwrap();
    let restored_planner = Planner::fold(&events);
    assert_eq!(restored_planner.state().tasks.len(), planner.state().tasks.len());
    
    // 3. Provides clear separation between command processing and infrastructure
    // The planner doesn't know or care about:
    // - How commands are delivered (HTTP, gRPC, CLI, etc.)
    // - How events are published (Kafka, Redis, in-memory, etc.)
    // - Whether it's running synchronously or asynchronously
    
    println!("Handler pattern successfully demonstrates separation of concerns!");
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_handler_pattern() {
        handler_pattern_benefits();
    }
    
    #[test]
    fn test_simple_flow() {
        simple_usage_example();
    }
    
    #[test]
    fn test_event_sourcing() {
        event_sourcing_example();
    }
    
    #[test] 
    fn test_clarification() {
        clarification_flow_example();
    }
}
