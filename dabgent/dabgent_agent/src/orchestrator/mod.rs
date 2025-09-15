//! Orchestrator module for managing the planning and execution workflow.
//!
//! ## Architecture
//! - The orchestrator handles planning and approval phases only
//! - A persistent worker handles execution with a shared sandbox
//! - Communication happens via events through the EventStore

use crate::planner_events::{PlannerEvent, PlannerState, PlanTask};
use dabgent_mq::db::{EventStore, Metadata, Query};
use eyre::Result;

/// Orchestrator manages the complete workflow: planning → approval → execution
pub struct Orchestrator<S: EventStore> {
    store: S,
    planning_stream_id: String,
    execution_stream_id: String,
    aggregate_id: String,
    state: PlannerState,
    current_plan: Option<Vec<PlanTask>>,
}

impl<S: EventStore> Orchestrator<S> {
    pub fn new(store: S, base_stream_id: String, aggregate_id: String) -> Self {
        Self {
            store,
            planning_stream_id: format!("{}_planning", base_stream_id),
            execution_stream_id: format!("{}_execution", base_stream_id),
            aggregate_id,
            state: PlannerState::default(),
            current_plan: None,
        }
    }

    pub async fn create_plan(&mut self, request: String) -> Result<()> {
        self.state = PlannerState::Planning;

        // Publish user request event
        self.store.push_event(
            &self.planning_stream_id,
            &self.aggregate_id,
            &PlannerEvent::UserRequest { content: request.clone() },
            &Metadata::default()
        ).await?;

        // TODO: Create plan in real implementation - this would use LLM)
        let tasks = self.generate_plan_tasks(&request);
        self.current_plan = Some(tasks.clone());

        self.store.push_event(
            &self.planning_stream_id,
            &self.aggregate_id,
            &PlannerEvent::PlanCreated { tasks: tasks.clone() },
            &Metadata::default()
        ).await?;

        // Present plan to user
        self.store.push_event(
            &self.planning_stream_id,
            &self.aggregate_id,
            &PlannerEvent::PlanPresented { tasks },
            &Metadata::default()
        ).await?;

        self.state = PlannerState::WaitingForApproval;
        Ok(())
    }

    /// Queue the current plan for execution by the persistent worker
    pub async fn queue_execution(&mut self) -> Result<()> {
        // Set state to executing if approved
        if matches!(self.state, PlannerState::WaitingForApproval) {
            self.state = PlannerState::Executing { current_task: 0 };
        }

        if !matches!(self.state, PlannerState::Executing { .. }) {
            return Err(eyre::eyre!("Cannot queue execution - plan not approved"));
        }

        let Some(ref plan) = self.current_plan else {
            return Err(eyre::eyre!("No plan available"));
        };

        // Send the plan to the execution worker
        self.store.push_event(
            &self.execution_stream_id,
            &self.aggregate_id,
            &PlannerEvent::PlanReady { tasks: plan.clone() },
            &Metadata::default()
        ).await?;

        Ok(())
    }

    pub async fn wait_for_approval(&mut self) -> Result<bool> {
        let mut receiver = self.store.subscribe::<PlannerEvent>(&Query {
            stream_id: self.planning_stream_id.clone(),
            event_type: None,
            aggregate_id: Some(self.aggregate_id.clone()),
        })?;

        while let Some(Ok(event)) = receiver.next().await {
            match event {
                PlannerEvent::PlanApproved => {
                    self.state = PlannerState::Executing { current_task: 0 };
                    return Ok(true);
                }
                PlannerEvent::PlanRejected { reason } => {
                    self.state = PlannerState::Failed { reason };
                    return Ok(false);
                }
                PlannerEvent::UserFeedback { content } => {
                    //TODO: Handle feedback - could modify plan here
                    tracing::info!("User feedback: {}", content);
                }
                _ => {}
            }
        }

        Ok(false)
    }


    pub async fn monitor_execution<F>(&self, mut on_progress: F) -> Result<()>
    where
        F: FnMut(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> + Send + 'static,
    {
        let mut receiver = self.store.subscribe::<PlannerEvent>(&Query {
            stream_id: self.execution_stream_id.clone(),
            event_type: None,
            aggregate_id: Some(self.aggregate_id.clone()),
        })?;

        while let Some(Ok(event)) = receiver.next().await {
            let status = match event {
                PlannerEvent::TaskStarted { task_id, description } => {
                    format!("🔧 Starting task {}: {}", task_id + 1, description)
                }
                PlannerEvent::TaskCompleted { task_id, result } => {
                    format!("✅ Completed task {}: {}", task_id + 1, result)
                }
                PlannerEvent::TaskFailed { task_id, error } => {
                    format!("❌ Failed task {}: {}", task_id + 1, error)
                }
                PlannerEvent::AllTasksCompleted => {
                    on_progress("🎉 All tasks completed!".to_string()).await?;
                    break;
                }
                _ => continue,
            };

            on_progress(status).await?;
        }

        Ok(())
    }

    fn generate_plan_tasks(&self, request: &str) -> Vec<PlanTask> {
        // Simple plan generation - in real implementation would use LLM
        vec![
            PlanTask {
                id: 0,
                description: format!("Analyze requirements: {}", request),
                dependencies: vec![],
            },
            PlanTask {
                id: 1,
                description: "Create project structure".to_string(),
                dependencies: vec![0],
            },
            PlanTask {
                id: 2,
                description: "Implement core functionality".to_string(),
                dependencies: vec![1],
            },
            PlanTask {
                id: 3,
                description: "Add error handling".to_string(),
                dependencies: vec![2],
            },
            PlanTask {
                id: 4,
                description: "Write tests and documentation".to_string(),
                dependencies: vec![2],
            },
        ]
    }
}