use dabgent_agent::processor::{Pipeline, Processor, ThreadProcessor, ToolProcessor};
use dabgent_agent::pipeline_config::{
    PipelineConfig, create_python_toolset
};
use dabgent_agent::examples_utils::push_prompt;
use dabgent_mq::{EventStore, db::sqlite::SqliteStore};
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::Sandbox;
use eyre::Result;
use rig::client::ProviderClient;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    const STREAM_ID: &str = "pipeline";
    let prompt = "minimal script that fetches my ip using some api like ipify.org";

    let store = create_store().await;
    push_prompt(&store, STREAM_ID, "", prompt).await.unwrap();
    pipeline_fn(STREAM_ID, store).await.unwrap();
}

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

pub async fn pipeline_fn(stream_id: &str, store: impl EventStore) -> Result<()> {
    let stream_id = stream_id.to_owned();
    let config = PipelineConfig::for_examples();
    let opts = ConnectOpts::default();

    opts.connect(|client| async move {
        let llm = rig::providers::anthropic::Client::from_env();
        let sandbox = create_dagger_sandbox(&client, &config.examples_path).await?;
        let tools = create_python_toolset();

        let thread_processor = ThreadProcessor::new(
            llm.clone(),
            store.clone(),
        );
        let tool_processor = ToolProcessor::new(sandbox.boxed(), store.clone(), tools, None);
        let pipeline = Pipeline::new(
            store.clone(),
            vec![thread_processor.boxed(), tool_processor.boxed()],
        );
        pipeline.run(stream_id.clone()).await?;
        Ok(())
    })
    .await
    .map_err(Into::into)
}

async fn create_store() -> SqliteStore {
    let pool = sqlx::SqlitePool::connect(":memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}