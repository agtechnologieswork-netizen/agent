use dabgent_agent::pipeline::PipelineBuilder;
use dabgent_agent::toolbox::basic::toolset;
use dabgent_agent::utils::PythonValidator;
use dabgent_mq::EventStore;
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::Sandbox;
use eyre::Result;
use rig::client::ProviderClient;

const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
const PYTHON_SYSTEM_PROMPT: &str = "You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.";

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

pub async fn run_pipeline(store: impl EventStore, stream_id: String) {
    const AGGREGATE_ID: &str = "thread";

    let opts = ConnectOpts::default();

    opts.connect(move |client| async move {
        let llm = rig::providers::anthropic::Client::from_env();
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

        // The pipeline will run continuously, processing events from the stream
        // It will only exit when a pipeline_shutdown event is received
        pipeline
            .run(stream_id.to_owned(), AGGREGATE_ID.to_owned())
            .await
    })
    .await
    .unwrap();
}
