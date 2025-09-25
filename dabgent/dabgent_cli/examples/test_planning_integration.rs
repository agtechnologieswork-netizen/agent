use dabgent_cli::PlanningAgent;
use dabgent_mq::db::sqlite::SqliteStore;
use sqlx::SqlitePool;
use uuid::Uuid;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    println!("=== Planning Integration Test ===\n");

    // Create store
    let pool = SqlitePool::connect(":memory:").await?;
    let store = SqliteStore::new(pool);
    store.migrate().await;

    // Test different planning scenarios
    test_plan_creation(store.clone()).await?;
    test_complex_plan(store.clone()).await?;

    println!("\n=== All Tests Complete ===");
    Ok(())
}

async fn test_plan_creation(store: SqliteStore) -> color_eyre::Result<()> {
    println!("\nTest: Simple Plan Creation");
    println!("{}", "=".repeat(40));

    let stream_id = format!("{}_simple", Uuid::now_v7());
    let planning_agent = PlanningAgent::new(store, stream_id);

    let task = "Write a function to calculate factorial";
    println!("Task: {}", task);

    let tasks = planning_agent.create_plan(task).await?;

    if tasks.is_empty() {
        println!("✗ No tasks created");
    } else {
        println!("✓ Plan created with {} tasks:", tasks.len());
        for (i, task) in tasks.iter().enumerate() {
            println!("  {}. {}", i + 1, task);
        }
    }

    Ok(())
}

async fn test_complex_plan(store: SqliteStore) -> color_eyre::Result<()> {
    println!("\nTest: Complex Plan Creation");
    println!("{}", "=".repeat(40));

    let stream_id = format!("{}_complex", Uuid::now_v7());
    let planning_agent = PlanningAgent::new(store, stream_id);

    let task = "Build a REST API with FastAPI that manages a todo list with CRUD operations";
    println!("Task: {}", task);

    let tasks = planning_agent.create_plan(task).await?;

    if tasks.is_empty() {
        println!("✗ No tasks created");
    } else {
        println!("✓ Plan created with {} tasks:", tasks.len());
        for (i, task) in tasks.iter().take(5).enumerate() {
            println!("  {}. {}", i + 1, task);
        }
        if tasks.len() > 5 {
            println!("  ... and {} more tasks", tasks.len() - 5);
        }
    }

    Ok(())
}