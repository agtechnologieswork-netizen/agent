use crate::handler::Handler;
use crate::llm::{Completion, CompletionResponse, LLMClient};
use crate::thread::{Command, Event, Thread, ToolResponse};
use crate::toolbox::{ToolCallExt, ToolDyn};
use dabgent_mq::EventStore;
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use rig::completion::ToolDefinition;
use std::collections::HashMap;

pub struct Worker<T: LLMClient, E: EventStore> {
    llm: T,
    event_store: E,
    model: String,
    preamble: String,
    temperature: f64,
    max_tokens: u64,
    tools: Vec<ToolDefinition>,
}

impl<T: LLMClient, E: EventStore> Worker<T, E> {
    pub fn new(
        llm: T,
        event_store: E,
        model: String,
        preamble: String,
        tools: Vec<ToolDefinition>,
    ) -> Self {
        Worker {
            llm,
            event_store,
            model,
            preamble,
            temperature: 1.0,
            max_tokens: 8192,
            tools,
        }
    }

    pub async fn run(&self, stream_id: &str, aggregate_id: &str) -> Result<()> {
        let query = dabgent_mq::db::Query {
            stream_id: stream_id.to_owned(),
            event_type: None,
            aggregate_id: Some(aggregate_id.to_owned()),
        };
        let mut receiver = self.event_store.subscribe::<Event>(&query)?;
        while let Some(event) = receiver.next().await {
            if let Err(error) = event {
                tracing::error!(?error, "llm worker");
                continue;
            }
            match event.unwrap() {
                Event::Prompted(..) | Event::ToolCompleted(..) => {
                    let events = self.event_store.load_events::<Event>(&query, None).await?;
                    let mut thread = Thread::fold(&events);
                    let completion = self.completion(&thread).await?;
                    let new_events = thread.process(Command::Completion(completion))?;
                    for event in new_events.iter() {
                        self.event_store
                            .push_event(stream_id, aggregate_id, event, &Default::default())
                            .await?;
                    }
                }
                _ => continue,
            }
        }
        Ok(())
    }

    pub async fn completion(&self, thread: &Thread) -> Result<CompletionResponse> {
        let mut history = thread.messages.clone();
        let message = history.pop().expect("No messages");
        let completion = Completion::new(self.model.clone(), message)
            .history(history)
            .preamble(self.preamble.clone())
            .tools(self.tools.clone())
            .temperature(self.temperature)
            .max_tokens(self.max_tokens);
        self.llm.completion(completion).await
    }

    pub fn temperature(mut self, temperature: f64) -> Self {
        self.temperature = temperature;
        self
    }

    pub fn max_tokens(mut self, max_tokens: u64) -> Self {
        self.max_tokens = max_tokens;
        self
    }
}

pub struct ToolWorker<E: EventStore> {
    sandbox: Box<dyn SandboxDyn>,
    event_store: E,
    tools: Vec<Box<dyn ToolDyn>>,
}

impl<E: EventStore> ToolWorker<E> {
    pub fn new(sandbox: Box<dyn SandboxDyn>, event_store: E, tools: Vec<Box<dyn ToolDyn>>) -> Self {
        Self {
            sandbox,
            event_store,
            tools,
        }
    }

    pub async fn run(&mut self, stream_id: &str, aggregate_id: &str) -> Result<()> {
        let query = dabgent_mq::db::Query {
            stream_id: stream_id.to_owned(),
            event_type: Some("llm_completed".to_owned()),
            aggregate_id: Some(aggregate_id.to_owned()),
        };
        let mut receiver = self.event_store.subscribe::<Event>(&query)?;
        while let Some(event) = receiver.next().await {
            match event {
                Ok(Event::LlmCompleted(response)) if Thread::has_tool_calls(&response) => {
                    let events = self.event_store.load_events::<Event>(&query, None).await?;
                    let mut thread = Thread::fold(&events);
                    let tools = self.run_tools(&response).await?;
                    let command = {
                        let tools = tools.into_iter().map(rig::message::UserContent::ToolResult);
                        ToolResponse {
                            content: rig::OneOrMany::many(tools)?,
                        }
                    };
                    let new_events = thread.process(Command::Tool(command.clone()))?;
                    for event in new_events.iter() {
                        self.event_store
                            .push_event(stream_id, aggregate_id, event, &Default::default())
                            .await?;
                    }

                    // Check if this was a "done" tool call that completed successfully
                    if thread.is_done(&command) {
                        tracing::info!("Task completed! Collecting artifacts...");
                        if let Err(e) = self.collect_and_emit_artifacts(stream_id, aggregate_id).await {
                            tracing::error!("Failed to collect artifacts: {:?}", e);
                        }
                    }
                }
                Err(error) => {
                    tracing::error!(?error, "sandbox worker");
                }
                _ => continue,
            }
        }
        Ok(())
    }

    async fn run_tools(
        &mut self,
        response: &CompletionResponse,
    ) -> Result<Vec<rig::message::ToolResult>> {
        let mut results = Vec::new();
        for content in response.choice.iter() {
            if let rig::message::AssistantContent::ToolCall(call) = content {
                let tool = self.tools.iter().find(|t| t.name() == call.function.name);
                let result = match tool {
                    Some(tool) => {
                        let args = call.function.arguments.clone();
                        tool.call(args, &mut self.sandbox).await?
                    }
                    None => {
                        let error = format!("{} not found", call.function.name);
                        Err(serde_json::json!(error))
                    }
                };
                results.push(call.to_result(result));
            }
        }
        Ok(results)
    }

    async fn collect_and_emit_artifacts(&mut self, stream_id: &str, aggregate_id: &str) -> Result<()> {
        // Create temp directory for export
        let temp_dir = tempfile::tempdir()?;
        let temp_path = temp_dir.path();

        // Export /app directory from container to temp dir
        tracing::info!("Exporting /app directory from container...");
        self.sandbox.export_directory("/app", &temp_path.to_string_lossy()).await?;

        // Collect all files from the exported directory
        tracing::info!("Reading exported files...");
        let files = Self::collect_exported_files(&temp_path)?;
        
        tracing::info!("Collected {} files from sandbox", files.len());
        for (path, _) in &files {
            tracing::info!("  - {}", path);
        }

        // Emit ArtifactsCollected event
        let event = Event::ArtifactsCollected(files);
        self.event_store
            .push_event(stream_id, aggregate_id, &event, &Default::default())
            .await?;

        tracing::info!("ArtifactsCollected event emitted successfully");
        Ok(())
    }

    fn collect_exported_files(export_path: &std::path::Path) -> Result<HashMap<String, String>> {
        use std::fs;
        
        let mut files = HashMap::new();
        let skip_dirs = ["node_modules", ".git", ".venv", "target", "dist", "build", "__pycache__"];
        
        fn collect_dir(
            dir_path: &std::path::Path,
            export_root: &std::path::Path,
            files: &mut HashMap<String, String>,
            skip_dirs: &[&str],
        ) -> Result<()> {
            for entry in fs::read_dir(dir_path)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    let dir_name = path.file_name().unwrap().to_string_lossy();
                    if skip_dirs.contains(&dir_name.as_ref()) {
                        continue;
                    }
                    collect_dir(&path, export_root, files, skip_dirs)?;
                } else if path.is_file() {
                    // Get relative path from export root
                    let rel_path = path.strip_prefix(export_root)?;
                    let file_path = rel_path.to_string_lossy().to_string();
                    
                    // Read file content if it's a text file
                    match fs::read_to_string(&path) {
                        Ok(content) => {
                            files.insert(file_path, content);
                        }
                        Err(_) => {
                            tracing::warn!("Skipping binary file: {:?}", path);
                        }
                    }
                }
            }
            Ok(())
        }
        
        collect_dir(export_path, export_path, &mut files, &skip_dirs)?;
        Ok(files)
    }
}
