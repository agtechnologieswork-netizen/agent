use dabgent_agent::event::Event as AgentEvent;
use dabgent_agent::processor::thread::{self, Thread};
use dabgent_agent::toolbox::{basic::toolset, planning::planning_toolset};
use dabgent_agent::Aggregate;
use dabgent_mq::{EventStore, Query};
use dabgent_mq::db::Metadata;
use rig::OneOrMany;
use rig::message::{Text, UserContent};
use std::sync::Arc;
use tokio::sync::Mutex;

/// CLI Emulator for testing without a real terminal
pub struct CliEmulator<S: EventStore> {
    store: S,
    thread: Thread,
    query: Query,
    history: Vec<AgentEvent>,
    output_buffer: Arc<Mutex<Vec<String>>>,
    stream_id: String,
}

impl<S: EventStore + Clone + Send + Sync + 'static> CliEmulator<S> {
    pub fn new(store: S, stream_id: String) -> color_eyre::Result<Self> {
        let query = Query {
            stream_id: stream_id.clone(),
            event_type: None,
            aggregate_id: Some("thread".to_owned()),
        };

        let thread = Thread::new();

        Ok(Self {
            store,
            thread,
            query,
            history: Vec::new(),
            output_buffer: Arc::new(Mutex::new(Vec::new())),
            stream_id,
        })
    }

    /// Setup thread with planning capabilities
    async fn setup_thread(&mut self) -> color_eyre::Result<()> {
        // Check if thread is already configured
        if self.thread.model.is_some() {
            return Ok(());
        }

        use crate::agent::Validator;

        // Combine basic tools with planning tools
        let mut tools = toolset(Validator);
        let planning_tools = planning_toolset(self.store.clone(), self.stream_id.clone());
        tools.extend(planning_tools);

        // Convert tools to definitions
        let tool_definitions: Vec<rig::completion::ToolDefinition> =
            tools.iter().map(|tool| tool.definition()).collect();

        // Send setup command with planning capabilities
        let setup_command = thread::Command::Setup {
            model: "claude-sonnet-4-20250514".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            preamble: Some(crate::agent::SYSTEM_PROMPT.to_string()),
            tools: Some(tool_definitions),
            recipient: Some("sandbox".to_string()),
        };

        let events = self.thread.process(setup_command)?;
        let metadata = Metadata::default();

        for event in events {
            self.store
                .push_event(
                    &self.query.stream_id,
                    self.query.aggregate_id.as_ref().unwrap(),
                    &event,
                    &metadata,
                )
                .await?;
        }
        Ok(())
    }

    async fn fold_thread(&mut self) -> color_eyre::Result<()> {
        let events = self.store.load_events::<AgentEvent>(&self.query, None).await?;
        self.thread = Thread::fold(&events);
        Ok(())
    }

    async fn send_message(&mut self, message: String) -> color_eyre::Result<()> {
        let user_content = UserContent::Text(Text { text: message });
        let command = thread::Command::User(OneOrMany::one(user_content));
        let events = self.thread.process(command)?;
        let metadata = Metadata::default();

        for event in events {
            self.store
                .push_event(
                    &self.query.stream_id,
                    self.query.aggregate_id.as_ref().unwrap(),
                    &event,
                    &metadata,
                )
                .await?;
        }
        Ok(())
    }

    /// Run a single command and get the response
    pub async fn run_command(
        mut self,
        command: String,
        timeout_secs: u64,
    ) -> color_eyre::Result<EmulatorResult> {
        // Setup thread
        self.setup_thread().await?;
        self.fold_thread().await?;

        // Send the command
        self.send_message(command.clone()).await?;

        // Wait for events with timeout
        let timeout = tokio::time::Duration::from_secs(timeout_secs);
        let start = tokio::time::Instant::now();

        let mut agent_responses = Vec::new();
        let mut plan_created = false;
        let mut tasks_created = Vec::new();

        // Subscribe to events
        let mut event_stream = self.store.subscribe::<AgentEvent>(&self.query)?;

        while start.elapsed() < timeout {
            let event_timeout = tokio::time::Duration::from_millis(500);
            match tokio::time::timeout(event_timeout, event_stream.next_full()).await {
                Ok(Some(Ok(event))) => {
                    match &event.data {
                        AgentEvent::PlanCreated { tasks } => {
                            plan_created = true;
                            tasks_created = tasks.clone();
                        }
                        AgentEvent::AgentMessage { response, .. } => {
                            // Extract text content
                            for content in response.choice.iter() {
                                if let rig::message::AssistantContent::Text(text) = content {
                                    agent_responses.push(text.text.clone());
                                }
                            }
                        }
                        _ => {}
                    }
                    self.history.push(event.data);
                }
                Ok(Some(Err(_))) => continue, // Skip errors
                Ok(None) => break,
                Err(_) => {
                    // Check if we have enough response
                    if !agent_responses.is_empty() || plan_created {
                        break;
                    }
                }
            }
        }

        Ok(EmulatorResult {
            command,
            responses: agent_responses,
            plan_created,
            tasks: tasks_created,
            events: self.history.clone(),
        })
    }
}

pub struct EmulatorResult {
    pub command: String,
    pub responses: Vec<String>,
    pub plan_created: bool,
    pub tasks: Vec<String>,
    pub events: Vec<AgentEvent>,
}

impl EmulatorResult {
    pub fn print_summary(&self) {
        println!("\n=== Emulator Result ===");
        println!("Command: {}", self.command);
        println!("Plan Created: {}", self.plan_created);
        if !self.tasks.is_empty() {
            println!("Tasks:");
            for (i, task) in self.tasks.iter().enumerate() {
                println!("  {}. {}", i + 1, task);
            }
        }
        if !self.responses.is_empty() {
            println!("Responses:");
            for response in &self.responses {
                println!("  {}", response);
            }
        }
        println!("Total Events: {}", self.events.len());
    }
}