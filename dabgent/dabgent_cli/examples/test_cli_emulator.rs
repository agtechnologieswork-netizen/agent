use dabgent_cli::CliEmulator;
use dabgent_mq::db::sqlite::SqliteStore;
use sqlx::SqlitePool;
use uuid::Uuid;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    println!("=== CLI Emulator Test ===\n");

    // Create store
    let pool = SqlitePool::connect(":memory:").await?;
    let store = SqliteStore::new(pool);
    store.migrate().await;

    // Test 1: Simple greeting
    println!("Test 1: Simple greeting");
    let stream_id = format!("{}_test1", Uuid::now_v7());
    let emulator = CliEmulator::new(store.clone(), stream_id)?;

    let result = emulator.run_command_with_pipeline(
        "Hello, can you help me?".to_string(),
        5
    ).await?;

    println!("Response received: {}", !result.responses.is_empty());
    for response in &result.responses {
        println!("  > {}", response);
    }
    println!();

    // Test 2: Planning request
    println!("Test 2: Planning request");
    let stream_id = format!("{}_test2", Uuid::now_v7());
    let emulator = CliEmulator::new(store.clone(), stream_id)?;

    let result = emulator.run_command_with_pipeline(
        "Use the create_plan tool to break down this task: Build a Python script that analyzes CSV data".to_string(),
        10
    ).await?;

    result.print_summary();
    println!();

    // Test 3: Complex task with planning
    println!("Test 3: Complex task with planning");
    let stream_id = format!("{}_test3", Uuid::now_v7());
    let emulator = CliEmulator::new(store.clone(), stream_id)?;

    let result = emulator.run_command_with_pipeline(
        "Please use the create_plan tool to create a detailed plan for this task: Build a web scraper that extracts data from a website and saves it to CSV format".to_string(),
        20  // Increased timeout to wait for PlanCreated events
    ).await?;

    result.print_summary();

    // Check if planning tools were used
    let planning_events = result.events.iter().filter(|e| {
        matches!(e,
            dabgent_agent::event::Event::PlanCreated { .. } |
            dabgent_agent::event::Event::PlanUpdated { .. }
        )
    }).count();

    println!("\nPlanning events found: {}", planning_events);

    // Debug: show all event types
    println!("\nDEBUG: All events in test 3:");
    for event in &result.events {
        match event {
            dabgent_agent::event::Event::PlanCreated { tasks } => {
                println!("  - PlanCreated with {} tasks", tasks.len());
            }
            dabgent_agent::event::Event::ToolResult(results) => {
                println!("  - ToolResult with {} results", results.len());
            }
            _ => {}
        }
    }

    println!("\n=== All Tests Complete ===");
    Ok(())
}