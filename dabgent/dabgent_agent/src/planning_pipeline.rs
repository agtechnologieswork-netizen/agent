use crate::agent::{ToolWorker, Worker};
use crate::handler::Handler;
use crate::llm::LLMClient;
use crate::thread::{self, Event, Thread};
use crate::toolbox::ToolDyn;
use dabgent_mq::{EventStore, db::Query};
use dabgent_sandbox::SandboxDyn;
use eyre::{OptionExt, Result};

pub struct PlanningPipelineBuilder<T, S>
where
    T: LLMClient,
    S: EventStore,
{
    llm: Option<T>,
    store: Option<S>,
    model: Option<String>,
    preamble: Option<String>,
    sandbox: Option<Box<dyn SandboxDyn>>,
    tools: Vec<Box<dyn ToolDyn>>,
    _worker_marker: std::marker::PhantomData<Worker<T, S>>,
    _sandbox_marker: std::marker::PhantomData<Box<dyn SandboxDyn>>,
}

impl<T, S> PlanningPipelineBuilder<T, S>
where
    T: LLMClient,
    S: EventStore,
{
    pub fn new() -> Self {
        Self {
            llm: None,
            store: None,
            sandbox: None,
            model: None,
            preamble: None,
            tools: Vec::new(),
            _worker_marker: std::marker::PhantomData,
            _sandbox_marker: std::marker::PhantomData,
        }
    }

    pub fn llm(mut self, llm: T) -> Self {
        self.llm = Some(llm);
        self
    }

    pub fn store(mut self, store: S) -> Self {
        self.store = Some(store);
        self
    }

    pub fn sandbox(mut self, sandbox: Box<dyn SandboxDyn>) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    pub fn model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    pub fn preamble(mut self, preamble: String) -> Self {
        self.preamble = Some(preamble);
        self
    }

    pub fn tool(mut self, tool: Box<dyn ToolDyn>) -> Self {
        self.tools.push(tool);
        self
    }

    pub fn tools(mut self, tools: Vec<Box<dyn ToolDyn>>) -> Self {
        self.tools.extend(tools);
        self
    }

    pub fn build(self) -> Result<PlanningPipeline<T, S>> {
        let llm = self.llm.ok_or_eyre("LLM Client not provided")?;
        let store = self.store.ok_or_eyre("Event Store not provided")?;
        let model = self.model.ok_or_eyre("Model not provided")?;
        let preamble = self.preamble.ok_or_eyre("Preamble not provided")?;
        let sandbox = self.sandbox.ok_or_eyre("Sandbox not provided")?;

        let tool_defs = self.tools.iter().map(|tool| tool.definition()).collect();
        let planner_worker = Worker::new(llm, store.clone(), model, preamble, tool_defs);
        let tool_worker = ToolWorker::new(sandbox, store.clone(), self.tools);

        Ok(PlanningPipeline::new(store, planner_worker, tool_worker))
    }
}

pub struct PlanningPipeline<T, S>
where
    T: LLMClient,
    S: EventStore,
{
    store: S,
    planner_worker: Worker<T, S>,
    tool_worker: ToolWorker<S>,
}

impl<T, S> PlanningPipeline<T, S>
where
    T: LLMClient,
    S: EventStore,
{
    pub fn new(store: S, planner_worker: Worker<T, S>, tool_worker: ToolWorker<S>) -> Self {
        Self {
            store,
            planner_worker,
            tool_worker,
        }
    }

    pub async fn run(self, stream_id: String, aggregate_id: String) -> Result<()> {
        let Self {
            store,
            planner_worker,
            mut tool_worker,
        } = self;
        tokio::select! {
            res = planner_worker.run(&stream_id, &aggregate_id) => {
                tracing::error!("Planner worker failed: {:?}", res);
                res
            },
            res = tool_worker.run(&stream_id, &aggregate_id) => {
                tracing::error!("Tool worker failed: {:?}", res);
                res
            },
            res = Self::subscriber(&store, &stream_id, &aggregate_id) => res,
        }
    }

    pub async fn subscriber(store: &S, stream_id: &str, aggregate_id: &str) -> Result<()> {
        let query = Query {
            stream_id: stream_id.to_owned(),
            event_type: None,
            aggregate_id: Some(aggregate_id.to_owned()),
        };
        let mut receiver = store.subscribe::<Event>(&query)?;
        let mut events = store.load_events(&query, None).await?;
        while let Some(event) = receiver.next().await {
            let event = event?;
            events.push(event.clone());
            let thread = Thread::fold(&events);
            tracing::info!(?thread.state, ?event, "event");
            match thread.state {
                thread::State::Done => break,
                _ => continue,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thread::Event;
    use crate::toolbox::{self, basic::{toolset_with_tasklist, TaskList}};
    use dabgent_mq::db::sqlite::SqliteStore;
    use dabgent_sandbox::{Sandbox, ExecResult};
    use rig::client::ProviderClient;
    use std::collections::HashMap;
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

    async fn wait_for_completion(store: &SqliteStore) -> Result<()> {
        let query = Query {
            stream_id: STREAM_ID.to_owned(),
            event_type: None,
            aggregate_id: Some(AGGREGATE_ID.to_owned()),
        };

        let mut receiver = store.subscribe::<Event>(&query)?;

        // Keep track of all events to check thread state
        let mut all_events = Vec::new();

        loop {
            match receiver.next().await {
                Some(Ok(event)) => {
                    tracing::debug!("Received event: {:?}", event);
                    all_events.push(event);

                    // Check the thread state after each event
                    let thread = Thread::fold(&all_events);
                    tracing::debug!("Thread state: {:?}", thread.state);

                    if matches!(thread.state, thread::State::Done) {
                        tracing::info!("Pipeline completed successfully");
                        return Ok(());
                    }

                    // Also check for failure state
                    if matches!(thread.state, thread::State::Fail(_)) {
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

    #[derive(Clone)]
    struct MockSandbox {
        files: Arc<Mutex<HashMap<String, String>>>,
    }

    impl MockSandbox {
        fn new() -> Self {
            Self {
                files: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        async fn add_file(&self, path: &str, content: &str) {
            self.files.lock().await.insert(path.to_string(), content.to_string());
        }

        async fn get_file(&self, path: &str) -> Option<String> {
            self.files.lock().await.get(path).cloned()
        }
    }

    impl Sandbox for MockSandbox {
        async fn exec(&mut self, command: &str) -> Result<ExecResult> {
            Ok(ExecResult {
                exit_code: 0,
                stdout: format!("Mock execution: {}", command),
                stderr: String::new(),
            })
        }

        async fn write_file(&mut self, path: &str, content: &str) -> Result<()> {
            self.files.lock().await.insert(path.to_string(), content.to_string());
            Ok(())
        }

        async fn write_files(&mut self, files: Vec<(&str, &str)>) -> Result<()> {
            let mut file_map = self.files.lock().await;
            for (path, content) in files {
                file_map.insert(path.to_string(), content.to_string());
            }
            Ok(())
        }

        async fn read_file(&self, path: &str) -> Result<String> {
            self.files
                .lock()
                .await
                .get(path)
                .cloned()
                .ok_or_else(|| eyre::eyre!("File not found: {}", path))
        }

        async fn delete_file(&mut self, path: &str) -> Result<()> {
            self.files.lock().await.remove(path);
            Ok(())
        }

        async fn list_directory(&self, _path: &str) -> Result<Vec<String>> {
            Ok(self.files.lock().await.keys().cloned().collect())
        }

        async fn set_workdir(&mut self, _path: &str) -> Result<()> {
            Ok(())
        }
    }

    // SandboxDyn is automatically implemented for types that implement Sandbox + Send + Sync

    struct MockValidator;

    impl toolbox::Validator for MockValidator {
        async fn run(&self, _sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {
            Ok(Ok(()))
        }
    }

    #[derive(Clone)]
    struct MockTaskList {
        updated: Arc<Mutex<bool>>,
    }

    impl MockTaskList {
        fn new() -> Self {
            Self {
                updated: Arc::new(Mutex::new(false)),
            }
        }

        async fn was_updated(&self) -> bool {
            *self.updated.lock().await
        }
    }

    impl TaskList for MockTaskList {
        fn update(&self, _current_content: String) -> Result<String> {
            let updated = self.updated.clone();
            tokio::spawn(async move {
                *updated.lock().await = true;
            });
            Ok("# Task List\n- [x] Task completed\n- [ ] New task".to_string())
        }
    }


    #[tokio::test]
    async fn test_planning_pipeline_basic_flow() {
        dotenvy::dotenv().ok();
        tracing_subscriber::fmt::init();

        // Skip test if no API key is present
        if std::env::var("ANTHROPIC_API_KEY").is_err() {
            eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let llm = rig::providers::anthropic::Client::from_env();
        let sandbox = MockSandbox::new();
        let store = create_test_store().await;
        let task_list = MockTaskList::new();
        let tools = toolset_with_tasklist(MockValidator, task_list.clone());

        // Push initial prompt that will trigger task list usage and done tool
        let event = Event::Prompted(
            "Create a simple task list with one item 'Complete test' using the update_task_list tool, \
             then immediately call the done tool to complete this task.".to_owned()
        );
        store
            .push_event(STREAM_ID, AGGREGATE_ID, &event, &Default::default())
            .await
            .expect("Failed to push prompt event");

        // Build and run pipeline
        let pipeline = PlanningPipelineBuilder::new()
            .llm(llm)
            .store(store.clone())
            .sandbox(Box::new(sandbox))
            .model(TEST_MODEL.to_owned())
            .preamble(
                "You are a helpful assistant. You have access to the following tools: \
                 update_task_list (to update task lists) and done (to mark completion). \
                 When asked to create a task list and complete, first call update_task_list, \
                 then call done to signal completion.".to_owned()
            )
            .tools(tools)
            .build()
            .expect("Failed to build pipeline");

        // Run pipeline in background
        let handle = tokio::spawn(async move {
            let result = pipeline.run(STREAM_ID.to_owned(), AGGREGATE_ID.to_owned()).await;
            if let Err(e) = result {
                tracing::error!("Pipeline error: {}", e);
            }
        });

        // Wait for completion with reasonable timeout for LLM
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(45),
            wait_for_completion(&store)
        ).await;

        // Cancel the pipeline task
        handle.abort();

        match result {
            Ok(Ok(())) => {
                tracing::info!("Test completed successfully");
                // Verify task list was updated
                assert!(task_list.was_updated().await, "TaskList should be updated during pipeline execution");
            }
            Ok(Err(e)) => {
                panic!("Pipeline failed: {}", e);
            }
            Err(_) => {
                panic!("Test timed out waiting for pipeline completion");
            }
        }
    }

    #[tokio::test]
    async fn test_planning_pipeline_with_file_operations() {
        dotenvy::dotenv().ok();

        // Skip test if no API key is present
        if std::env::var("ANTHROPIC_API_KEY").is_err() {
            eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let llm = rig::providers::anthropic::Client::from_env();
        let sandbox = MockSandbox::new();
        sandbox.add_file("main.py", "print('existing code')").await;

        let store = create_test_store().await;
        let task_list = MockTaskList::new();
        let tools = toolset_with_tasklist(MockValidator, task_list.clone());

        // Push prompt that requires file interaction and completion
        let event = Event::Prompted("Write 'print(\"Hello, World!\")' to a file called main.py, then call the done tool to complete the task.".to_owned());
        store
            .push_event(STREAM_ID, AGGREGATE_ID, &event, &Default::default())
            .await
            .expect("Failed to push prompt event");

        let sandbox_clone = sandbox.clone();
        let pipeline = PlanningPipelineBuilder::new()
            .llm(llm)
            .store(store.clone())
            .sandbox(Box::new(sandbox_clone))
            .model(TEST_MODEL.to_owned())
            .preamble("You are a Python developer. Use write_file to write to files. After completing the task, call the 'done' tool to signal completion.".to_owned())
            .tools(tools)
            .build()
            .expect("Failed to build pipeline");

        let handle = tokio::spawn(async move {
            pipeline.run(STREAM_ID.to_owned(), AGGREGATE_ID.to_owned()).await
        });

        // Wait for completion with reasonable timeout for LLM
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            wait_for_completion(&store)
        ).await;

        handle.abort();

        assert!(result.is_ok(), "Pipeline should complete within timeout");
        assert!(result.unwrap().is_ok(), "Pipeline should complete successfully");
    }
}
