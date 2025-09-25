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

    let result = emulator.run_command(
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

    let result = emulator.run_command(
        "Create a plan to build a Python script that analyzes CSV data".to_string(),
        10
    ).await?;

    result.print_summary();
    println!();

    // Test 3: Complex task with planning
    println!("Test 3: Complex task with planning");
    let stream_id = format!("{}_test3", Uuid::now_v7());
    let emulator = CliEmulator::new(store.clone(), stream_id)?;

    let result = emulator.run_command(
        "Build a web scraper that extracts data from a website and saves it to CSV".to_string(),
        15
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

    println!("\n=== All Tests Complete ===");
    Ok(())
}