use dabgent_agent::processor::builder::{self, ThreadConfig};
use dabgent_agent::toolbox::{self, basic::toolset};
use dabgent_mq::db::sqlite::SqliteStore;
use dabgent_mq::listener::PollingQueue;
use dabgent_sandbox::dagger::ConnectOpts;
use dabgent_sandbox::{DaggerSandbox, Sandbox, SandboxHandle};
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

    let llm = Arc::new(rig::providers::anthropic::Client::from_env());

    let opts = ConnectOpts::default();
    let sandbox_handle = SandboxHandle::new(opts).await?;

    let tools = toolset(Validator);
    let definitions = tools.iter().map(|tool| tool.definition()).collect();

    let (worker_handler, thread_handler, sandbox_handler) =
        builder::create_handlers(store.clone(), llm, sandbox_handle, tools);

    let queue = PollingQueue::new(store);
    let mut listeners = builder::spawn_listeners(
        worker_handler.clone(),
        thread_handler.clone(),
        sandbox_handler.clone(),
        queue,
    );

    let config = ThreadConfig {
        model: MODEL.to_string(),
        preamble: Some(SYSTEM_PROMPT.to_string()),
        tools: Some(definitions),
        ..Default::default()
    };

    builder::start_worker(
        &worker_handler,
        &thread_handler,
        &sandbox_handler,
        config,
        prompt,
        "./examples".to_string(),
        "Dockerfile".to_string(),
    )
    .await?;

    while let Some(result) = listeners.join_next().await {
        result??;
    }

    Ok(())
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
    async fn run(&self, sandbox: &mut DaggerSandbox) -> Result<Result<(), String>> {
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
