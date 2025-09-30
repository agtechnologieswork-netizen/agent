use dabgent_agent::processor::builder::{self, ThreadConfig};
use dabgent_agent::toolbox::{self, basic::toolset};
use dabgent_mq::db::sqlite::SqliteStore;
use dabgent_mq::listener::PollingQueue;
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::{Sandbox, SandboxDyn};
use eyre::Result;
use rig::client::ProviderClient;
use std::sync::Arc;

const MODEL: &str = "claude-sonnet-4-20250514";

const SYSTEM_PROMPT: &str = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.
";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let prompt = "minimal script that fetches my ip using some api like ipify.org";

    let store = store().await;
    run_worker(store, prompt).await.unwrap();
}

pub async fn run_worker(store: SqliteStore, prompt: &str) -> Result<()> {
    let prompt = prompt.to_string();
    let opts = ConnectOpts::default();
    opts.connect(move |client| async move {
        tracing::info!("initializing infrastructure");
        let llm = Arc::new(rig::providers::anthropic::Client::from_env());
        let sandbox = sandbox(&client).await?;
        tracing::info!("infrastructure initialized");
        let tools = toolset(Validator);
        let definitions = tools.iter().map(|tool| tool.definition()).collect();

        let (worker_handler, thread_handler, sandbox_handler) =
            builder::create_handlers(store.clone(), llm, sandbox.boxed(), tools);

        let queue = PollingQueue::new(store);
        let mut listeners = builder::spawn_listeners(
            worker_handler.clone(),
            thread_handler.clone(),
            sandbox_handler,
            queue,
        );

        let config = ThreadConfig {
            model: MODEL.to_string(),
            preamble: Some(SYSTEM_PROMPT.to_string()),
            tools: Some(definitions),
            ..Default::default()
        };

        builder::start_worker(&worker_handler, &thread_handler, config, prompt).await?;

        while let Some(result) = listeners.join_next().await {
            result??;
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
    let store = SqliteStore::new(pool, "agent");
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
