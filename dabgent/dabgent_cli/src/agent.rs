use dabgent_agent::pipeline::PipelineBuilder;
use dabgent_agent::toolbox::{self, basic::toolset};
use dabgent_mq::EventStore;
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::{Sandbox, SandboxDyn};
use eyre::Result;
use rig::client::ProviderClient;

const SYSTEM_PROMPT: &str = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.
";

const MODEL: &str = "claude-sonnet-4-20250514";
const RECIPIENT: &str = "sandbox";
const DEFAULT_TEMPERATURE: f64 = 0.0;
const DEFAULT_MAX_TOKENS: u64 = 4_096;

pub async fn run_pipeline(store: impl EventStore, stream_id: String) {
    const AGGREGATE_ID: &str = "thread";

    let opts = ConnectOpts::default();
    opts.connect(|client| async move {
        let llm = rig::providers::anthropic::Client::from_env();
        let sandbox = sandbox(&client).await?;
        let tools = toolset(Validator);
        let pipeline = PipelineBuilder::new()
            .llm(llm)
            .store(store)
            .sandbox(sandbox.boxed())
            .model(MODEL.to_owned())
            .temperature(DEFAULT_TEMPERATURE)
            .max_tokens(DEFAULT_MAX_TOKENS)
            .preamble(SYSTEM_PROMPT.to_owned())
            .recipient(RECIPIENT.to_owned())
            .tools(tools)
            .build()?;

        pipeline
            .run(stream_id.to_owned(), AGGREGATE_ID.to_owned())
            .await
    })
    .await
    .unwrap();
}

async fn sandbox(client: &dagger_sdk::DaggerConn) -> Result<DaggerSandbox> {
    let opts = dagger_sdk::ContainerBuildOptsBuilder::default()
        .dockerfile("Dockerfile")
        .build()?;
    let ctr = client
        .container()
        .build_opts(client.host().directory("./dabgent_agent/examples"), opts);
    ctr.sync().await?;
    let sandbox = DaggerSandbox::from_container(ctr, client.clone());
    Ok(sandbox)
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
