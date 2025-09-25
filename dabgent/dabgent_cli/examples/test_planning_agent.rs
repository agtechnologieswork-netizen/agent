use dabgent_cli::agent::PlanningAgent;
use dabgent_mq::db::sqlite::SqliteStore;
use sqlx::SqlitePool;
use uuid::Uuid;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    dotenvy::dotenv().ok();

    println!("=== Planning Agent Test ===\n");

    // Create store
    let pool = SqlitePool::connect(":memory:").await?;
    let store = SqliteStore::new(pool);
    store.migrate().await;

    let stream_id = format!("{}_planning_test", Uuid::now_v7());

    // Create planning agent
    let planning_agent = PlanningAgent::new(store, stream_id);

    // Test 1: Create a simple plan
    println!("Test 1: Creating a plan for a simple task");
    let task = "Create a Python script that prints hello world";

    match planning_agent.create_plan(task).await {
        Ok(tasks) => {
            println!("✓ Plan created successfully with {} tasks:", tasks.len());
            for (i, task) in tasks.iter().enumerate() {
                println!("  {}. {}", i + 1, task);
            }
        }
        Err(e) => {
            println!("✗ Failed to create plan: {}", e);
        }
    }

    println!("\n=== Test Complete ===");
    Ok(())
}