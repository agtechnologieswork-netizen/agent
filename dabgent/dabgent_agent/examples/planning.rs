use dabgent_agent::planning::PlanningAgent;
use dabgent_mq::db::sqlite::SqliteStore;
use dabgent_sandbox::dagger::Sandbox as DaggerSandbox;
use dabgent_sandbox::Sandbox;
use eyre::Result;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    run().await;
}

async fn run() {
    dagger_sdk::connect(|client| async move {
        dotenvy::dotenv().ok();
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .expect("ANTHROPIC_API_KEY must be set in environment or .env file");
        let llm = rig::providers::anthropic::Client::new(api_key.as_str());
        let sandbox = sandbox(&client).await?;
        let store = store().await;
        
        let planning_agent = PlanningAgent::new(store.clone(), "example".to_string(), "demo".to_string());
        planning_agent.setup_workers(sandbox.boxed(), llm).await?;
        
        let agent = PlanningAgent::new(store.clone(), "example".to_string(), "demo".to_string());
        let task = "Implement a service that takes CSV file as input and produces Hypermedia API as output. Make sure to run it in such a way it does not block the agent while running (it will be run by uv run main.py command)";
        agent.process_message(task.to_string()).await?;
        agent.monitor_progress(|status| Box::pin(async move {
            tracing::info!("Status: {}", status);
            Ok(())
        })).await?;
        Ok(())
    }).await.unwrap();
}

async fn sandbox(client: &dagger_sdk::DaggerConn) -> Result<DaggerSandbox> {
    let opts = dagger_sdk::ContainerBuildOptsBuilder::default()
        .dockerfile("Dockerfile")
        .build()?;
    let ctr = client.container().build_opts(client.host().directory("./examples"), opts);
    ctr.sync().await?;
    Ok(DaggerSandbox::from_container(ctr))
}

async fn store() -> SqliteStore {
    let pool = sqlx::SqlitePool::connect(":memory:").await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}