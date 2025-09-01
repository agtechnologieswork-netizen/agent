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
