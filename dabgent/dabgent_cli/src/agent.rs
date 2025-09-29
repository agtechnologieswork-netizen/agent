use dabgent_agent::llm::LLMClient;
use dabgent_agent::pipeline::PipelineBuilder;
use dabgent_agent::planning_mode::{create_planner_pipeline, create_executor_pipeline, monitor_plan_execution, ModelConfig, PLANNING_PROMPT};
use dabgent_agent::toolbox::basic::toolset;
use dabgent_agent::toolbox::planning::planning_toolset;
use dabgent_agent::utils::PythonValidator;
use dabgent_mq::EventStore;
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::Sandbox;
use eyre::Result;
use rig::client::ProviderClient;

const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
const PYTHON_SYSTEM_PROMPT: &str = "You are a python software engineer executing a single specific task.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.

CRITICAL: When you complete the assigned task, you MUST call the 'done' tool immediately.
Do not continue with additional tasks - only complete the one specific task given to you.";

async fn create_dagger_sandbox(
    client: &dagger_sdk::DaggerConn,
    examples_path: &str,
) -> Result<DaggerSandbox> {
    tracing::info!("Creating Dagger sandbox from {}/Dockerfile", examples_path);
    let opts = dagger_sdk::ContainerBuildOptsBuilder::default()
        .dockerfile("Dockerfile")
        .build()?;
    let ctr = client
        .container()
        .build_opts(client.host().directory(examples_path), opts);
    tracing::info!("Syncing container...");
    ctr.sync().await?;
    let sandbox = DaggerSandbox::from_container(ctr, client.clone());
    tracing::info!("Dagger sandbox created successfully");
    Ok(sandbox)
}

pub async fn run_pipeline(store: impl EventStore, stream_id: String, planning: bool) {
    let opts = ConnectOpts::default();

    opts.connect(move |client| async move {
        let llm = rig::providers::anthropic::Client::from_env();

        if planning {
            run_planning_mode(store, stream_id, llm, &client).await
        } else {
            run_standard_mode(store, stream_id, llm, &client).await
        }
    })
    .await
    .unwrap();
}

async fn run_standard_mode<C: LLMClient + 'static>(
    store: impl EventStore,
    stream_id: String,
    llm: C,
    client: &dagger_sdk::DaggerConn,
) -> Result<()> {
    let sandbox = create_dagger_sandbox(&client, "./examples").await?;
    let tools = toolset(PythonValidator);

    let pipeline = PipelineBuilder::new()
        .llm(llm)
        .store(store)
        .sandbox(sandbox.boxed())
        .model(DEFAULT_MODEL.to_string())
        .temperature(0.0)
        .max_tokens(4096)
        .preamble(PYTHON_SYSTEM_PROMPT.to_string())
        .recipient("sandbox".to_string())
        .tools(tools)
        .build()?;

    pipeline
        .run(stream_id.to_owned(), "thread".to_owned())
        .await
}

async fn run_planning_mode<C: LLMClient + 'static>(
    store: impl EventStore + Clone,
    stream_id: String,
    llm: C,
    client: &dagger_sdk::DaggerConn,
) -> Result<()> {
    // Set up the planner configuration first
    // This needs to be done before the user sends messages
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

    // Create two separate pipelines using extracted functions
    let planner_pipeline = create_planner_pipeline(llm.clone(), store.clone(), stream_id.clone());

    // Create executor pipeline with sandbox and tools
    let execution_sandbox = create_dagger_sandbox(client, "./examples").await?;
    let execution_tools = toolset(PythonValidator);
    let executor_pipeline = create_executor_pipeline(
        llm.clone(),
        store.clone(),
        execution_sandbox,
        execution_tools
    );

    // Set up monitor task
    let monitor_store = store.clone();
    let monitor_stream_id = stream_id.clone();
    let model_config = ModelConfig {
        model: DEFAULT_MODEL.to_string(),
        temperature: 0.7,
        max_tokens: 4096,
        preamble: PYTHON_SYSTEM_PROMPT.to_string(),
    };
    tokio::spawn(async move {
        tracing::info!("Monitor task started");
        if let Err(e) = monitor_plan_execution(monitor_store, monitor_stream_id, model_config).await {
            tracing::error!("Monitor task failed: {:?}", e);
        }
    });

    // Run both pipelines concurrently
    let planner_stream_id = stream_id.clone();
    let executor_stream_id = stream_id.clone();

    let planner_handle = tokio::spawn(async move {
        tracing::info!("Starting planner pipeline");
        let result = planner_pipeline.run(planner_stream_id).await;
        tracing::info!("Planner pipeline finished with result: {:?}", result.is_ok());
        result
    });
    let executor_handle = tokio::spawn(async move {
        tracing::info!("Starting executor pipeline");
        let result = executor_pipeline.run(executor_stream_id).await;
        tracing::info!("Executor pipeline finished with result: {:?}", result.is_ok());
        result
    });

    // Use try_join to wait for both pipelines without terminating early
    // This ensures both pipelines keep running even if one encounters an error
    match tokio::try_join!(planner_handle, executor_handle) {
        Ok((planner_result, executor_result)) => {
            if let Err(e) = planner_result {
                tracing::error!("Planner pipeline error: {:?}", e);
            } else {
                tracing::info!("Planner pipeline completed successfully");
            }
            if let Err(e) = executor_result {
                tracing::error!("Executor pipeline error: {:?}", e);
            } else {
                tracing::info!("Executor pipeline completed successfully");
            }
        }
        Err(e) => {
            tracing::error!("Pipeline join error: {:?}", e);
            // Even if one task panicked, try to gracefully handle it
            return Err(eyre::eyre!("Pipeline execution failed: {}", e));
        }
    }

    Ok(())
}
