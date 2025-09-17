use crate::agent::{ToolWorker, Worker};
use crate::handler::Handler;
use crate::thread::{self, Thread};
use crate::toolbox::{self, basic::{toolset_with_tasklist, TaskList}};
use dabgent_mq::db::{EventStore, Metadata, Query};
use dabgent_sandbox::SandboxDyn;
use eyre;
use eyre::Result;
use std::env;
use std::future::Future;
use std::pin::Pin;

const DEFAULT_SYSTEM_PROMPT: &str = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.

IMPORTANT: You must use the update_task_list tool to create and update a planning.md file.
1. First use update_task_list to create a task breakdown in planning.md
2. Execute each task and update planning.md marking tasks as complete
3. Use the done tool only when all tasks are complete
";

// Simple TaskList implementation that uses the instruction as the new content
pub struct SimpleTaskList;

impl TaskList for SimpleTaskList {
    fn update(&self, _current_content: String, instruction: String) -> Result<String> {
        // Just use the instruction as the new planning.md content
        Ok(instruction)
    }
}

pub struct PlannerValidator;

impl toolbox::Validator for PlannerValidator {
    async fn run(
        &self,
        sandbox: &mut Box<dyn SandboxDyn>,
    ) -> Result<Result<(), String>, eyre::Report> {
        let result = sandbox.exec("uv run main.py").await?;
        Ok(match result.exit_code {
            0 | 124 => Ok(()),
            code => Err(format!(
                "code: {}\nstdout: {}\nstderr: {}",
                code, result.stdout, result.stderr
            )),
        })
    }
}

pub struct PlanningAgent<S: EventStore> {
    store: S,
    planning_stream_id: String,
    planning_aggregate_id: String,
}

impl<S: EventStore> PlanningAgent<S> {
    pub fn new(store: S, base_stream_id: String, _base_aggregate_id: String) -> Self {
        Self {
            store,
            planning_stream_id: format!("{}_planning", base_stream_id),
            planning_aggregate_id: "thread".to_string(),
        }
    }

    pub async fn process_message(&self, content: String) -> eyre::Result<()> {
        self.store
            .push_event(
                &self.planning_stream_id,
                &self.planning_aggregate_id,
                &thread::Event::Prompted(content),
                &Metadata::default(),
            )
            .await?;
        Ok(())
    }

    pub async fn setup_workers(
        self,
        sandbox: Box<dyn SandboxDyn>,
        llm: rig::providers::anthropic::Client,
    ) -> eyre::Result<()> {
        let task_list = SimpleTaskList;
        let tools = toolset_with_tasklist(PlannerValidator, task_list);
        let planning_worker = Worker::new(
            llm.clone(),
            self.store.clone(),
            "claude-sonnet-4-20250514".to_owned(),
            env::var("SYSTEM_PROMPT").unwrap_or_else(|_| DEFAULT_SYSTEM_PROMPT.to_owned()),
            tools.iter().map(|tool| tool.definition()).collect(),
        );
        let task_list2 = SimpleTaskList;
        let tools = toolset_with_tasklist(PlannerValidator, task_list2);
        let mut sandbox_worker = ToolWorker::new(sandbox, self.store.clone(), tools);
        let stream = self.planning_stream_id.clone();
        let aggregate = self.planning_aggregate_id.clone();
        tokio::spawn(async move {
            let _ = planning_worker.run(&stream, &aggregate).await;
        });
        let stream = self.planning_stream_id.clone();
        let aggregate = self.planning_aggregate_id.clone();
        tokio::spawn(async move {
            let _ = sandbox_worker.run(&stream, &aggregate).await;
        });
        Ok(())
    }

    pub async fn monitor_progress<F>(&self, mut on_status: F) -> eyre::Result<()>
    where
        F: FnMut(String) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + Send>> + Send + 'static,
    {
        let mut receiver = self.store.subscribe::<thread::Event>(&Query {
            stream_id: self.planning_stream_id.clone(),
            event_type: None,
            aggregate_id: Some(self.planning_aggregate_id.clone()),
        })?;
        let mut events = self
            .store
            .load_events(
                &Query {
                    stream_id: self.planning_stream_id.clone(),
                    event_type: None,
                    aggregate_id: Some(self.planning_aggregate_id.clone()),
                },
                None,
            )
            .await?;
        let timeout = std::time::Duration::from_secs(300);
        loop {
            match tokio::time::timeout(timeout, receiver.next()).await {
                Ok(Some(Ok(event))) => {
                    events.push(event.clone());
                    let status = match &event {
                        thread::Event::Prompted(p) => format!("🎯 Starting task: {}", p),
                        thread::Event::LlmCompleted(response) => {
                            // Extract text content from LLM response
                            let mut text = String::new();
                            for item in response.choice.iter() {
                                if let rig::message::AssistantContent::Text(t) = item {
                                    text.push_str(&t.text);
                                    text.push('\n');
                                }
                            }
                            eprintln!("DEBUG LLM Response: {}", text);
                            if !text.is_empty() {
                                text.trim().to_string()
                            } else {
                                "🤔 Planning...".to_string()
                            }
                        },
                        thread::Event::ToolCompleted(tool_response) => {
                            // Extract ALL tool results for debugging
                            let mut all_results = String::new();
                            for item in tool_response.content.iter() {
                                if let rig::message::UserContent::ToolResult(result) = item {
                                    for content in result.content.iter() {
                                        if let rig::message::ToolResultContent::Text(t) = content {
                                            all_results.push_str(&format!("Tool output: {}\n", t.text));
                                        }
                                    }
                                }
                            }
                            eprintln!("DEBUG Tool Results: {}", all_results);

                            // Try to extract planning content
                            let mut text = String::new();
                            for item in tool_response.content.iter() {
                                if let rig::message::UserContent::ToolResult(result) = item {
                                    for content in result.content.iter() {
                                        if let rig::message::ToolResultContent::Text(t) = content {
                                            // Check for any planning-related content
                                            if t.text.contains("Task list") ||
                                               t.text.contains("Planning") ||
                                               t.text.contains("[ ]") ||
                                               t.text.contains("[x]") ||
                                               t.text.contains("✅") {
                                                text = t.text.clone();
                                                break;
                                            }
                                        }
                                    }
                                    if !text.is_empty() { break; }
                                }
                            }

                            if !text.is_empty() {
                                text
                            } else if !all_results.is_empty() {
                                // Send any tool output if we have it
                                all_results.trim().to_string()
                            } else {
                                "🔧 Working...".to_string()
                            }
                        },
                        thread::Event::ArtifactsCollected(files) => {
                            format!("📁 Collected {} artifacts", files.len())
                        }
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
