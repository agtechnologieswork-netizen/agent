use dabgent_agent::handler::Handler;
use dabgent_agent::planning_pipeline::PlanningPipelineBuilder;
use dabgent_agent::thread::{Event, State, Thread};
use dabgent_agent::toolbox::{basic::{toolset_with_tasklist, TaskList}, Validator};
use dabgent_mq::{EventStore, db::{sqlite::SqliteStore, Query}};
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use rig::client::ProviderClient;
use std::sync::Arc;
use tokio::sync::Mutex;

const TEST_MODEL: &str = "claude-sonnet-4-20250514";
const STREAM_ID: &str = "test_pipeline";
const AGGREGATE_ID: &str = "test_thread";

async fn create_test_store() -> SqliteStore {
    let pool = sqlx::SqlitePool::connect(":memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}

async fn create_real_sandbox() -> Result<Box<dyn SandboxDyn>> {
    // Create a real Docker container with Python environment
    let opts = ConnectOpts::default();

    // Use async connect with a closure
    let sandbox_result = std::sync::Arc::new(tokio::sync::Mutex::new(None));
    let sandbox_clone = sandbox_result.clone();

    opts.connect(|conn| async move {
        // Create a Python container for testing
        let container = conn
            .container()
            .from("python:3.11-slim")
            .with_workdir("/workspace")
            .with_exec(vec!["pip", "install", "--quiet", "uv"])
            .with_exec(vec!["uv", "init", "--quiet", "--name", "test-project"]);

        container.sync().await?;
        let sandbox = DaggerSandbox::from_container(container);

        // Store the sandbox
        *sandbox_clone.lock().await = Some(sandbox);
        Ok(())
    }).await?;

    // Extract the sandbox
    let sandbox = sandbox_result.lock().await.take()
        .ok_or_else(|| eyre::eyre!("Failed to create sandbox"))?;

    Ok(Box::new(sandbox) as Box<dyn SandboxDyn>)
}

// Real validator that executes Python code
struct PythonValidator;

impl Validator for PythonValidator {
    async fn run(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {
        // Execute the Python script to validate it works
        let result = sandbox.exec("python main.py").await?;

        if result.exit_code == 0 {
            Ok(Ok(()))
        } else {
            Ok(Err(format!(
                "Python execution failed with exit code {}: {}",
                result.exit_code,
                result.stderr
            )))
        }
    }
}

// Real TaskList implementation that tracks actual updates
#[derive(Clone)]
struct RealTaskList {
    updates: Arc<Mutex<Vec<String>>>,
}

impl RealTaskList {
    fn new() -> Self {
        Self {
            updates: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn get_updates(&self) -> Vec<String> {
        self.updates.lock().await.clone()
    }

    async fn was_updated(&self) -> bool {
        !self.updates.lock().await.is_empty()
    }
}

impl TaskList for RealTaskList {
    fn update(&self, current_content: String) -> Result<String> {
        let updates = self.updates.clone();

        // Track the update
        let update_record = format!("Update at {}: {}", chrono::Utc::now(), current_content);
        tokio::spawn(async move {
            updates.lock().await.push(update_record);
        });

        // Return a proper task list
        if current_content.is_empty() {
            Ok("# Task List\n\n- [ ] Create Python script\n- [ ] Validate execution\n- [ ] Complete task\n".to_string())
        } else if current_content.contains("- [ ] Create Python script") {
            Ok(current_content.replace("- [ ] Create Python script", "- [x] Create Python script"))
        } else if current_content.contains("- [ ] Validate execution") {
            Ok(current_content.replace("- [ ] Validate execution", "- [x] Validate execution"))
        } else {
            Ok(current_content.replace("- [ ]", "- [x]"))
        }
    }
}

async fn wait_for_completion(store: &SqliteStore) -> Result<()> {
    let query = Query {
        stream_id: STREAM_ID.to_owned(),
        event_type: None,
        aggregate_id: Some(AGGREGATE_ID.to_owned()),
    };

    let mut receiver = store.subscribe::<Event>(&query)?;
    let mut all_events = Vec::new();

    loop {
        match receiver.next().await {
            Some(Ok(event)) => {
                tracing::info!("Received event: {:?}", event);
                all_events.push(event);

                // Check the thread state after each event
                let thread = Thread::fold(&all_events);
                tracing::info!("Thread state: {:?}", thread.state);

                if matches!(thread.state, State::Done) {
                    tracing::info!("Pipeline completed successfully");
                    return Ok(());
                }

                if matches!(thread.state, State::Fail(_)) {
                    return Err(eyre::eyre!("Pipeline failed"));
                }
            }
            Some(Err(e)) => {
                return Err(eyre::eyre!("Error receiving event: {}", e));
            }
            None => {
                return Err(eyre::eyre!("Event stream ended unexpectedly"));
            }
        }
    }
}

#[tokio::test]
#[ignore] // Requires Docker and API key
async fn test_planning_pipeline_with_real_components() {
    dotenvy::dotenv().ok();
    let _ = tracing_subscriber::fmt::try_init();

    // Skip test if no API key is present
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
        return;
    }

    // Create all real components
    let llm = rig::providers::anthropic::Client::from_env();
    let sandbox = create_real_sandbox()
        .await
        .expect("Failed to create Docker sandbox");
    let store = create_test_store().await;
    let task_list = RealTaskList::new();
    let tools = toolset_with_tasklist(PythonValidator, task_list.clone());

    // Push a real task that requires actual file operations
    let event = Event::Prompted(
        "Create a Python script called main.py that:\n\
         1. Prints 'Hello from integration test!'\n\
         2. Calculates and prints the factorial of 5\n\
         3. Prints 'Test completed successfully'\n\
         Update the task list as you work, then call done to complete.".to_owned()
    );

    store
        .push_event(STREAM_ID, AGGREGATE_ID, &event, &Default::default())
        .await
        .expect("Failed to push prompt event");

    // Build pipeline with real components
    let pipeline = PlanningPipelineBuilder::new()
        .llm(llm)
        .store(store.clone())
        .sandbox(sandbox)
        .model(TEST_MODEL.to_owned())
        .preamble(
            "You are a Python developer working in a real Docker container. \
             Use bash commands to verify your environment. \
             Use write_file or edit_file to create files. \
             Use update_task_list to track your progress. \
             Before calling done, make sure your Python script runs without errors. \
             The done tool will validate by running 'python main.py'.".to_owned()
        )
        .tools(tools)
        .build()
        .expect("Failed to build pipeline");

    // Run pipeline
    let handle = tokio::spawn(async move {
        match pipeline.run(STREAM_ID.to_owned(), AGGREGATE_ID.to_owned()).await {
            Ok(()) => tracing::info!("Pipeline completed successfully"),
            Err(e) => tracing::error!("Pipeline failed: {}", e),
        }
    });

    // Wait for completion with extended timeout for real operations
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        wait_for_completion(&store)
    ).await;

    // Clean up
    handle.abort();

    // Verify results
    match result {
        Ok(Ok(())) => {
            tracing::info!("Integration test completed successfully");

            // Verify task list was actually updated
            assert!(
                task_list.was_updated().await,
                "TaskList should have been updated during pipeline execution"
            );

            let updates = task_list.get_updates().await;
            assert!(
                !updates.is_empty(),
                "Should have recorded task list updates"
            );

            tracing::info!("Task list updates: {:?}", updates);
        }
        Ok(Err(e)) => {
            panic!("Pipeline failed: {}", e);
        }
        Err(_) => {
            panic!("Test timed out - this may indicate the pipeline is stuck");
        }
    }
}

#[tokio::test]
#[ignore] // Requires Docker and API key
async fn test_planning_pipeline_error_handling() {
    dotenvy::dotenv().ok();
    let _ = tracing_subscriber::fmt::try_init();

    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
        return;
    }

    let llm = rig::providers::anthropic::Client::from_env();
    let sandbox = create_real_sandbox()
        .await
        .expect("Failed to create Docker sandbox");
    let store = create_test_store().await;
    let task_list = RealTaskList::new();

    // Use a validator that will fail initially
    struct StrictValidator;
    impl Validator for StrictValidator {
        async fn run(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {
            // Check if the script exists and contains specific content
            let result = sandbox.exec("python main.py").await?;
            if result.exit_code == 0 && result.stdout.contains("SUCCESS") {
                Ok(Ok(()))
            } else {
                Ok(Err("Script must print 'SUCCESS'".to_string()))
            }
        }
    }

    let tools = toolset_with_tasklist(StrictValidator, task_list);

    // Push a task that requires error correction
    let event = Event::Prompted(
        "Create a Python script main.py that prints 'SUCCESS'. \
         The validator requires this exact output. \
         If done fails, fix the script and try again. \
         Update the task list as you work.".to_owned()
    );

    store
        .push_event(STREAM_ID, AGGREGATE_ID, &event, &Default::default())
        .await
        .expect("Failed to push prompt event");

    let pipeline = PlanningPipelineBuilder::new()
        .llm(llm)
        .store(store.clone())
        .sandbox(sandbox)
        .model(TEST_MODEL.to_owned())
        .preamble(
            "You are a Python developer. The done tool validates that main.py prints 'SUCCESS'. \
             If validation fails, read the error, fix the script, and try again. \
             Use update_task_list to track your progress.".to_owned()
        )
        .tools(tools)
        .build()
        .expect("Failed to build pipeline");

    let handle = tokio::spawn(async move {
        pipeline.run(STREAM_ID.to_owned(), AGGREGATE_ID.to_owned()).await
    });

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        wait_for_completion(&store)
    ).await;

    handle.abort();

    assert!(result.is_ok(), "Pipeline should complete within timeout");
    assert!(result.unwrap().is_ok(), "Pipeline should eventually succeed after corrections");
}