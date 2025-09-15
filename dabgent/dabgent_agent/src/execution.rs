//! Execution worker module for handling task execution with a persistent sandbox.

use crate::planner_events::{PlannerEvent, PlanTask};
use dabgent_mq::db::{EventStore, Metadata, Query};
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Run a persistent execution worker that monitors an execution queue
pub async fn run_execution_worker<S: EventStore>(
    store: S,
    stream_id: String,
    aggregate_id: String,
    sandbox: Arc<Mutex<Box<dyn SandboxDyn>>>,
) -> Result<()> {
    let mut current_plan: Option<Vec<PlanTask>> = None;
    let mut completed_tasks: Vec<usize> = Vec::new();

    // Subscribe to execution events
    let mut receiver = store.subscribe::<PlannerEvent>(&Query {
        stream_id: stream_id.clone(),
        event_type: None,
        aggregate_id: Some(aggregate_id.clone()),
    })?;

    tracing::info!("Execution worker started, monitoring queue...");

    while let Some(Ok(event)) = receiver.next().await {
        match event {
            PlannerEvent::PlanReady { tasks } => {
                tracing::info!("Received new plan with {} tasks", tasks.len());
                current_plan = Some(tasks);
                completed_tasks.clear();

                // Start first task if available
                if let Some(ref plan) = current_plan {
                    if let Some(first_task) = plan.first() {
                        store.push_event(
                            &stream_id,
                            &aggregate_id,
                            &PlannerEvent::TaskStarted {
                                task_id: first_task.id,
                                description: first_task.description.clone(),
                            },
                            &Metadata::default()
                        ).await?;
                    }
                }
            }
            PlannerEvent::TaskStarted { task_id, description } => {
                tracing::info!("Starting task {}: {}", task_id, description);

                // Execute the task using the shared sandbox
                let mut sandbox_guard = sandbox.lock().await;
                let result = execute_task(&mut *sandbox_guard, task_id, &description).await;
                drop(sandbox_guard); // Release lock as soon as possible

                match result {
                    Ok(result_msg) => {
                        store.push_event(
                            &stream_id,
                            &aggregate_id,
                            &PlannerEvent::TaskCompleted { task_id, result: result_msg },
                            &Metadata::default()
                        ).await?;
                        completed_tasks.push(task_id);
                    }
                    Err(e) => {
                        store.push_event(
                            &stream_id,
                            &aggregate_id,
                            &PlannerEvent::TaskFailed { task_id, error: e.to_string() },
                            &Metadata::default()
                        ).await?;
                    }
                }
            }
            PlannerEvent::TaskCompleted { .. } => {
                // Find and start next task
                if let Some(ref plan) = current_plan {
                    if let Some(next_task) = find_next_task(plan, &completed_tasks) {
                        store.push_event(
                            &stream_id,
                            &aggregate_id,
                            &PlannerEvent::TaskStarted {
                                task_id: next_task.id,
                                description: next_task.description.clone(),
                            },
                            &Metadata::default()
                        ).await?;
                    } else if completed_tasks.len() == plan.len() {
                        // All tasks completed
                        store.push_event(
                            &stream_id,
                            &aggregate_id,
                            &PlannerEvent::AllTasksCompleted,
                            &Metadata::default()
                        ).await?;
                        // Clear state for next plan
                        current_plan = None;
                        completed_tasks.clear();
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Execute a single task in the sandbox
async fn execute_task(
    sandbox: &mut Box<dyn SandboxDyn>,
    task_id: usize,
    description: &str,
) -> Result<String> {
    // Simple task execution - in real implementation would use LLM
    // For now, just simulate task execution
    tracing::info!("Executing task {}: {}", task_id, description);

    // Example: Write a file to demonstrate sandbox persistence
    let file_path = format!("/app/task_{}.txt", chrono::Utc::now().timestamp());
    sandbox.write_file(&file_path, &format!("Task {}: {}", task_id, description)).await?;

    // List files to show accumulation
    let result = sandbox.exec("ls -la /app/").await?;

    Ok(format!("Task {} completed. Files in sandbox:\n{}", task_id + 1, result.stdout))
}

/// Find the next task that can be executed based on dependencies
pub fn find_next_task<'a>(plan: &'a [PlanTask], completed_tasks: &[usize]) -> Option<&'a PlanTask> {
    plan.iter().find(|task| {
        !completed_tasks.contains(&task.id) &&
        task.dependencies.iter().all(|dep| completed_tasks.contains(dep))
    })
}