/// Event-sourced planner module for meta_agent
/// 
/// This module implements a task planner that:
/// - Parses user input into executable tasks
/// - Manages task execution via event sourcing
/// - Handles clarification requests
/// - Compacts context to manage token limits

pub mod handler;
pub mod types;

#[cfg(test)]
pub mod example_usage;

// Re-export core handler trait and implementation
pub use handler::{Command, Event, Handler, Planner, PlannerError, TaskPlan};

// Re-export types
pub use types::{
    NodeKind, TaskStatus, PlannerCmd, ExecutorEvent,
    AttachmentKind, Attachment, Task, PlannerState, PlannerConfig
};

// Re-export for convenience
pub use types::{
    NodeKind::{Clarification, ToolCall, Processing},
    TaskStatus::{Planned, Running, Completed, NeedsClarification, Failed},
};
