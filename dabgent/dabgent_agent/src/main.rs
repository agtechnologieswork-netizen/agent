use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("Event-driven Agent Architecture");
    println!("================================");
    println!();
    println!("This demonstrates a simplified event-driven architecture:");
    println!("1. Orchestrator launches workers");
    println!("2. LLM worker subscribes to events and generates completions");
    println!("3. Sandbox worker subscribes to events and executes tools");
    println!("4. All communication happens through the event bus");
    println!();
    println!("Key benefits:");
    println!("- No polymorphism or async traits");
    println!("- Native Rust futures with impl Future");
    println!("- Pure event-driven communication");
    println!("- Clean separation of concerns");
    
    Ok(())
}