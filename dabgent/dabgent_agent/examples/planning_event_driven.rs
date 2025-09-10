//! Event-driven planning example demonstrating proper decoupling
//! 
//! This example shows how the planner and executor communicate
//! exclusively through the event store, with no direct coupling.

use dabgent_agent::planner;
use dabgent_mq::db::sqlite::SqliteStore;
use eyre::Result;
use rig::client::ProviderClient;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().init();

    // Initialize LLM client
    let llm = rig::providers::anthropic::Client::from_env();

    // Initialize event store (in-memory for example)
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await?;
    sqlx::migrate!("../dabgent_mq/migrations/sqlite").run(&pool).await?;
    let store = SqliteStore::new(pool);

    // Example input
    let input = "Create a Python script that fetches weather data for New York and saves it to a JSON file";

    println!("🎯 Starting event-driven planning for: {}", input);
    println!("{}", "─".repeat(80));
    println!("📡 Using event store for all planner-executor communication");
    println!();

    // Run with event-driven architecture
    planner::event_runner::run_with_timeout(
        llm,
        store,
        "You are a helpful AI assistant".to_string(),
        vec![],
        input.to_string(),
        30, // 30 second timeout
    ).await?;

    Ok(())
}
