use dabgent_agent::planner::{Planner, ThreadSettings};
use dabgent_agent::processor::{Pipeline, Processor, ThreadProcessor, ToolProcessor};
use dabgent_agent::toolbox::{self, basic::toolset, planning::planning_toolset};
use dabgent_mq::{EventStore, db::sqlite::SqliteStore};
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::{Sandbox, SandboxDyn};
use eyre::Result;
use rig::client::ProviderClient;
use std::sync::{Arc, Mutex};

const MODEL: &str = "claude-sonnet-4-20250514";

const SYSTEM_PROMPT: &str = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.
";

const PLANNING_PROMPT: &str = "
You are an engineering project manager.
Your role is to break down complex tasks into manageable steps.
Create a clear, actionable plan that an engineer can follow.

When creating a plan:
1. Break down the task into clear, specific steps
2. Each step should be a concrete action
3. Order the steps logically
4. Use the create_plan tool to submit your plan

The create_plan tool expects an array of task descriptions.
Each task should be a concrete, actionable step that can be independently executed.
";

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    const STREAM_ID: &str = "planning-pipeline";
    let prompt = "Build a weather app that fetches and displays weather for a given city using an API";

    let store = store().await;

    // Run the planning and execution pipeline
    planning_pipeline(STREAM_ID, store, &prompt).await.unwrap();
}

pub async fn planning_pipeline(stream_id: &str, store: impl EventStore + Clone, task: &str) -> Result<()> {
    let stream_id = stream_id.to_owned();
    let task = task.to_owned();

    // Shared planner state
    let planner: Arc<Mutex<Option<Planner<_>>>> = Arc::new(Mutex::new(None));

    // Thread settings
    let settings = ThreadSettings::new(MODEL, 0.7, 4096);

    let opts = ConnectOpts::default();
    opts.connect(|client| async move {
        println!("=== SINGLE PIPELINE, MULTI-AGENT ARCHITECTURE ===\n");
        println!("Task: {}\n", task);

        let llm = rig::providers::anthropic::Client::from_env();

        // Create processors for different agent types

        // 1. Planning Agent (no sandbox needed)
        let planning_thread = ThreadProcessor::new(
            llm.clone(),
            store.clone(),
        );

        // Create a dummy sandbox for planning tools
        // Planning tools don't need actual sandbox functionality
        let planning_sandbox = DummySandbox::new();

        let planning_tools = planning_toolset(
            planner.clone(),
            store.clone(),
            stream_id.clone(),
            settings.clone(),
        );

        let planning_tool_processor = ToolProcessor::new(
            planning_sandbox.boxed(),
            store.clone(),
            planning_tools,
            Some("planner".to_string()),  // Only process planner messages
        );

        // 2. Execution Agent (with sandbox)
        let execution_thread = ThreadProcessor::new(
            llm.clone(),
            store.clone(),
        );

        let execution_sandbox = sandbox(&client).await?;
        let execution_tools = toolset(Validator);

        let execution_tool_processor = ToolProcessor::new(
            execution_sandbox.boxed(),
            store.clone(),
            execution_tools,
            None,  // Process all non-planner messages (worker threads)
        );

        // Create the unified pipeline with both agent processors
        // They will route based on the recipient field in events
        let pipeline = Pipeline::new(
            store.clone(),
            vec![
                planning_thread.boxed(),
                planning_tool_processor.boxed(),
                execution_thread.boxed(),
                execution_tool_processor.boxed(),
            ],
        );

        // Start the pipeline in background
        let pipeline_handle = tokio::spawn({
            let stream_id = stream_id.clone();
            async move {
                println!("Pipeline started, processing events...\n");
                pipeline.run(stream_id).await
            }
        });

        // === PHASE 1: PLANNING ===
        println!("=== PHASE 1: PLANNING ===");

        // Configure the planner agent
        let planning_config = dabgent_agent::event::Event::LLMConfig {
            model: MODEL.to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            preamble: Some(PLANNING_PROMPT.to_string()),
            tools: Some(
                planning_toolset(planner.clone(), store.clone(), stream_id.clone(), settings.clone())
                    .iter()
                    .map(|tool| tool.definition())
                    .collect()
            ),
            recipient: Some("planner".to_string()),
        };
        store
            .push_event(&stream_id, "planner", &planning_config, &Default::default())
            .await?;

        // Send task to planner
        let user_message = dabgent_agent::event::Event::UserMessage(
            rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
                text: format!("Please create a plan for the following task: {}", task),
            }))
        );
        store
            .push_event(&stream_id, "planner", &user_message, &Default::default())
            .await?;

        // Wait for planning to complete
        println!("Creating plan...");
        let mut attempts = 0;
        while attempts < 30 {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            if planner.lock().unwrap().is_some() {
                println!("✓ Plan created!\n");
                break;
            }
            attempts += 1;
            if attempts % 5 == 0 {
                println!("  Still planning... ({} seconds)", attempts);
            }
        }

        // === PHASE 2: EXECUTION ===
        if let Some(planner_instance) = planner.lock().unwrap().as_ref() {
            let tasks = planner_instance.tasks();

            if !tasks.is_empty() {
                println!("=== PHASE 2: EXECUTION ===");
                println!("Executing {} tasks:\n", tasks.len());

                // Configure each worker thread and dispatch tasks
                for (i, task) in tasks.iter().enumerate() {
                    let thread_id = task.thread();
                    println!("Task {}/{}: {}", i + 1, tasks.len(), task.description);
                    println!("  Thread: {}", thread_id);

                    // Configure the worker thread with execution prompt and tools
                    let worker_config = dabgent_agent::event::Event::LLMConfig {
                        model: MODEL.to_string(),
                        temperature: 0.7,
                        max_tokens: 4096,
                        preamble: Some(SYSTEM_PROMPT.to_string()),
                        tools: Some(
                            toolset(Validator)
                                .iter()
                                .map(|tool| tool.definition())
                                .collect()
                        ),
                        recipient: Some(thread_id.to_string()),
                    };
                    store
                        .push_event(&stream_id, thread_id, &worker_config, &Default::default())
                        .await?;

                    // Send task to worker
                    let task_message = dabgent_agent::event::Event::UserMessage(
                        rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
                            text: task.description.clone(),
                        }))
                    );
                    store
                        .push_event(&stream_id, thread_id, &task_message, &Default::default())
                        .await?;

                    println!("  ✓ Dispatched to worker\n");

                    // Give some time between tasks for processing
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }

                // Wait for all tasks to complete
                println!("Waiting for workers to complete tasks...");
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            }
        } else {
            println!("No plan was created.");
        }

        // Stop the pipeline
        pipeline_handle.abort();
        println!("\n✅ Pipeline execution complete!");

        Ok(())
    })
    .await
    .map_err(Into::into)
}

async fn sandbox(client: &dagger_sdk::DaggerConn) -> Result<DaggerSandbox> {
    let opts = dagger_sdk::ContainerBuildOptsBuilder::default()
        .dockerfile("Dockerfile")
        .build()?;
    let ctr = client
        .container()
        .build_opts(client.host().directory("./examples"), opts);
    ctr.sync().await?;
    let sandbox = DaggerSandbox::from_container(ctr, client.clone());
    Ok(sandbox)
}

async fn store() -> SqliteStore {
    let pool = sqlx::SqlitePool::connect(":memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}

/// Dummy sandbox for planning tools that don't need actual execution
struct DummySandbox;

impl DummySandbox {
    fn new() -> Self {
        Self
    }
}

impl Sandbox for DummySandbox {
    async fn exec(&mut self, _command: &str) -> Result<dabgent_sandbox::ExecResult> {
        Ok(dabgent_sandbox::ExecResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    async fn write_file(&mut self, _path: &str, _content: &str) -> Result<()> {
        Ok(())
    }

    async fn write_files(&mut self, _files: Vec<(&str, &str)>) -> Result<()> {
        Ok(())
    }

    async fn read_file(&self, _path: &str) -> Result<String> {
        Ok(String::new())
    }

    async fn delete_file(&mut self, _path: &str) -> Result<()> {
        Ok(())
    }

    async fn list_directory(&self, _path: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn set_workdir(&mut self, _path: &str) -> Result<()> {
        Ok(())
    }

    async fn export_directory(&self, _container_path: &str, _host_path: &str) -> Result<String> {
        Ok(String::new())
    }
}

pub struct Validator;

impl toolbox::Validator for Validator {
    async fn run(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {
        sandbox.exec("uv run main.py").await.map(|result| {
            if result.exit_code == 0 {
                Ok(())
            } else {
                Err(format!(
                    "code: {}\nstdout: {}\nstderr: {}",
                    result.exit_code, result.stdout, result.stderr
                ))
            }
        })
    }
}