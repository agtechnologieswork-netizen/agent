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

Example format:
- Create the main file
- Add necessary imports
- Implement core functionality
- Add error handling
- Test the implementation
";

#[tokio::main]
async fn main() {
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

    // Thread settings for execution
    let settings = ThreadSettings::new(MODEL, 0.7, 4096);

    let opts = ConnectOpts::default();
    opts.connect(|client| async move {
        let llm = rig::providers::anthropic::Client::from_env();

        println!("=== PLANNING PHASE ===");
        println!("Task: {}\n", task);

        // Phase 1: Create the plan using planning tools
        {
            let planning_sandbox = sandbox(&client).await?;
            let planning_tools = planning_toolset(
                planner.clone(),
                store.clone(),
                stream_id.clone(),
                settings.clone(),
            );

            // Create planning thread processor with planning-specific prompt
            let planning_thread = ThreadProcessor::new(
                llm.clone(),
                store.clone(),
            );

            // Create tool processor for planning tools
            let planning_tool_processor = ToolProcessor::new(
                planning_sandbox.boxed(),
                store.clone(),
                planning_tools,
                Some("planner".to_string()),
            );

            // Run planning pipeline
            let planning_pipeline = Pipeline::new(
                store.clone(),
                vec![planning_thread.boxed(), planning_tool_processor.boxed()],
            );

            // Push initial planning configuration
            let config_event = dabgent_agent::event::Event::LLMConfig {
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
                .push_event(&stream_id, "planner", &config_event, &Default::default())
                .await?;

            // Push the task as a user message
            let user_message = dabgent_agent::event::Event::UserMessage(
                rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
                    text: format!("Please create a plan for the following task: {}", task),
                }))
            );
            store
                .push_event(&stream_id, "planner", &user_message, &Default::default())
                .await?;

            // Run the planning pipeline
            planning_pipeline.run(stream_id.clone()).await?;
        }

        println!("\n=== EXECUTION PHASE ===");

        // Phase 2: Execute each task in the plan
        if let Some(planner) = planner.lock().unwrap().take() {
            let tasks = planner.tasks().to_vec();
            println!("Executing {} tasks:\n", tasks.len());

            for (i, task) in tasks.iter().enumerate() {
                println!("Task {}/{}: {}", i + 1, tasks.len(), task.description);

                // Create execution sandbox for this task
                let execution_sandbox = sandbox(&client).await?;
                let execution_tools = toolset(Validator);

                // Create thread processor for execution with system prompt
                let execution_thread = ThreadProcessor::new(
                    llm.clone(),
                    store.clone(),
                );

                // Create tool processor for execution tools
                let execution_tool_processor = ToolProcessor::new(
                    execution_sandbox.boxed(),
                    store.clone(),
                    execution_tools,
                    None,
                );

                // Run execution pipeline
                let execution_pipeline = Pipeline::new(
                    store.clone(),
                    vec![execution_thread.boxed(), execution_tool_processor.boxed()],
                );

                // Push configuration for this task's thread
                let config_event = dabgent_agent::event::Event::LLMConfig {
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
                    recipient: Some(task.thread().to_string()),
                };
                store
                    .push_event(&stream_id, task.thread(), &config_event, &Default::default())
                    .await?;

                // Push the task description as a user message
                let task_message = dabgent_agent::event::Event::UserMessage(
                    rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
                        text: task.description.clone(),
                    }))
                );
                store
                    .push_event(&stream_id, task.thread(), &task_message, &Default::default())
                    .await?;

                // Run the execution pipeline for this task
                execution_pipeline.run(stream_id.clone()).await?;

                println!("  ✓ Completed\n");
            }

            println!("All tasks completed successfully!");
        } else {
            println!("No plan was created.");
        }

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