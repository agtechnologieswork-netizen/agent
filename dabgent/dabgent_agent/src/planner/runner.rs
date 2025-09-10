//! Planner runner that orchestrates planning and execution

use crate::handler::Handler;
use crate::llm::{LLMClient, LLMClientDyn};
use crate::planner::{Planner, Command, Event, Executor, ExecutorCommand, ExecutorEventOutput};
use crate::planner::llm::LLMPlanner;
use crate::planner::handler::TaskPlan;
use crate::planner::types::ExecutorEvent;
use crate::toolbox::ToolDyn;
use dabgent_mq::EventStore;
use eyre::Result;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

/// Run planning with default 5 minute timeout
pub async fn run<T, E>(
    llm: T,
    store: E,
    preamble: String,
    tools: Vec<Box<dyn ToolDyn>>,
    input: String,
) -> Result<()>
where
    T: LLMClient + Clone + Send + 'static,
    E: EventStore + Clone + Send + 'static,
{
    run_with_timeout(llm, store, preamble, tools, input, 300).await
}

/// Run planning with custom timeout in seconds
pub async fn run_with_timeout<T, E>(
    llm: T,
    store: E,
    _preamble: String,
    _tools: Vec<Box<dyn ToolDyn>>,
    input: String,
    timeout_secs: u64,
) -> Result<()>
where
    T: LLMClient + Clone + Send + 'static,
    E: EventStore + Clone + Send + 'static,
{
    let fut = async {
        let id = Uuid::new_v4().to_string();
        
        // Parse tasks using LLM
        let llm_planner = LLMPlanner::new(
            Box::new(llm.clone()) as Box<dyn LLMClientDyn>,
            std::env::var("PLANNER_MODEL").unwrap_or_else(|_| "sonnet-4.1".to_string()),
        );
        let parsed_tasks = llm_planner.parse_tasks(&input).await?;
        
        // Convert to TaskPlan
        let tasks: Vec<TaskPlan> = parsed_tasks.into_iter()
            .map(|t| t.into())
            .collect();
        
        // Initialize planner and executor
        let mut planner = Planner::new();
        let mut executor = Executor::new();
        
        // Initialize planner with parsed tasks
        let initial_events = planner.process(Command::Initialize { tasks })?;
        
        // Store initial planner events and process any dispatched tasks
        for event in &initial_events {
            store.push_event("planner", &id, event, &Default::default()).await?;
            
            if let Event::TasksPlanned { tasks } = event {
                tracing::info!("Planned {} tasks:", tasks.len());
                for task in tasks {
                    tracing::info!("  Task {}: {} ({:?})", task.id, task.description, task.kind);
                }
            }
        }
        
        // Process the planning loop
        let mut pending_tasks = Vec::new();
        for event in initial_events {
            if let Event::TaskDispatched { task_id, command } = event {
                pending_tasks.push((task_id, command));
            }
        }
        
        // Execute tasks and handle feedback loop
        while !pending_tasks.is_empty() {
            let (task_id, command) = pending_tasks.remove(0);
            
            // Execute the task
            let exec_events = executor.process(ExecutorCommand::ExecuteTask(command))?;
            
            // Process executor events
            for exec_event in exec_events {
                store.push_event("executor", &id, &exec_event, &Default::default()).await?;
                
                // Convert relevant executor events back to planner
                if let Some(planner_exec_event) = convert_to_planner_event(&exec_event) {
                    let planner_response = planner.process(
                        Command::HandleExecutorEvent(planner_exec_event)
                    )?;
                    
                    // Process planner's response
                    for response_event in planner_response {
                        store.push_event("planner", &id, &response_event, &Default::default()).await?;
                        
                        match response_event {
                            Event::TaskDispatched { task_id: new_id, command: new_cmd } => {
                                pending_tasks.push((new_id, new_cmd));
                            }
                            Event::PlanningCompleted { summary } => {
                                tracing::info!("Planning completed: {}", summary);
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        
        // If we get here with no pending tasks, check if planning is complete
        let final_events = planner.process(Command::Continue)?;
        for event in final_events {
            store.push_event("planner", &id, &event, &Default::default()).await?;
            if let Event::PlanningCompleted { summary } = event {
                tracing::info!("Planning completed: {}", summary);
            }
        }
        
        Ok::<(), eyre::Error>(())
    };
    
    timeout(Duration::from_secs(timeout_secs), fut)
        .await
        .map_err(|_| eyre::eyre!("Timeout after {} seconds", timeout_secs))?
}

/// Convert executor events to planner's executor events
fn convert_to_planner_event(event: &ExecutorEventOutput) -> Option<ExecutorEvent> {
    match event {
        ExecutorEventOutput::TaskCompleted { task_id, result } => {
            Some(ExecutorEvent::TaskCompleted { 
                node_id: *task_id, 
                result: result.clone() 
            })
        }
        ExecutorEventOutput::TaskFailed { task_id, error } => {
            Some(ExecutorEvent::TaskFailed { 
                node_id: *task_id, 
                error: error.clone() 
            })
        }
        ExecutorEventOutput::NeedsClarification { task_id, question } => {
            Some(ExecutorEvent::NeedsClarification { 
                node_id: *task_id, 
                question: question.clone() 
            })
        }
        ExecutorEventOutput::ClarificationProvided { task_id, answer } => {
            Some(ExecutorEvent::ClarificationProvided { 
                node_id: *task_id, 
                answer: answer.clone() 
            })
        }
        _ => None
    }
}