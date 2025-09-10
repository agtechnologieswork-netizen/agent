//! Executor that processes planner commands and reports back results

use crate::handler::Handler;
use crate::planner::types::{ExecutorEvent, NodeKind, PlannerCmd};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Commands the executor can process
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutorCommand {
    /// Execute a task from the planner
    ExecuteTask(PlannerCmd),
    /// Provide clarification answer
    ProvideClarification { task_id: u64, answer: String },
}

/// Events produced by the executor
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutorEventOutput {
    /// Task execution started
    TaskStarted { task_id: u64 },
    /// Task completed successfully
    TaskCompleted { task_id: u64, result: String },
    /// Task failed
    TaskFailed { task_id: u64, error: String },
    /// Needs clarification from user
    NeedsClarification { task_id: u64, question: String },
    /// Clarification provided
    ClarificationProvided { task_id: u64, answer: String },
}

/// Executor state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ExecutorState {
    /// Currently executing tasks
    pub executing_tasks: HashMap<u64, String>,
    /// Completed tasks
    pub completed_tasks: HashMap<u64, String>,
    /// Failed tasks
    pub failed_tasks: HashMap<u64, String>,
    /// Tasks waiting for clarification
    pub pending_clarifications: HashMap<u64, String>,
}

/// The executor handler
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Executor {
    pub state: ExecutorState,
    pub event_log: Vec<ExecutorEventOutput>,
}

impl Executor {
    pub fn new() -> Self {
        Self::default()
    }

    fn apply_event(&mut self, event: &ExecutorEventOutput) {
        match event {
            ExecutorEventOutput::TaskStarted { task_id } => {
                self.state.executing_tasks.insert(*task_id, String::new());
            }
            ExecutorEventOutput::TaskCompleted { task_id, result } => {
                self.state.executing_tasks.remove(task_id);
                self.state.completed_tasks.insert(*task_id, result.clone());
            }
            ExecutorEventOutput::TaskFailed { task_id, error } => {
                self.state.executing_tasks.remove(task_id);
                self.state.failed_tasks.insert(*task_id, error.clone());
            }
            ExecutorEventOutput::NeedsClarification { task_id, question } => {
                self.state.executing_tasks.remove(task_id);
                self.state.pending_clarifications.insert(*task_id, question.clone());
            }
            ExecutorEventOutput::ClarificationProvided { task_id, .. } => {
                self.state.pending_clarifications.remove(task_id);
            }
        }
    }

    /// Simulate task execution (in real implementation, this would call actual tools)
    fn execute_task(&self, task_id: u64, kind: NodeKind, parameters: &str) -> ExecutorEventOutput {
        match kind {
            NodeKind::Clarification => {
                // Clarification tasks need user input
                ExecutorEventOutput::NeedsClarification {
                    task_id,
                    question: parameters.to_string(),
                }
            }
            NodeKind::ToolCall => {
                // Simulate tool execution
                ExecutorEventOutput::TaskCompleted {
                    task_id,
                    result: format!("Executed tool: {}", parameters),
                }
            }
            NodeKind::Processing => {
                // Simulate processing
                ExecutorEventOutput::TaskCompleted {
                    task_id,
                    result: format!("Processed: {}", parameters),
                }
            }
        }
    }
}

/// Executor error type
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("Executor error: {0}")]
    Error(String),
}

impl From<eyre::Report> for ExecutorError {
    fn from(e: eyre::Report) -> Self {
        ExecutorError::Error(e.to_string())
    }
}

impl Handler for Executor {
    type Command = ExecutorCommand;
    type Event = ExecutorEventOutput;
    type Error = ExecutorError;

    fn process(&mut self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        let mut events = Vec::new();

        match command {
            ExecutorCommand::ExecuteTask(planner_cmd) => {
                match planner_cmd {
                    PlannerCmd::ExecuteTask { node_id, kind, parameters } => {
                        // Mark task as started
                        let start_event = ExecutorEventOutput::TaskStarted { task_id: node_id };
                        events.push(start_event.clone());
                        self.apply_event(&start_event);

                        // Execute the task
                        let result_event = self.execute_task(node_id, kind, &parameters);
                        events.push(result_event.clone());
                        self.apply_event(&result_event);
                    }
                }
            }
            ExecutorCommand::ProvideClarification { task_id, answer } => {
                let event = ExecutorEventOutput::ClarificationProvided { task_id, answer };
                events.push(event.clone());
                self.apply_event(&event);
            }
        }

        self.event_log.extend(events.clone());
        Ok(events)
    }

    fn fold(events: &[Self::Event]) -> Self {
        let mut executor = Self::new();
        for event in events {
            executor.apply_event(event);
            executor.event_log.push(event.clone());
        }
        executor
    }
}

/// Convert executor events to planner's executor events
impl From<ExecutorEventOutput> for ExecutorEvent {
    fn from(event: ExecutorEventOutput) -> Self {
        match event {
            ExecutorEventOutput::TaskCompleted { task_id, result } => {
                ExecutorEvent::TaskCompleted { node_id: task_id, result }
            }
            ExecutorEventOutput::TaskFailed { task_id, error } => {
                ExecutorEvent::TaskFailed { node_id: task_id, error }
            }
            ExecutorEventOutput::NeedsClarification { task_id, question } => {
                ExecutorEvent::NeedsClarification { node_id: task_id, question }
            }
            ExecutorEventOutput::ClarificationProvided { task_id, answer } => {
                ExecutorEvent::ClarificationProvided { node_id: task_id, answer }
            }
            _ => panic!("Cannot convert {:?} to ExecutorEvent", event),
        }
    }
}
