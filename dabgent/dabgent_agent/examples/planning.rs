//! Example of using the simplified planning functions
//!
//! This example shows how to use planning with the existing Worker pattern,
//! following the same approach as basic.rs

use dabgent_agent::planner;
use dabgent_agent::toolbox::{self, basic::toolset};
use dabgent_mq::EventStore;
use dabgent_mq::db::sqlite::SqliteStore;
use dabgent_sandbox::dagger::Sandbox as DaggerSandbox;
use dabgent_sandbox::{Sandbox, SandboxDyn};
use eyre::Result;
use rig::client::ProviderClient;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    run().await;
}

async fn run() {
    dagger_sdk::connect(|client| async move {
        // Setup LLM
        let llm = rig::providers::anthropic::Client::from_env();
        
        // Setup sandbox
        let sandbox = sandbox(&client).await?;
        
        // Setup event store
        let store = store().await;
        
        // Setup tools
        let tools = toolset(Validator);
        
        // System prompt for the worker
        let preamble = "You are a helpful AI assistant that can plan and execute tasks.".to_string();
        
        // User request
        let user_input = "Create a Python script that fetches weather data for New York and saves it to a JSON file".to_string();
        
        // Run planning and execution using the simplified approach
        planner::runner::run(
            llm,
            store,
            preamble,
            tools,
            user_input,
        ).await?;
        
        Ok(())
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
        .build_opts(client.host().directory("./examples"), opts);
    ctr.sync().await?;
    let sandbox = DaggerSandbox::from_container(ctr);
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
        sandbox.exec("python main.py").await.map(|result| {
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
