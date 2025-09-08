use crate::handler::Handler;
use crate::llm::{Completion, CompletionResponse, LLMClient};
use crate::thread::{Command, Event, Thread};
use crate::toolbox::ToolDyn;
use dabgent_mq::EventStore;
use dabgent_sandbox::{SandboxDyn, SandboxForkDyn};
use eyre::Result;

pub struct Worker<T: LLMClient, E: EventStore> {
    llm: T,
    event_store: E,
    preamble: String,
    tools: Vec<Box<dyn ToolDyn>>,
}

impl<T: LLMClient, E: EventStore> Worker<T, E> {
    pub fn new(llm: T, event_store: E, preamble: String, tools: Vec<Box<dyn ToolDyn>>) -> Self {
        Worker {
            llm,
            event_store,
            preamble,
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
        const MODEL: &str = "claude-sonnet-4-20250514";
        let mut history = thread.messages.clone();
        let message = history.pop().expect("No messages");
        let completion = Completion::new(MODEL.to_owned(), message)
            .history(history)
            .preamble(self.preamble.clone())
            .tools(self.tools.iter().map(|tool| tool.definition()).collect())
            .temperature(1.0)
            .max_tokens(8192);
        self.llm.completion(completion).await
    }
}

pub struct ToolWorker<T: SandboxDyn + SandboxForkDyn, E: EventStore> {
    sandbox: T,
    event_store: E,
    tools: Vec<Box<dyn ToolDyn>>,
}

impl<T: SandboxDyn + SandboxForkDyn, E: EventStore> ToolWorker<T, E> {
    pub fn new(sandbox: T, event_store: E, tools: Vec<Box<dyn ToolDyn>>) -> Self {
        Self {
            sandbox,
            event_store,
            tools,
        }
    }

    pub async fn run(&self, stream_id: &str, aggregate_id: &str) -> Result<()> {
        let mut workspace = self.sandbox.fork().await?;
        let query = dabgent_mq::db::Query {
            stream_id: stream_id.to_owned(),
            event_type: Some("llm_completed".to_owned()),
            aggregate_id: Some(aggregate_id.to_owned()),
        };
        let mut receiver = self.event_store.subscribe::<Event>(&query)?;
        while let Some(event) = receiver.next().await {
            match event {
                Ok(Event::LlmCompleted(response)) if Thread::has_tool_calls(&response) => {}
                Err(error) => {
                    tracing::error!(?error, "sandbox worker");
                }
                _ => continue,
            }
        }
        todo!();
    }
}

pub fn default_toolset() -> Vec<rig::completion::ToolDefinition> {
    vec![
        rig::completion::ToolDefinition {
            name: "bash".to_string(),
            description: "Run a bash command".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command to run in the shell",
                    }
                },
                "required": ["command"],
            }),
        },
        rig::completion::ToolDefinition {
            name: "write_file".to_string(),
            description: "Write content to a file".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file",
                    },
                    "contents": {
                        "type": "string",
                        "description": "Content to write to the file",
                    }
                },
                "required": ["path", "content"],
            }),
        },
        rig::completion::ToolDefinition {
            name: "read_file".to_string(),
            description: "Read content from a file".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file",
                    }
                },
                "required": ["path"],
            }),
        },
        rig::completion::ToolDefinition {
            name: "list_files".to_string(),
            description: "List files in a directory".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the directory",
                    }
                },
                "required": ["path"],
            }),
        },
        rig::completion::ToolDefinition {
            name: "remove_file".to_string(),
            description: "Remove a file".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to remove",
                    }
                },
                "required": ["path"],
            }),
        },
        rig::completion::ToolDefinition {
            name: "edit_file".to_string(),
            description: "Edit a file by replacing text".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file",
                    },
                    "find": {
                        "type": "string",
                        "description": "Text to find in the file",
                    },
                    "replace": {
                        "type": "string",
                        "description": "Text to replace with",
                    }
                },
                "required": ["path", "find", "replace"],
            }),
        },
    ]
}
