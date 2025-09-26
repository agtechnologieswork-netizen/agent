use dabgent_agent::pipeline::PipelineBuilder;
use dabgent_agent::pipeline_config::{
    PipelineConfig, create_python_toolset,
};
use dabgent_mq::EventStore;
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::Sandbox;
use eyre::Result;
use rig::client::ProviderClient;

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

    let config = PipelineConfig::for_cli();
    let opts = ConnectOpts::default();

    opts.connect(move |client| async move {
        let llm = rig::providers::anthropic::Client::from_env();
        let sandbox = create_dagger_sandbox(&client, &config.examples_path).await?;
        let tools = create_python_toolset();

        let pipeline = PipelineBuilder::new()
            .llm(llm)
            .store(store)
            .sandbox(sandbox.boxed())
            .model(config.model)
            .temperature(config.temperature)
            .max_tokens(config.max_tokens)
            .preamble(config.preamble)
            .recipient(config.recipient.unwrap_or_default())
            .tools(tools)
            .build()?;

        pipeline
            .run(stream_id.to_owned(), AGGREGATE_ID.to_owned())
            .await
    })
    .await
    .unwrap();
}
