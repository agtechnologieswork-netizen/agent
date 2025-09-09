use crate::handler::Handler;
use crate::llm::{Completion, CompletionResponse, LLMClient};
use crate::thread::{Command, Event, Thread, ToolResponse};
use crate::toolbox::{ToolCallExt, ToolDyn};
use dabgent_mq::EventStore;
use dabgent_sandbox::SandboxDyn;
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
                    let new_events = thread.process(Command::Tool(command))?;
                    for event in new_events.iter() {
                        self.event_store
                            .push_event(stream_id, aggregate_id, event, &Default::default())
                            .await?;
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
}

/// Worker that processes planner events and coordinates task execution
pub struct PlannerWorker<T: LLMClient, E: EventStore> {
    llm: T,
    event_store: E,
    planner: crate::planner::Planner,
    thread_bridge: crate::event_router::PlannerThreadBridge<E>,
}

impl<T: LLMClient, E: EventStore> PlannerWorker<T, E> {
    /// Create a new planner worker
    pub fn new(llm: T, event_store: E) -> Self {
        Self {
            llm,
            event_store: event_store.clone(),
            planner: crate::planner::Planner::new(),
            thread_bridge: crate::event_router::PlannerThreadBridge::new(event_store),
        }
    }

    /// Run the planner worker, subscribing to planner events
    pub async fn run(&mut self, stream_id: &str, aggregate_id: &str) -> Result<()> {
        let query = dabgent_mq::db::Query {
            stream_id: format!("{}-planner", stream_id),
            event_type: Some("TaskDispatched".to_string()),
            aggregate_id: Some(aggregate_id.to_owned()),
        };
        
        let mut receiver = self.event_store.subscribe::<crate::planner::Event>(&query)?;
        
        while let Some(event) = receiver.next().await {
            match event {
                Ok(crate::planner::Event::TaskDispatched { task_id, command }) => {
                    // Route task to appropriate executor
                    self.dispatch_to_executor(task_id, command).await?;
                }
                Ok(crate::planner::Event::ClarificationRequested { task_id, question }) => {
                    // Handle clarification request
                    tracing::info!("Clarification needed for task {}: {}", task_id, question);
                    // In Phase 3, we'll implement UI interaction
                }
                Ok(_) => {
                    // Other events are informational
                    continue;
                }
                Err(error) => {
                    tracing::error!(?error, "Error receiving planner event");
                }
            }
        }
        
        Ok(())
    }
    
    /// Dispatch a task to the appropriate executor based on NodeKind
    async fn dispatch_to_executor(
        &self,
        task_id: u64,
        command: crate::planner::PlannerCmd,
    ) -> Result<()> {
        // Use the bridge to convert planner commands to thread events
        self.thread_bridge.handle_task_dispatch(task_id, command).await
    }
    
    /// Initialize planning with user input
    pub async fn initialize_planning(&mut self, user_input: String) -> Result<()> {
        use crate::handler::Handler;
        use crate::planner::Command;
        
        let command = Command::Initialize {
            user_input,
            attachments: vec![],
        };
        
        let events = self.planner.process(command)?;
        
        // Store events to event store
        for event in events {
            self.event_store
                .push_event(
                    "planner",
                    "session",
                    &event,
                    &dabgent_mq::db::Metadata::default(),
                )
                .await?;
        }
        
        Ok(())
    }
    
    /// Handle executor results and update planner state
    pub async fn handle_executor_result(
        &mut self,
        task_id: u64,
        result: Result<String>,
    ) -> Result<()> {
        use crate::handler::Handler;
        use crate::planner::{Command, ExecutorEvent};
        
        let executor_event = match result {
            Ok(output) => ExecutorEvent::TaskCompleted {
                node_id: task_id,
                result: output,
            },
            Err(error) => ExecutorEvent::TaskFailed {
                node_id: task_id,
                error: error.to_string(),
            },
        };
        
        let command = Command::HandleExecutorEvent(executor_event);
        let events = self.planner.process(command)?;
        
        // Store resulting events
        for event in events {
            self.event_store
                .push_event(
                    "planner",
                    "session",
                    &event,
                    &dabgent_mq::db::Metadata::default(),
                )
                .await?;
        }
        
        Ok(())
    }
}
