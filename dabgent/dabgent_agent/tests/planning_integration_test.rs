use dabgent_agent::event::Event;
use dabgent_agent::planning_mode::{create_planner_pipeline, create_executor_pipeline, monitor_plan_execution, ModelConfig, DEFAULT_MODEL, PLANNING_PROMPT};
use dabgent_agent::toolbox::planning::planning_toolset;
use dabgent_mq::{EventStore, Query};
use dabgent_mq::db::sqlite::SqliteStore;
use dabgent_sandbox::NoOpSandbox;
use std::time::Duration;
use tokio::time::sleep;
use rig::providers::anthropic;
use rig::client::ProviderClient;

#[tokio::test]
#[ignore = "requires ANTHROPIC_API_KEY environment variable"]
async fn test_planning_mode_complete_flow() {
    // Initialize tracing for debugging
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();

    // Use SQLite in-memory store for testing
    let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
    let store = SqliteStore::new(pool);
    store.migrate().await;
    let stream_id = "test_planning_session";

    // Use real Anthropic client from environment
    let llm = anthropic::Client::from_env();

    tracing::info!("=== Starting Planning Mode Integration Test ===");

    // Step 1: Set up planner configuration (like run_planning_mode does)
    tracing::info!("Step 1: Configuring planner...");
    let planning_config = Event::LLMConfig {
        model: DEFAULT_MODEL.to_string(),
        temperature: 0.7,
        max_tokens: 4096,
        preamble: Some(PLANNING_PROMPT.to_string()),
        tools: Some(
            planning_toolset(store.clone(), stream_id.to_string())
                .iter()
                .map(|tool| tool.definition())
                .collect()
        ),
        recipient: Some("planner".to_string()),
        parent: None,
    };

    store
        .push_event(stream_id, "planner", &planning_config, &Default::default())
        .await
        .unwrap();

    tracing::info!("✓ Planner configured");

    // Step 2: Create planner and executor pipelines using extracted functions
    tracing::info!("Step 2: Creating pipelines...");

    let planner_pipeline = create_planner_pipeline(llm.clone(), store.clone(), stream_id.to_string());

    // Create executor with NoOpSandbox for testing
    let executor_pipeline = create_executor_pipeline(
        llm.clone(),
        store.clone(),
        NoOpSandbox::new(),
        vec![], // No actual tools for test
    );

    // Start planner pipeline in background
    let planner_stream_id = stream_id.to_string();
    let planner_handle = tokio::spawn(async move {
        tracing::info!("Planner pipeline starting...");
        let result = planner_pipeline.run(planner_stream_id).await;
        tracing::info!("Planner pipeline ended: {:?}", result);
        result
    });

    // Start executor pipeline in background
    let executor_stream_id = stream_id.to_string();
    let executor_handle = tokio::spawn(async move {
        tracing::info!("Executor pipeline starting...");
        let result = executor_pipeline.run(executor_stream_id).await;
        tracing::info!("Executor pipeline ended: {:?}", result);
        result
    });

    sleep(Duration::from_millis(100)).await;
    tracing::info!("✓ Pipelines started");

    // Step 3: Start monitor task (using extracted function)
    tracing::info!("Step 3: Starting monitor task...");
    let monitor_store = store.clone();
    let monitor_stream_id = stream_id.to_string();
    let model_config = ModelConfig {
        model: DEFAULT_MODEL.to_string(),
        temperature: 0.7,
        max_tokens: 4096,
        preamble: dabgent_agent::planning_mode::PYTHON_SYSTEM_PROMPT.to_string(),
    };

    // Start monitor in background with timeout
    let monitor_handle = tokio::spawn(async move {
        tracing::info!("Monitor starting...");
        // Use select to timeout the monitor after a certain duration
        tokio::select! {
            result = monitor_plan_execution(monitor_store, monitor_stream_id, model_config) => {
                tracing::info!("Monitor ended: {:?}", result);
                result
            }
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                tracing::info!("Monitor timed out after 30 seconds");
                Ok(())
            }
        }
    });

    sleep(Duration::from_millis(100)).await;
    tracing::info!("✓ Monitor task started");

    // Step 4: Send initial user message to planner
    tracing::info!("Step 4: User sends initial message...");
    let user_msg = Event::UserMessage(
        rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
            text: "create hello world in 1 step".to_string(),
        }))
    );

    store
        .push_event(stream_id, "planner", &user_msg, &Default::default())
        .await
        .unwrap();

    tracing::info!("Sent: 'create hello world in 1 step'");

    // Wait for plan creation (real API calls take longer)
    sleep(Duration::from_secs(5)).await;

    // Check if plan was created
    let query = Query::stream(stream_id).aggregate("planner");
    let planner_events = store.load_events::<Event>(&query, None).await.unwrap();

    // Debug: log all events
    tracing::debug!("Planner events after waiting:");
    for (i, event) in planner_events.iter().enumerate() {
        let event_type = match event {
            Event::LLMConfig { .. } => "LLMConfig",
            Event::UserMessage(_) => "UserMessage",
            Event::AgentMessage { .. } => "AgentMessage",
            Event::PlanCreated { .. } => "PlanCreated",
            Event::ToolResult(_) => "ToolResult",
            _ => "Other",
        };
        tracing::debug!("  {}: {}", i, event_type);
    }

    // Check for tool calls in AgentMessage
    if let Some(Event::AgentMessage { response, .. }) = planner_events.last() {
        tracing::debug!("AgentMessage contains:");
        // OneOrMany has iter() method
        for item in response.choice.iter() {
            match item {
                rig::message::AssistantContent::ToolCall(tc) => {
                    tracing::debug!("Tool call: {} with args: {}", tc.function.name, tc.function.arguments);
                }
                rig::message::AssistantContent::Text(t) => {
                    tracing::debug!("Text: {}", t.text);
                }
                rig::message::AssistantContent::Reasoning(r) => {
                    tracing::debug!("Reasoning: {:?}", r.reasoning);
                }
            }
        }
    }

    let plan_created = planner_events.iter().any(|e| matches!(e, Event::PlanCreated { .. }));
    assert!(plan_created, "Plan should have been created - found {} events", planner_events.len());
    tracing::info!("✓ Plan created successfully");

    // Wait for task completion and monitor to send UserInputRequested (with real API)
    sleep(Duration::from_secs(10)).await;

    // Check if UserInputRequested was sent
    let planner_events = store.load_events::<Event>(&query, None).await.unwrap();

    // Debug: print all events to see what happened
    tracing::info!("Planner events after task execution:");
    for (i, event) in planner_events.iter().enumerate() {
        let event_type = match event {
            Event::LLMConfig { .. } => "LLMConfig",
            Event::UserMessage(_) => "UserMessage",
            Event::AgentMessage { .. } => "AgentMessage",
            Event::PlanCreated { .. } => "PlanCreated",
            Event::PlanCompleted { .. } => "PlanCompleted",
            Event::UserInputRequested { .. } => "UserInputRequested",
            Event::ToolResult(_) => "ToolResult",
            _ => "Other",
        };
        tracing::info!("  Event {}: {}", i, event_type);
    }

    let has_input_request = planner_events.iter().any(|e| matches!(e, Event::UserInputRequested { .. }));
    assert!(has_input_request, "UserInputRequested should have been sent after completion");
    tracing::info!("✓ UserInputRequested sent after task completion");

    // Step 5: User responds to prompt
    tracing::info!("Step 5: User responds with 'add emoji'...");
    let follow_up_msg = Event::UserMessage(
        rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
            text: "add emoji to the app".to_string(),
        }))
    );

    store
        .push_event(stream_id, "planner", &follow_up_msg, &Default::default())
        .await
        .unwrap();

    tracing::info!("Sent: 'add emoji to the app'");

    // Wait for planner response (with real API)
    sleep(Duration::from_secs(5)).await;

    // Check if planner responded
    let final_events = store.load_events::<Event>(&query, None).await.unwrap();

    tracing::debug!("Final planner events:");
    for (i, event) in final_events.iter().enumerate() {
        let event_type = match event {
            Event::LLMConfig { .. } => "LLMConfig",
            Event::UserMessage(_) => "UserMessage",
            Event::AgentMessage { .. } => "AgentMessage",
            Event::PlanCreated { .. } => "PlanCreated",
            Event::PlanCompleted { .. } => "PlanCompleted",
            Event::UserInputRequested { .. } => "UserInputRequested",
            Event::ToolResult(_) => "ToolResult",
            _ => "Other",
        };
        tracing::debug!("  {}: {}", i, event_type);
    }

    let agent_message_count = final_events.iter()
        .filter(|e| matches!(e, Event::AgentMessage { .. }))
        .count();

    tracing::info!("Agent messages generated: {}", agent_message_count);

    // This is the assertion that verifies the planner responds to follow-up
    assert!(agent_message_count >= 2,
        "Planner should have responded at least twice (initial + follow-up), got {} responses",
        agent_message_count);

    tracing::info!("✓ TEST PASSED: Planner responded to follow-up message!");

    // Cleanup - send shutdown signal
    store
        .push_event(stream_id, "planner", &Event::PipelineShutdown, &Default::default())
        .await
        .unwrap();

    // Give pipelines time to shutdown gracefully
    let _ = tokio::time::timeout(Duration::from_secs(1), planner_handle).await;
    let _ = tokio::time::timeout(Duration::from_secs(1), executor_handle).await;
    let _ = tokio::time::timeout(Duration::from_secs(1), monitor_handle).await;
}

