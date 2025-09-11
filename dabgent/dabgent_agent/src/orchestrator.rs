use crate::agent::{Worker, ToolWorker};
use crate::handler::Handler;
use crate::planning::{PLAN_FILE_NAME, PLAN_INSTRUCTIONS};
use crate::thread::{self, Thread};
use crate::toolbox::{self, basic::toolset};
use dabgent_mq::db::{EventStore, Metadata, Query};
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use std::env;
use std::future::Future;
use std::pin::Pin;

/// System prompt that instructs the LLM to manage plan.md directly
const DEFAULT_SYSTEM_PROMPT: &str = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.

You MUST manage your planning in a plan.md file:
1. Create plan.md when starting a new task
2. Update plan.md as you make progress
3. Use markdown checkboxes: [ ] pending, [~] in progress, [x] completed, [!] failed
4. Add notes and context as needed
5. Keep the plan organized and up-to-date

Always read plan.md first if it exists, then update it with your progress.
";

/// Orchestrates the execution of planning agents and workers
/// The agent will manage plan.md directly using its read/write tools
pub struct PlanningOrchestrator<S: EventStore> {
    store: S,
    stream_id: String,
    aggregate_id: String,
}

impl<S: EventStore> PlanningOrchestrator<S> {
    pub fn new(store: S, stream_id: String, aggregate_id: String) -> Self {
        Self {
            store,
            stream_id: format!("{}_planning", stream_id),
            aggregate_id,
        }
    }

    /// Process a user message by triggering the planning agent
    pub async fn process_message(&self, content: String) -> Result<()> {
        tracing::info!("Publishing Prompted event to stream: {}, aggregate: {}", 
            self.stream_id, self.aggregate_id);
        
        // Include instructions about plan.md in the prompt
        let enhanced_prompt = format!(
            "{}\n\n{}",
            content,
            PLAN_INSTRUCTIONS
        );
        
        self.store.push_event(
            &self.stream_id,
            &self.aggregate_id,
            &thread::Event::Prompted(enhanced_prompt),
            &Metadata::default()
        ).await?;
        Ok(())
    }

    /// Setup and spawn the planning and sandbox workers
    pub async fn setup_workers(
        &self, 
        sandbox: Box<dyn SandboxDyn>, 
        llm: rig::providers::anthropic::Client,
        validator: impl toolbox::Validator + Clone + Send + Sync + 'static,
    ) -> Result<()> {
        tracing::info!("Setting up workers for stream: {}, aggregate: {}", 
            self.stream_id, self.aggregate_id);
        
        // Setup planning worker with LLM
        // The system prompt instructs it to manage plan.md directly
        let tools = toolset(validator.clone());
        let planning_worker = Worker::new(
            llm,
            self.store.clone(),
            env::var("SYSTEM_PROMPT").unwrap_or_else(|_| DEFAULT_SYSTEM_PROMPT.to_owned()),
            tools
        );
        
        // Setup sandbox worker for tool execution
        let tools = toolset(validator);
        let mut sandbox_worker = ToolWorker::new(sandbox, self.store.clone(), tools);
        
        // Spawn workers
        let stream = self.stream_id.clone();
        let aggregate = self.aggregate_id.clone();
        tokio::spawn(async move {
            tracing::info!("Planning worker started");
            let _ = planning_worker.run(&stream, &aggregate).await;
        });
        
        let stream = self.stream_id.clone();
        let aggregate = self.aggregate_id.clone();
        tokio::spawn(async move {
            tracing::info!("Sandbox worker started");
            let _ = sandbox_worker.run(&stream, &aggregate).await;
        });
        
        Ok(())
    }

    /// Monitor the progress of the planning task
    /// The agent will be managing plan.md directly, so we just monitor events
    pub async fn monitor_progress<F>(&self, mut on_status: F) -> Result<()>
    where
        F: FnMut(String) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + 'static,
    {
        let mut receiver = self.store.subscribe::<thread::Event>(&Query {
            stream_id: self.stream_id.clone(),
            event_type: None,
            aggregate_id: Some(self.aggregate_id.clone()),
        })?;
        
        let mut events = self.store.load_events(&Query {
            stream_id: self.stream_id.clone(),
            event_type: None,
            aggregate_id: Some(self.aggregate_id.clone()),
        }, None).await?;
        
        let timeout = std::time::Duration::from_secs(300);
        
        loop {
            match tokio::time::timeout(timeout, receiver.next()).await {
                Ok(Some(Ok(event))) => {
                    events.push(event.clone());
                    
                    let status = match &event {
                        thread::Event::Prompted(task) => {
                            tracing::info!("Starting task: {}", task);
                            // Extract just the task part, not the instructions
                            let task_lines: Vec<&str> = task.lines()
                                .take_while(|line| !line.contains("When managing the plan.md file:"))
                                .collect();
                            format!("🎯 Starting task: {}", task_lines.join(" ").trim())
                        },
                        thread::Event::LlmCompleted(_) => {
                            // The LLM should be managing plan.md directly
                            // We can check if the file exists and report its status
                            if tokio::fs::metadata(PLAN_FILE_NAME).await.is_ok() {
                                if let Ok(content) = tokio::fs::read_to_string(PLAN_FILE_NAME).await {
                                    on_status(format!("📋 Current plan.md:\n```markdown\n{}\n```", content)).await?;
                                }
                            }
                            "🤔 Planning next steps...".to_string()
                        },
                        thread::Event::ToolCompleted(_) => {
                            // Check plan.md after tool execution
                            if tokio::fs::metadata(PLAN_FILE_NAME).await.is_ok() {
                                if let Ok(content) = tokio::fs::read_to_string(PLAN_FILE_NAME).await {
                                    on_status(format!("📋 Updated plan.md:\n```markdown\n{}\n```", content)).await?;
                                }
                            }
                            "🔧 Executing tools...".to_string()
                        },
                    };
                    
                    on_status(status).await?;
                    
                    if matches!(Thread::fold(&events).state, thread::State::Done) {
                        on_status("✅ Task completed successfully!".to_string()).await?;
                        break;
                    }
                }
                Ok(Some(Err(e))) => {
                    on_status(format!("❌ Error: {}", e)).await?;
                    break;
                }
                Ok(None) => {
                    on_status("⚠️ Event stream closed".to_string()).await?;
                    break;
                }
                Err(_) => {
                    on_status("⏱️ Task timed out after 5 minutes".to_string()).await?;
                    break;
                }
            }
        }
        Ok(())
    }
}