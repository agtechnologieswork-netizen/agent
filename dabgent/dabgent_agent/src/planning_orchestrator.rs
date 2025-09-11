use crate::thread::Event;
use crate::worker_orchestrator::WorkerOrchestrator;
use dabgent_mq::db::{EventStore, Metadata, Query};
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;

/// Events for the planning system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanningEvent {
    // User events
    UserMessage(String),
    UserClarificationResponse(String),
    
    // Planner events
    PlanCreated(String),
    PlanUpdated(String),
    ClarificationNeeded(String),
    
    // Task events
    TaskStarted(String),
    TaskCompleted(String),
    TaskFailed(String),
}

impl dabgent_mq::Event for PlanningEvent {
    const EVENT_VERSION: &'static str = "1.0";
    fn event_type(&self) -> &'static str {
        match self {
            PlanningEvent::UserMessage(_) => "user_message",
            PlanningEvent::UserClarificationResponse(_) => "user_clarification",
            PlanningEvent::PlanCreated(_) => "plan_created",
            PlanningEvent::PlanUpdated(_) => "plan_updated",
            PlanningEvent::ClarificationNeeded(_) => "clarification_needed",
            PlanningEvent::TaskStarted(_) => "task_started",
            PlanningEvent::TaskCompleted(_) => "task_completed",
            PlanningEvent::TaskFailed(_) => "task_failed",
        }
    }
}

/// Main orchestrator that coordinates user agent, planner, and workers
pub struct PlanningOrchestrator<S: EventStore> {
    store: S,
    user_stream: String,      // Stream for user communication
    planning_stream: String,  // Stream for planning
    worker_stream: String,    // Stream for worker tasks
}

impl<S: EventStore> PlanningOrchestrator<S> {
    pub fn new(store: S, base_stream: String) -> Self {
        Self {
            store,
            user_stream: format!("{}_user", base_stream),
            planning_stream: format!("{}_planning", base_stream),
            worker_stream: format!("{}_worker", base_stream),
        }
    }

    /// Start the user agent that handles user communication
    pub async fn start_user_agent(&self) -> Result<()> {
        let store = self.store.clone();
        let user_stream = self.user_stream.clone();
        let planning_stream = self.planning_stream.clone();
        
        tokio::spawn(async move {
            let mut receiver = store.subscribe::<PlanningEvent>(&Query {
                stream_id: user_stream.clone(),
                event_type: None,
                aggregate_id: None,
            }).unwrap();
            
            while let Some(Ok(event)) = receiver.next().await {
                match event {
                    PlanningEvent::UserMessage(msg) => {
                        tracing::info!("User agent received message: {}", msg);
                        
                        // Forward to planner
                        store.push_event(
                            &planning_stream,
                            "planner",
                            &PlanningEvent::UserMessage(msg),
                            &Metadata::default(),
                        ).await.unwrap();
                    }
                    PlanningEvent::ClarificationNeeded(question) => {
                        tracing::info!("User agent: Clarification needed - {}", question);
                        // In a real implementation, this would prompt the user
                    }
                    PlanningEvent::TaskCompleted(result) => {
                        tracing::info!("User agent: Task completed - {}", result);
                        // Report back to user
                    }
                    _ => {}
                }
            }
        });
        
        Ok(())
    }

    /// Start the planner that manages plan.md and coordinates tasks
    pub async fn start_planner(
        &self,
        sandbox: Box<dyn SandboxDyn>,
    ) -> Result<()> {
        let store = self.store.clone();
        let planning_stream = self.planning_stream.clone();
        let worker_stream = self.worker_stream.clone();
        let user_stream = self.user_stream.clone();
        
        tokio::spawn(async move {
            let mut receiver = store.subscribe::<PlanningEvent>(&Query {
                stream_id: planning_stream.clone(),
                event_type: None,
                aggregate_id: None,
            }).unwrap();
            
            let mut sandbox = sandbox;
            
            while let Some(Ok(event)) = receiver.next().await {
                match event {
                    PlanningEvent::UserMessage(msg) => {
                        tracing::info!("Planner received task: {}", msg);
                        
                        // Create plan.md in the sandbox
                        let plan_content = format!(
                            r#"# Task Planning

## Task Description
{}

## Plan
- [ ] Analyze requirements
- [ ] Create implementation
- [ ] Test solution
- [ ] Validate output

## Status
Planning in progress...
"#,
                            msg
                        );
                        
                        // Write plan to sandbox
                        sandbox.write_file("/app/plan.md", &plan_content).await.unwrap();
                        
                        // Emit plan created event
                        store.push_event(
                            &planning_stream,
                            "planner",
                            &PlanningEvent::PlanCreated(plan_content.clone()),
                            &Metadata::default(),
                        ).await.unwrap();
                        
                        // Start first task
                        store.push_event(
                            &worker_stream,
                            "worker",
                            &PlanningEvent::TaskStarted(msg),
                            &Metadata::default(),
                        ).await.unwrap();
                    }
                    PlanningEvent::TaskCompleted(result) => {
                        tracing::info!("Planner: Task completed - {}", result);
                        
                        // Update plan.md
                        let mut plan = sandbox.read_file("/app/plan.md").await
                            .unwrap_or_else(|_| String::new());
                        plan = plan.replace("- [ ] Create implementation", "- [x] Create implementation");
                        sandbox.write_file("/app/plan.md", &plan).await.unwrap();
                        
                        // Notify user
                        store.push_event(
                            &user_stream,
                            "planner",
                            &PlanningEvent::TaskCompleted(result),
                            &Metadata::default(),
                        ).await.unwrap();
                    }
                    _ => {}
                }
            }
        });
        
        Ok(())
    }

    /// Start workers that execute tasks
    pub async fn start_workers<V>(
        &self,
        sandbox: Box<dyn SandboxDyn>,
        llm: impl crate::llm::LLMClient + 'static,
        validator: V,
    ) -> Result<()>
    where
        V: crate::toolbox::Validator + Clone + Send + Sync + 'static,
    {
        let orchestrator = WorkerOrchestrator::<S, V>::new(
            self.store.clone(),
            self.worker_stream.clone(),
            "worker".to_string(),
        );
        
        // System prompt for workers
        let system_prompt = r#"
You are a Python software engineer implementing tasks.
The plan is already created in plan.md.
Focus on executing the current task.
"#.to_string();
        
        orchestrator.spawn_workers(llm, sandbox, system_prompt, validator).await?;
        
        // Monitor worker events and report completion
        let store = self.store.clone();
        let worker_stream = self.worker_stream.clone();
        let planning_stream = self.planning_stream.clone();
        
        tokio::spawn(async move {
            let mut receiver = store.subscribe::<Event>(&Query {
                stream_id: worker_stream.clone(),
                event_type: None,
                aggregate_id: None,
            }).unwrap();
            
            while let Some(Ok(event)) = receiver.next().await {
                if let Event::ToolCompleted(response) = event {
                    // Check if task is done
                    let response_str = format!("{:?}", response.content);
                    if response_str.contains("done") || response_str.contains("complete") {
                        store.push_event(
                            &planning_stream,
                            "worker",
                            &PlanningEvent::TaskCompleted("Task executed successfully".to_string()),
                            &Metadata::default(),
                        ).await.unwrap();
                    }
                }
            }
        });
        
        Ok(())
    }

    /// Process a user message
    pub async fn process_user_message(&self, message: String) -> Result<()> {
        self.store.push_event(
            &self.user_stream,
            "user",
            &PlanningEvent::UserMessage(message),
            &Metadata::default(),
        ).await?;
        Ok(())
    }
}