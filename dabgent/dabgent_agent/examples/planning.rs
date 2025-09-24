use dabgent_agent::processor::{Pipeline, Processor, ThreadProcessor, ToolProcessor};
use dabgent_agent::toolbox::{self, basic::toolset, planning::planning_toolset};
use dabgent_mq::db::sqlite::SqliteStore;
use dabgent_mq::EventStore;
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::{Sandbox, SandboxDyn};
use eyre::Result;
use rig::client::ProviderClient;

const MODEL: &str = "claude-sonnet-4-20250514";

const SYSTEM_PROMPT: &str = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.
";

const PLANNING_PROMPT: &str = "
You are a planning assistant that breaks down complex tasks into actionable steps.

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
    planning_pipeline(STREAM_ID, store, prompt)
        .await
        .expect("Pipeline failed");

    println!("\n✨ Planning example completed!");
}

pub async fn planning_pipeline(stream_id: &str, store: impl EventStore + Clone, task: &str) -> Result<()> {
    let stream_id = stream_id.to_owned();
    let task = task.to_owned();

    let opts = ConnectOpts::default();
    opts.connect(|client| async move {
        println!("=== EVENT-DRIVEN PLANNING PIPELINE ===\n");
        println!("Task: {}\n", task);

        let llm = rig::providers::anthropic::Client::from_env();

        // === PHASE 1: PLANNING ===
        println!("=== PHASE 1: PLANNING ===");

        // Configure and run the planning agent
        let planning_config = dabgent_agent::event::Event::LLMConfig {
            model: MODEL.to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            preamble: Some(PLANNING_PROMPT.to_string()),
            tools: Some(
                planning_toolset(store.clone(), stream_id.clone())
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

        // Create planning pipeline with tools
        let planning_sandbox = DummySandbox::new();
        let planning_tools = planning_toolset(store.clone(), stream_id.clone());

        let planning_thread = ThreadProcessor::new(llm.clone(), store.clone());
        let planning_tool_processor = ToolProcessor::new(
            planning_sandbox.boxed(),
            store.clone(),
            planning_tools,
            Some("planner".to_string()),  // Only process planner messages
        );

        let planning_pipeline = Pipeline::new(
            store.clone(),
            vec![planning_thread.boxed(), planning_tool_processor.boxed()],
        );

        // Run planning pipeline briefly
        println!("Creating plan...");
        let pipeline_handle = tokio::spawn({
            let stream_id = stream_id.clone();
            async move {
                planning_pipeline.run(stream_id).await
            }
        });

        // Wait for plan to be created
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        pipeline_handle.abort();

        // === PHASE 2: EXECUTION ===
        println!("\n=== PHASE 2: EXECUTION ===");

        // Load events to get the plan
        let query = dabgent_mq::Query::stream(&stream_id).aggregate("planner");
        let events = store.load_events::<dabgent_agent::event::Event>(&query, None).await?;

        // Find the most recent plan
        let mut plan_tasks: Option<Vec<String>> = None;
        for event in events.iter() {
            match event {
                dabgent_agent::event::Event::PlanCreated { tasks } |
                dabgent_agent::event::Event::PlanUpdated { tasks } => {
                    plan_tasks = Some(tasks.clone());
                }
                _ => {}
            }
        }

        if let Some(tasks) = plan_tasks {
            println!("Executing {} tasks:\n", tasks.len());

            // Create sandbox for execution
            let execution_sandbox = sandbox(&client).await?;
            let execution_tools = toolset(Validator);

            // Configure each task thread and dispatch
            for (i, task_desc) in tasks.iter().enumerate() {
                let thread_id = format!("task-{}", i);
                println!("Task {}/{}: {}", i + 1, tasks.len(), task_desc);
                println!("  Thread: {}", thread_id);

                // Configure the worker thread
                let worker_config = dabgent_agent::event::Event::LLMConfig {
                    model: MODEL.to_string(),
                    temperature: 0.7,
                    max_tokens: 4096,
                    preamble: Some(SYSTEM_PROMPT.to_string()),
                    tools: Some(
                        execution_tools
                            .iter()
                            .map(|tool| tool.definition())
                            .collect()
                    ),
                    recipient: Some(thread_id.to_string()),
                };
                store
                    .push_event(&stream_id, &thread_id, &worker_config, &Default::default())
                    .await?;

                // Send task to worker
                let task_message = dabgent_agent::event::Event::UserMessage(
                    rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
                        text: task_desc.clone(),
                    }))
                );
                store
                    .push_event(&stream_id, &thread_id, &task_message, &Default::default())
                    .await?;

                println!("  ✓ Dispatched to worker\n");
            }

            // Create execution pipeline
            let execution_thread = ThreadProcessor::new(llm.clone(), store.clone());
            let execution_tool_processor = ToolProcessor::new(
                execution_sandbox.boxed(),
                store.clone(),
                execution_tools,
                None,  // Process all non-planner messages
            );

            let execution_pipeline = Pipeline::new(
                store.clone(),
                vec![execution_thread.boxed(), execution_tool_processor.boxed()],
            );

            // Run execution briefly
            println!("Executing tasks...");
            let exec_handle = tokio::spawn({
                let stream_id = stream_id.clone();
                async move {
                    execution_pipeline.run(stream_id).await
                }
            });

            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            exec_handle.abort();
        } else {
            println!("No plan was created.");
        }

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

    async fn fork(&self) -> Result<DummySandbox> {
        Ok(DummySandbox)
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