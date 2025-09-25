use dabgent_cli::PlanningAgent;
use dabgent_mq::db::sqlite::SqliteStore;
use sqlx::SqlitePool;
use uuid::Uuid;

/// Test CLI planning mode without Docker execution
#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    dotenvy::dotenv().ok();

    println!("=== CLI Planning Mode Test ===\n");

    // Create store
    let pool = SqlitePool::connect(":memory:").await?;
    let store = SqliteStore::new(pool);
    store.migrate().await;

    let stream_id = format!("{}_cli_planning", Uuid::now_v7());

    // Test various tasks
    let tasks = vec![
        "Create a simple hello world script",
        "Build a function to check if a number is prime",
        "Create a REST API endpoint that returns the current time",
    ];

    for (idx, task) in tasks.iter().enumerate() {
        println!("\n{}", "=".repeat(60));
        println!("Task: {}", task);
        println!("{}", "=".repeat(60));

        // Create a new planning agent for each task with unique stream
        let task_stream_id = format!("{}_task_{}", stream_id, idx);
        let task_agent = PlanningAgent::new(store.clone(), task_stream_id);

        match task_agent.create_plan(task).await {
            Ok(plan_tasks) => {
                if plan_tasks.is_empty() {
                    println!("❌ No plan created");
                } else {
                    println!("✅ Plan created with {} steps:", plan_tasks.len());
                    for (i, step) in plan_tasks.iter().enumerate() {
                        println!("   {}. {}", i + 1, step);
                    }
                }
            }
            Err(e) => {
                println!("❌ Error creating plan: {}", e);
            }
        }
    }

    println!("\n=== Test Complete ===");
    Ok(())
}