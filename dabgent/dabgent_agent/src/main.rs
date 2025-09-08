<<<<<<< HEAD
fn main() {
    println!("Hello, world!");
}

pub trait Handler {
    type Command;
    type Event;

    fn process_command(&mut self, command: Self::Command) -> Result<Vec<Self::Event>, String>;
    fn fold(events: Vec<Self::Event>) -> Self;
}

pub enum TaskCommand {
    UserMessage(String),
    LLMAskTools,
    SandboxToolResult,
}

pub struct Workspace {
    pub files: std::collections::HashMap<String, String>,
}
=======
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
>>>>>>> main
