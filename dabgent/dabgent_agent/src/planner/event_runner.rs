//! Event-driven planner runner using event store for communication

use crate::handler::Handler;
use crate::llm::{LLMClient, LLMClientDyn};
use crate::planner::{Planner, Command, Event, Executor, ExecutorCommand, ExecutorEventOutput};
use crate::planner::llm::LLMPlanner;
use crate::planner::handler::TaskPlan;
use crate::planner::types::{ExecutorEvent, PlannerCmd};
use dabgent_mq::EventStore;
use dabgent_mq::db::Query;
use eyre::Result;
use tokio::time::{timeout, Duration};
use tokio_stream::StreamExt;
use uuid::Uuid;

/// Run planning with event-driven architecture
pub async fn run<T, E>(
    llm: T,
    store: E,
    preamble: String,
    tools: Vec<Box<dyn crate::toolbox::ToolDyn>>,
    input: String,
) -> Result<()>
where
    T: LLMClient + Clone + Send + 'static,
    E: EventStore + Clone + Send + 'static,
{
    run_with_timeout(llm, store, preamble, tools, input, 300).await
}

/// Run planning with custom timeout using event-driven communication
pub async fn run_with_timeout<T, E>(
    llm: T,
    store: E,
    _preamble: String,
    _tools: Vec<Box<dyn crate::toolbox::ToolDyn>>,
    input: String,
    timeout_secs: u64,
) -> Result<()>
where
    T: LLMClient + Clone + Send + 'static,
    E: EventStore + Clone + Send + 'static,
{
    let fut = async {
        let plan_id = Uuid::new_v4().to_string();
        let exec_id = Uuid::new_v4().to_string();
        
        // Parse tasks using LLM
        let llm_planner = LLMPlanner::new(
            Box::new(llm.clone()) as Box<dyn LLMClientDyn>,
            std::env::var("PLANNER_MODEL").unwrap_or_else(|_| "sonnet-4.1".to_string()),
        );
        let parsed_tasks = llm_planner.parse_tasks(&input).await?;
        let tasks: Vec<TaskPlan> = parsed_tasks.into_iter().map(|t| t.into()).collect();
        
        // Spawn planner worker
        let planner_store = store.clone();
        let planner_id = plan_id.clone();
        let planner_handle = tokio::spawn(async move {
            tracing::info!("Starting planner worker");
            let result = run_planner_worker(planner_store, planner_id, tasks).await;
            tracing::info!("Planner worker completed: {:?}", result);
            result
        });
        
        // Spawn executor worker
        let executor_store = store.clone();
        let executor_plan_id = plan_id.clone();
        let executor_handle = tokio::spawn(async move {
            tracing::info!("Starting executor worker");
            let result = run_executor_worker(executor_store, executor_plan_id, exec_id).await;
            tracing::info!("Executor worker completed: {:?}", result);
            result
        });
        
        // Wait for planning completion
        let mut completion_rx = store.subscribe::<Event>(&Query {
            stream_id: "planner".to_owned(),
            event_type: Some("PlanningCompleted".to_owned()),
            aggregate_id: Some(plan_id.clone()),
        })?;
        
        if let Some(Ok(Event::PlanningCompleted { summary })) = completion_rx.next().await {
            tracing::info!("Planning completed: {}", summary);
            
            // Signal executor to stop (in production, would use a control event)
            executor_handle.abort();
            let _ = planner_handle.await;
        }
        
        Ok::<(), eyre::Error>(())
    };
    
    timeout(Duration::from_secs(timeout_secs), fut)
        .await
        .map_err(|_| eyre::eyre!("Timeout after {} seconds", timeout_secs))?
}

/// Planner worker that processes events
async fn run_planner_worker<E>(
    store: E,
    plan_id: String,
    initial_tasks: Vec<TaskPlan>,
) -> Result<()>
where
    E: EventStore + Clone,
{
    let mut planner = Planner::new();
    
    // Initialize with tasks
    tracing::info!("Planner: Initializing with {} tasks", initial_tasks.len());
    let events = planner.process(Command::Initialize { tasks: initial_tasks })?;
    for event in events {
        tracing::info!("Planner: Publishing event: {:?}", event);
        store.push_event("planner", &plan_id, &event, &Default::default()).await?;
        
        // If a task was dispatched, publish it for executor
        if let Event::TaskDispatched { task_id, command } = &event {
            tracing::info!("Planner: Dispatching task {} to executor", task_id);
            store.push_event("planner_commands", &plan_id, &PlannerCommandEvent {
                task_id: *task_id,
                command: command.clone(),
            }, &Default::default()).await?;
        }
    }
    
    // Subscribe to executor feedback
    tracing::info!("Planner: Subscribing to executor feedback for plan_id: {}", plan_id);
    let mut feedback_rx = store.subscribe::<ExecutorFeedbackEvent>(&Query {
        stream_id: "executor_feedback".to_owned(),
        event_type: None,
        aggregate_id: Some(plan_id.clone()),
    })?;
    
    // Process feedback
    tracing::info!("Planner: Waiting for executor feedback...");
    while let Some(result) = feedback_rx.next().await {
        match result {
            Ok(feedback) => {
                tracing::info!("Planner: Received feedback: {:?}", feedback);
                let executor_event = match feedback {
                    ExecutorFeedbackEvent::TaskCompleted { task_id, result } => {
                        ExecutorEvent::TaskCompleted { node_id: task_id, result }
                    }
                    ExecutorFeedbackEvent::TaskFailed { task_id, error } => {
                        ExecutorEvent::TaskFailed { node_id: task_id, error }
                    }
                    ExecutorFeedbackEvent::NeedsClarification { task_id, question } => {
                        ExecutorEvent::NeedsClarification { node_id: task_id, question }
                    }
                    ExecutorFeedbackEvent::ClarificationProvided { task_id, answer } => {
                        ExecutorEvent::ClarificationProvided { node_id: task_id, answer }
                    }
                };
                
                let events = planner.process(Command::HandleExecutorEvent(executor_event))?;
                for event in events {
                    store.push_event("planner", &plan_id, &event, &Default::default()).await?;
                    
                    // Dispatch new tasks
                    if let Event::TaskDispatched { task_id, command } = &event {
                        store.push_event("planner_commands", &plan_id, &PlannerCommandEvent {
                            task_id: *task_id,
                            command: command.clone(),
                        }, &Default::default()).await?;
                    }
                    
                    // Check for completion
                    if matches!(event, Event::PlanningCompleted { .. }) {
                        return Ok(());
                    }
                }
            }
            Err(e) => {
                tracing::error!("Planner: Error receiving feedback: {:?}", e);
                break;
            }
        }
    }
    
    Ok(())
}

/// Executor worker that processes commands
async fn run_executor_worker<E>(
    store: E,
    plan_id: String,
    exec_id: String,
) -> Result<()>
where
    E: EventStore + Clone,
{
    let mut executor = Executor::new();
    
    // Subscribe to planner commands
    tracing::info!("Executor: Subscribing to planner commands for plan_id: {}", plan_id);
    let mut command_rx = store.subscribe::<PlannerCommandEvent>(&Query {
        stream_id: "planner_commands".to_owned(),
        event_type: None,
        aggregate_id: Some(plan_id.clone()),
    })?;
    
    // Process commands
    tracing::info!("Executor: Waiting for commands...");
    while let Some(result) = command_rx.next().await {
        match result {
            Ok(cmd_event) => {
                tracing::info!("Executor: Received command for task {}", cmd_event.task_id);
                let events = executor.process(ExecutorCommand::ExecuteTask(cmd_event.command))?;
                
                for event in events {
                    tracing::info!("Executor: Generated event: {:?}", event);
                    // Store executor event
                    store.push_event("executor", &exec_id, &event, &Default::default()).await?;
                    
                    // Send feedback to planner
                    if let Some(feedback) = convert_to_feedback(&event) {
                        tracing::info!("Executor: Sending feedback to planner: {:?}", feedback);
                        store.push_event("executor_feedback", &plan_id, &feedback, &Default::default()).await?;
                    }
                }
            }
            Err(e) => {
                tracing::error!("Executor: Error receiving command: {:?}", e);
                break;
            }
        }
    }
    
    Ok(())
}

/// Event wrapper for planner commands
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PlannerCommandEvent {
    task_id: u64,
    command: PlannerCmd,
}

impl dabgent_mq::models::Event for PlannerCommandEvent {
    const EVENT_VERSION: &'static str = "1.0";
    fn event_type(&self) -> &'static str { "PlannerCommand" }
}

/// Event wrapper for executor feedback
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
enum ExecutorFeedbackEvent {
    TaskCompleted { task_id: u64, result: String },
    TaskFailed { task_id: u64, error: String },
    NeedsClarification { task_id: u64, question: String },
    ClarificationProvided { task_id: u64, answer: String },
}

impl dabgent_mq::models::Event for ExecutorFeedbackEvent {
    const EVENT_VERSION: &'static str = "1.0";
    fn event_type(&self) -> &'static str {
        match self {
            Self::TaskCompleted { .. } => "TaskCompleted",
            Self::TaskFailed { .. } => "TaskFailed",
            Self::NeedsClarification { .. } => "NeedsClarification",
            Self::ClarificationProvided { .. } => "ClarificationProvided",
        }
    }
}

/// Convert executor output to feedback event
fn convert_to_feedback(event: &ExecutorEventOutput) -> Option<ExecutorFeedbackEvent> {
    match event {
        ExecutorEventOutput::TaskCompleted { task_id, result } => {
            Some(ExecutorFeedbackEvent::TaskCompleted {
                task_id: *task_id,
                result: result.clone(),
            })
        }
        ExecutorEventOutput::TaskFailed { task_id, error } => {
            Some(ExecutorFeedbackEvent::TaskFailed {
                task_id: *task_id,
                error: error.clone(),
            })
        }
        ExecutorEventOutput::NeedsClarification { task_id, question } => {
            Some(ExecutorFeedbackEvent::NeedsClarification {
                task_id: *task_id,
                question: question.clone(),
            })
        }
        ExecutorEventOutput::ClarificationProvided { task_id, answer } => {
            Some(ExecutorFeedbackEvent::ClarificationProvided {
                task_id: *task_id,
                answer: answer.clone(),
            })
        }
        _ => None,
    }
}
