use serde::{Deserialize, Serialize};

/// Events for the planning and execution workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlannerEvent {
    // Planning phase events
    UserRequest { content: String },
    PlanCreated { tasks: Vec<PlanTask> },
    PlanPresented { tasks: Vec<PlanTask> },
    UserFeedback { content: String },
    PlanApproved,
    PlanRejected { reason: String },

    // Execution phase events
    PlanReady { tasks: Vec<PlanTask> },
    TaskStarted { task_id: usize, description: String },
    TaskCompleted { task_id: usize, result: String },
    TaskFailed { task_id: usize, error: String },
    AllTasksCompleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTask {
    pub id: usize,
    pub description: String,
    pub dependencies: Vec<usize>, // IDs of tasks that must complete first
}

impl dabgent_mq::Event for PlannerEvent {
    const EVENT_VERSION: &'static str = "1.0";

    fn event_type(&self) -> &'static str {
        match self {
            PlannerEvent::UserRequest { .. } => "user_request",
            PlannerEvent::PlanCreated { .. } => "plan_created",
            PlannerEvent::PlanPresented { .. } => "plan_presented",
            PlannerEvent::UserFeedback { .. } => "user_feedback",
            PlannerEvent::PlanApproved => "plan_approved",
            PlannerEvent::PlanRejected { .. } => "plan_rejected",
            PlannerEvent::PlanReady { .. } => "plan_ready",
            PlannerEvent::TaskStarted { .. } => "task_started",
            PlannerEvent::TaskCompleted { .. } => "task_completed",
            PlannerEvent::TaskFailed { .. } => "task_failed",
            PlannerEvent::AllTasksCompleted => "all_tasks_completed",
        }
    }
}

/// Planner state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlannerState {
    Idle,
    Planning,
    WaitingForApproval,
    Executing { current_task: usize },
    Completed,
    Failed { reason: String },
}

impl Default for PlannerState {
    fn default() -> Self {
        PlannerState::Idle
    }
}