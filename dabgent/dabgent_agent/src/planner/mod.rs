/// Event-sourced planner module for meta_agent
///
/// This module implements a task planner that:
/// - Parses user input into executable tasks
/// - Manages task execution via event sourcing
/// - Handles clarification requests
/// - Compacts context to manage token limits

pub mod handler;
pub mod types;
pub mod llm;
pub mod mq;
pub mod runner;

// Re-export planner implementation and types
pub use handler::{Command, Event, Planner, PlannerError, TaskPlan};

// Re-export types
pub use types::{
    NodeKind, TaskStatus, PlannerCmd, ExecutorEvent,
    Task, PlannerState
};

// Re-export for convenience
pub use types::{
    NodeKind::{Clarification, ToolCall, Processing},
    TaskStatus::{Planned, Running, Completed, NeedsClarification, Failed},
};