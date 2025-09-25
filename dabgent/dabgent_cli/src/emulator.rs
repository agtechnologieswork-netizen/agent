use dabgent_agent::event::Event as AgentEvent;
use dabgent_agent::processor::{thread::{self, Thread}, Pipeline, Processor, ThreadProcessor, ToolProcessor};
use dabgent_agent::toolbox::{basic::toolset, planning::planning_toolset};
use dabgent_agent::Aggregate;
use dabgent_mq::{EventStore, Query};
use dabgent_mq::db::Metadata;
use dabgent_sandbox::{Sandbox, SandboxDyn};
use eyre::Result;
use rig::OneOrMany;
use rig::client::ProviderClient;
use rig::message::{Text, UserContent};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Dummy sandbox for testing (planning tools don't need actual execution)
struct DummySandbox;

impl DummySandbox {
    fn new() -> Self {
        Self
    }
}

impl Sandbox for DummySandbox {
    async fn exec(&mut self, _command: &str) -> Result<dabgent_sandbox::ExecResult> {
        Ok(dabgent_sandbox::ExecResult {
            exit_code: 0,
            stdout: String::from("Command executed successfully"),
            stderr: String::new(),
        })
    }

    async fn write_file(&mut self, _path: &str, _content: &str) -> Result<()> {
        Ok(())
    }

    async fn write_files(&mut self, _files: Vec<(&str, &str)>) -> Result<()> {
        Ok(())
    }

    async fn read_file(&self, _path: &str) -> Result<String> {
        Ok(String::from("File content"))
    }

    async fn delete_file(&mut self, _path: &str) -> Result<()> {
        Ok(())
    }

    async fn list_directory(&self, _path: &str) -> Result<Vec<String>> {
        Ok(vec!["file1.txt".to_string(), "file2.py".to_string()])
    }

    async fn set_workdir(&mut self, _path: &str) -> Result<()> {
        Ok(())
    }

    async fn export_directory(&self, _container_path: &str, _host_path: &str) -> Result<String> {
        Ok(String::new())
    }

    async fn fork(&self) -> Result<DummySandbox> {
        Ok(DummySandbox)
    }
}

/// CLI Emulator for testing without a real terminal
pub struct CliEmulator<S: EventStore> {
    store: S,
    thread: Thread,
    query: Query,
    history: Vec<AgentEvent>,
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

    /// Run a single command and get the response with pipeline processors
    pub async fn run_command_with_pipeline(
        mut self,
        command: String,
        timeout_secs: u64,
    ) -> color_eyre::Result<EmulatorResult> {
        // Setup thread
        self.setup_thread().await?;
        self.fold_thread().await?;

        // Create the pipeline processors
        let llm = rig::providers::anthropic::Client::from_env();
        let sandbox = DummySandbox::new();

        use crate::agent::Validator;
        let mut tools = toolset(Validator);
        let planning_tools = planning_toolset(self.store.clone(), self.stream_id.clone());
        tools.extend(planning_tools);

        let thread_processor = ThreadProcessor::new(llm, self.store.clone());
        let tool_processor = ToolProcessor::new(
            Box::new(sandbox) as Box<dyn SandboxDyn>,
            self.store.clone(),
            tools,
            Some("sandbox".to_string()),
        );

        let pipeline = Pipeline::new(
            self.store.clone(),
            vec![thread_processor.boxed(), tool_processor.boxed()],
        );

        // Subscribe to events BEFORE starting pipeline and sending message
        let mut event_stream = self.store.subscribe::<AgentEvent>(&self.query)?;

        // Start the pipeline in the background
        println!("DEBUG: Starting pipeline for stream {}", self.stream_id);
        let pipeline_handle = tokio::spawn({
            let stream_id = self.stream_id.clone();
            async move {
                println!("DEBUG: Pipeline task started");
                let result = pipeline.run(stream_id).await;
                println!("DEBUG: Pipeline task ended with result: {:?}", result);
                result
            }
        });

        // Send the command
        self.send_message(command.clone()).await?;

        // Give pipeline a moment to start processing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Wait for events with timeout
        let timeout = tokio::time::Duration::from_secs(timeout_secs);
        let start = tokio::time::Instant::now();

        let mut agent_responses = Vec::new();
        let mut plan_created = false;
        let mut tasks_created = Vec::new();

        while start.elapsed() < timeout {
            let event_timeout = tokio::time::Duration::from_millis(500);
            match tokio::time::timeout(event_timeout, event_stream.next_full()).await {
                Ok(Some(Ok(event))) => {
                    // Debug the actual event type
                    let event_type = match &event.data {
                        AgentEvent::PlanCreated { .. } => "PlanCreated",
                        AgentEvent::PlanUpdated { .. } => "PlanUpdated",
                        AgentEvent::TaskCompleted { .. } => "TaskCompleted",
                        AgentEvent::AgentMessage { .. } => "AgentMessage",
                        AgentEvent::UserMessage(_) => "UserMessage",
                        AgentEvent::ToolResult(_) => "ToolResult",
                        AgentEvent::LLMConfig { .. } => "LLMConfig",
                        _ => "Other",
                    };
                    println!("DEBUG: Received event: {}", event_type);

                    match &event.data {
                        AgentEvent::PlanCreated { tasks } => {
                            plan_created = true;
                            tasks_created = tasks.clone();
                            println!("DEBUG: *** PlanCreated event received with {} tasks!", tasks.len());
                            for (i, task) in tasks.iter().enumerate() {
                                println!("DEBUG:   Task {}: {}", i + 1, task);
                            }
                        }
                        AgentEvent::AgentMessage { response, recipient, .. } => {
                            println!("DEBUG: AgentMessage recipient: {:?}", recipient);
                            // Extract content
                            for content in response.choice.iter() {
                                match content {
                                    rig::message::AssistantContent::Text(text) => {
                                        agent_responses.push(text.text.clone());
                                    }
                                    rig::message::AssistantContent::ToolCall(tool_call) => {
                                        println!("DEBUG: Tool call detected: {}", tool_call.function.name);
                                    }
                                    _ => {}
                                }
                            }

                            // Check finish reason
                            println!("DEBUG: Finish reason: {:?}", response.finish_reason);

                            // Don't break immediately if we're expecting tool use
                            if response.finish_reason == dabgent_agent::llm::FinishReason::ToolUse {
                                println!("DEBUG: Agent is using tools, waiting for results...");
                                // Continue to wait for tool results and subsequent events
                            } else if response.finish_reason == dabgent_agent::llm::FinishReason::Stop {
                                // Give a bit more time for any pending events
                                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            }
                        }
                        AgentEvent::ToolResult(results) => {
                            println!("DEBUG: ToolResult event received with {} results", results.len());
                            for result in results {
                                println!("DEBUG: Tool result content: {:?}", result.content.first());
                            }
                            // Continue waiting for more events after ToolResult, as PlanCreated may follow
                            // Don't stop here
                        }
                        _ => {
                            println!("DEBUG: Other event type received: {:?}", std::any::type_name_of_val(&event.data));
                        }
                    }
                    self.history.push(event.data);
                }
                Ok(Some(Err(e))) => {
                    println!("DEBUG: Event error: {:?}", e);
                    continue;
                }
                Ok(None) => break,
                Err(_) => {
                    // Check if we have enough response
                    if !agent_responses.is_empty() || plan_created {
                        break;
                    }
                }
            }
        }

        // Stop the pipeline
        pipeline_handle.abort();

        // After pipeline stops, load ALL events from the store to ensure we get everything
        let all_events = self.store.load_events::<AgentEvent>(&self.query, None).await?;
        println!("DEBUG: Total events loaded from store: {}", all_events.len());

        // Check for planning events in the loaded events
        for event in &all_events {
            match event {
                AgentEvent::PlanCreated { tasks } => {
                    plan_created = true;
                    tasks_created = tasks.clone();
                    println!("DEBUG: Found PlanCreated in store with {} tasks!", tasks.len());
                }
                AgentEvent::ToolResult(results) => {
                    println!("DEBUG: Found ToolResult in store with {} results", results.len());
                }
                _ => {}
            }
        }

        // Update history with all events
        self.history = all_events;

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

        // Show event types
        println!("\nEvent Types:");
        let mut event_counts = std::collections::HashMap::new();
        for event in &self.events {
            let event_type = match event {
                AgentEvent::LLMConfig { .. } => "LLMConfig",
                AgentEvent::AgentMessage { .. } => "AgentMessage",
                AgentEvent::UserMessage(_) => "UserMessage",
                AgentEvent::PlanCreated { .. } => "PlanCreated",
                AgentEvent::PlanUpdated { .. } => "PlanUpdated",
                AgentEvent::TaskCompleted { .. } => "TaskCompleted",
                AgentEvent::ToolResult(_) => "ToolResult",
                _ => "Other",
            };
            *event_counts.entry(event_type).or_insert(0) += 1;
        }
        for (event_type, count) in event_counts {
            println!("  {}: {}", event_type, count);
        }
    }
}