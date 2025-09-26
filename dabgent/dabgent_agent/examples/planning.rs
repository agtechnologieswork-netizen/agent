use dabgent_agent::Aggregate;
use dabgent_agent::processor::{Pipeline, Processor, ThreadProcessor, ToolProcessor, thread};
use dabgent_agent::toolbox::planning::planning_toolset;
use dabgent_agent::toolbox::basic::toolset;
use dabgent_agent::utils::PythonValidator;
use dabgent_mq::{EventStore, db::sqlite::SqliteStore};
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::{NoOpSandbox, Sandbox};
use eyre::Result;
use rig::client::ProviderClient;

const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
const PYTHON_SYSTEM_PROMPT: &str = "You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.";

// Helper function for creating Dagger sandboxes
async fn create_dagger_sandbox(
    client: &dagger_sdk::DaggerConn,
    examples_path: &str,
) -> Result<DaggerSandbox> {
    let opts = dagger_sdk::ContainerBuildOptsBuilder::default()
        .dockerfile("Dockerfile")
        .build()?;
    let ctr = client
        .container()
        .build_opts(client.host().directory(examples_path), opts);
    ctr.sync().await?;
    let sandbox = DaggerSandbox::from_container(ctr, client.clone());
    Ok(sandbox)
}

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
    let prompt = "Create a hello world Python script that prints a greeting";

    let store = create_store().await;

    // Run the planning and execution pipeline
    planning_pipeline(STREAM_ID, store, prompt)
        .await
        .expect("Pipeline failed");
}

pub async fn planning_pipeline(
    stream_id: &str,
    store: impl EventStore + Clone,
    task: &str,
) -> Result<()> {
    let stream_id = stream_id.to_owned();
    let task = task.to_owned();

    let opts = ConnectOpts::default();
    opts.connect(|client| async move {

        let llm = rig::providers::anthropic::Client::from_env();

        // Phase 1: Planning

        let planning_config = dabgent_agent::event::Event::LLMConfig {
            model: DEFAULT_MODEL.to_string(),
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
            parent: None,
        };
        store
            .push_event(&stream_id, "planner", &planning_config, &Default::default())
            .await?;

        let user_message = dabgent_agent::event::Event::UserMessage(
            rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
                text: format!("Please create a plan for the following task: {}", task),
            }))
        );
        store
            .push_event(&stream_id, "planner", &user_message, &Default::default())
            .await?;

        let planning_sandbox = NoOpSandbox::new();
        let planning_tools = planning_toolset(store.clone(), stream_id.clone());

        let planning_thread = ThreadProcessor::new(llm.clone(), store.clone());
        let planning_tool_processor = ToolProcessor::new(
            planning_sandbox.boxed(),
            store.clone(),
            planning_tools,
            Some("planner".to_string()),
        );

        let planning_pipeline = Pipeline::new(
            store.clone(),
            vec![planning_thread.boxed(), planning_tool_processor.boxed()],
        );

        let pipeline_handle = tokio::spawn({
            let stream_id = stream_id.clone();
            async move {
                planning_pipeline.run(stream_id).await
            }
        });

        // Wait for PlanCreated event
        let mut plan_created = false;
        let mut feedback_sent = false;
        while !plan_created {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let query = dabgent_mq::Query::stream(&stream_id).aggregate("planner");
            let events = store.load_events::<dabgent_agent::event::Event>(&query, None).await?;

            for event in events.iter() {
                match event {
                    dabgent_agent::event::Event::PlanCreated { tasks } => {
                        println!("PlanCreated event detected with {} tasks", tasks.len());
                        plan_created = true;
                        break;
                    }
                    dabgent_agent::event::Event::UserInputRequested { prompt, context }
                        if !feedback_sent =>
                    {
                        println!("Planner requested feedback: {prompt}");
                        if let Some(context) = context {
                            if let Ok(pretty) = serde_json::to_string_pretty(context) {
                                println!("Context: {pretty}");
                            }
                        }

                        send_planner_feedback(
                            &store,
                            &stream_id,
                            "Looks good, please proceed with the execution plan.",
                        )
                        .await?;
                        feedback_sent = true;
                    }
                    _ => {}
                }
            }
        }

        // Stop the planning pipeline
        pipeline_handle.abort();

        // Phase 2: Execution

        let query = dabgent_mq::Query::stream(&stream_id).aggregate("planner");
        let events = store.load_events::<dabgent_agent::event::Event>(&query, None).await?;

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

            let execution_sandbox = create_dagger_sandbox(&client, "./examples").await?;
            let execution_tools = toolset(PythonValidator);

            let execution_thread = ThreadProcessor::new(llm.clone(), store.clone());
            let execution_tool_processor = ToolProcessor::new(
                execution_sandbox.boxed(),
                store.clone(),
                execution_tools,
                None,
            );

            let execution_pipeline = Pipeline::new(
                store.clone(),
                vec![execution_thread.boxed(), execution_tool_processor.boxed()],
            );

            let exec_handle = tokio::spawn({
                let stream_id = stream_id.clone();
                async move {
                    execution_pipeline.run(stream_id).await
                }
            });

            for (i, task_desc) in tasks.iter().enumerate() {
                let thread_id = format!("task-{}", i);

                let worker_config = dabgent_agent::event::Event::LLMConfig {
                    model: DEFAULT_MODEL.to_string(),
                    temperature: 0.7,
                    max_tokens: 4096,
                    preamble: Some(PYTHON_SYSTEM_PROMPT.to_string()),
                    tools: Some(
                        toolset(PythonValidator)
                            .iter()
                            .map(|tool| tool.definition())
                            .collect()
                    ),
                    recipient: None,
                    parent: None,
                };
                store
                    .push_event(&stream_id, &thread_id, &worker_config, &Default::default())
                    .await?;

                let task_message = dabgent_agent::event::Event::UserMessage(
                    rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
                        text: format!("{}\nWhen complete, call the 'done' tool to mark this task as finished.", task_desc),
                    }))
                );
                store
                    .push_event(&stream_id, &thread_id, &task_message, &Default::default())
                    .await?;

                // Wait for task completion
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                    let query = dabgent_mq::Query::stream(&stream_id);
                    let events = store.load_events::<dabgent_agent::event::Event>(&query, None).await?;

                    let completed_count = events.iter()
                        .filter(|e| matches!(e, dabgent_agent::event::Event::TaskCompleted { .. }))
                        .count();

                    if completed_count > i {
                        break;
                    }
                }
            }

            exec_handle.abort();

        }

        Ok(())
    })
    .await
    .map_err(Into::into)
}

async fn send_planner_feedback<S: EventStore>(
    store: &S,
    stream_id: &str,
    feedback: &str,
) -> Result<()> {
    let query = dabgent_mq::Query::stream(stream_id).aggregate("planner");
    let events = store
        .load_events::<dabgent_agent::event::Event>(&query, None)
        .await?;
    let mut planner_thread = thread::Thread::fold(&events);

    let message = rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
        text: feedback.to_string(),
    }));

    let new_events = planner_thread
        .process(thread::Command::User(message))
        .map_err(|err: thread::Error| eyre::eyre!(err))?;

    for event in new_events {
        store
            .push_event(stream_id, "planner", &event, &Default::default())
            .await?;
    }

    Ok(())
}

async fn create_store() -> SqliteStore {
    let pool = sqlx::SqlitePool::connect(":memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}

