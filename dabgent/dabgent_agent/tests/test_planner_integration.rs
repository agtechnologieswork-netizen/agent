//! Integration tests for the planner module

use dabgent_agent::planner;
use dabgent_agent::llm::{Completion, CompletionResponse, FinishReason, LLMClient};
use dabgent_agent::toolbox::ToolDyn;
use dabgent_mq::db::sqlite::SqliteStore;
use dabgent_mq::EventStore;
use eyre::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Mock LLM client for testing
#[derive(Clone)]
struct MockLLMClient {
    responses: Arc<Mutex<Vec<String>>>,
}

impl MockLLMClient {
    fn new(responses: Vec<String>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
        }
    }
}

impl LLMClient for MockLLMClient {
    async fn completion(&self, _completion: Completion) -> Result<CompletionResponse> {
        let mut responses = self.responses.lock().await;
        let response = responses.pop().unwrap_or_else(|| "No response".to_string());
        
        Ok(CompletionResponse {
            choice: rig::OneOrMany::one(rig::message::AssistantContent::Text(rig::agent::Text { text: response })),
            finish_reason: FinishReason::Stop,
            output_tokens: 20,
        })
    }
}

async fn setup_store() -> SqliteStore {
    let pool = sqlx::SqlitePool::connect(":memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}

#[tokio::test]
async fn test_planner_completes_successfully() {
    let store = setup_store().await;
    // Return valid XML that will parse and complete successfully
    let llm = MockLLMClient::new(vec![
        r#"<tasks>
<task>
  <id>1</id>
  <description>Test task</description>
  <kind>Processing</kind>
</task>
</tasks>"#.to_string()
    ]);
    let preamble = "Test".to_string();
    let tools: Vec<Box<dyn ToolDyn>> = vec![];
    
    // Should complete successfully within 5 seconds
    let result = planner::runner::run_with_timeout(
        llm,
        store,
        preamble,
        tools,
        "Test task".to_string(),
        5,
    ).await;
    
    // The runner now completes tasks automatically, so it should succeed
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_planner_initialization() {
    use dabgent_agent::handler::Handler;
    use dabgent_agent::planner::{Planner, Command, TaskPlan, NodeKind};
    
    let mut planner = Planner::new();
    let command = Command::Initialize {
        tasks: vec![
            TaskPlan {
                id: 1,
                description: "Test task".to_string(),
                kind: NodeKind::Processing,
            },
        ],
    };
    
    let events = planner.process(command);
    assert!(events.is_ok());
    assert!(!events.unwrap().is_empty());
}

#[tokio::test]
async fn test_event_persistence() {
    use dabgent_agent::handler::Handler;
    use dabgent_agent::planner::{Planner, Command, Event, TaskPlan, NodeKind};
    use dabgent_mq::db::Query;
    
    let store = setup_store().await;
    let mut planner = Planner::new();
    
    let command = Command::Initialize {
        tasks: vec![
            TaskPlan {
                id: 1,
                description: "Test task".to_string(),
                kind: NodeKind::Processing,
            },
        ],
    };
    let events = planner.process(command).unwrap();
    
    // Persist events
    let aggregate_id = "test-123";
    for event in &events {
        store.push_event("test", aggregate_id, event, &Default::default())
            .await
            .unwrap();
    }
    
    // Load events back
    let query = Query {
        stream_id: "test".to_owned(),
        event_type: None,
        aggregate_id: Some(aggregate_id.to_owned()),
    };
    
    let loaded_events = store.load_events::<Event>(&query, None).await.unwrap();
    assert_eq!(loaded_events.len(), events.len());
}