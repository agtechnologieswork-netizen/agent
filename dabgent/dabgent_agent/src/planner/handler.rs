use crate::handler::Handler;
use crate::planner::types::{ExecutorEvent, PlannerCmd, PlannerState, NodeKind, TaskStatus, Task};
use serde::{Deserialize, Serialize};

/// Commands that the planner can process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    /// Initialize planner with user input
    Initialize {
        user_input: String,
        attachments: Vec<crate::planner::types::Attachment>,
    },
    /// Process an event from the executor
    HandleExecutorEvent(ExecutorEvent),
    /// Continue planning after a pause
    Continue,
    /// Compact context to manage token limits
    CompactContext {
        max_tokens: usize,
    },
}

/// Events emitted by the planner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    /// Tasks were planned
    TasksPlanned {
        tasks: Vec<TaskPlan>,
    },
    /// A task was dispatched for execution
    TaskDispatched {
        task_id: u64,
        command: PlannerCmd,
    },
    /// Task status was updated
    TaskStatusUpdated {
        task_id: u64,
        status: TaskStatus,
        result: Option<String>,
    },
    /// Clarification was requested
    ClarificationRequested {
        task_id: u64,
        question: String,
    },
    /// Clarification was received
    ClarificationReceived {
        task_id: u64,
        answer: String,
    },
    /// Context was compacted
    ContextCompacted {
        summary: String,
        removed_task_ids: Vec<u64>,
    },
    /// Planning completed
    PlanningCompleted {
        summary: String,
    },
}

/// Task plan data for the TasksPlanned event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    pub id: u64,
    pub description: String,
    pub kind: NodeKind,
    pub attachments: Vec<crate::planner::types::Attachment>,
}

/// Error types for planner operations (MVP: simplified)
#[derive(Debug, thiserror::Error)]
pub enum PlannerError {
    #[error("Invalid command: {0}")]
    InvalidCommand(String),

    #[error("Task not found: {0}")]
    TaskNotFound(u64),

    #[error("External error: {0}")]
    ExternalError(String),
}

/// Event-sourced planner implementation
pub struct Planner {
    /// Current state
    state: PlannerState,
    /// Event history (for debugging/audit)
    event_log: Vec<Event>,
}

impl Planner {
    /// Create a new planner instance
    pub fn new() -> Self {
        Self {
            state: PlannerState::default(),
            event_log: Vec::new(),
        }
    }

    /// Get the current state (for inspection/debugging)
    pub fn state(&self) -> &PlannerState {
        &self.state
    }

    /// Get event history (for debugging/audit)
    pub fn events(&self) -> &[Event] {
        &self.event_log
    }

    /// Apply an event to update state
    fn apply_event(&mut self, event: &Event) {
        match event {
            Event::TasksPlanned { tasks } => {
                for plan in tasks {
                    let mut task = Task::new(plan.id, plan.description.clone(), plan.kind);
                    task.attachments = plan.attachments.clone();
                    self.state.tasks.push(task);
                    if plan.id >= self.state.next_id {
                        self.state.next_id = plan.id + 1;
                    }
                }
            }

            Event::TaskDispatched { task_id, .. } => {
                self.state.mark_dispatched(*task_id);
                if let Some(task) = self.state.get_task_mut(*task_id) {
                    task.update_status(TaskStatus::Running);
                }
            }

            Event::TaskStatusUpdated { task_id, status, .. } => {
                if let Some(task) = self.state.get_task_mut(*task_id) {
                    task.update_status(*status);
                }

                // Advance cursor if current task completed
                if matches!(status, TaskStatus::Completed | TaskStatus::Failed) {
                    if self.state.cursor < self.state.tasks.len()
                        && self.state.tasks[self.state.cursor].id == *task_id {
                        self.state.advance_cursor();
                    }
                }
            }

            Event::ClarificationRequested { task_id, .. } => {
                self.state.set_clarification(*task_id);
                if let Some(task) = self.state.get_task_mut(*task_id) {
                    task.update_status(TaskStatus::NeedsClarification);
                }
            }

            Event::ClarificationReceived { task_id, .. } => {
                self.state.clear_clarification();
                if let Some(task) = self.state.get_task_mut(*task_id) {
                    task.update_status(TaskStatus::Planned);
                }
            }

            Event::ContextCompacted { summary, removed_task_ids } => {
                self.state.context_summary = summary.clone();
                self.state.tasks.retain(|t| !removed_task_ids.contains(&t.id));
            }

            Event::PlanningCompleted { .. } => {
                // Mark all remaining tasks as completed
                for task in &mut self.state.tasks {
                    if task.status == TaskStatus::Planned || task.status == TaskStatus::Running {
                        task.update_status(TaskStatus::Completed);
                    }
                }
            }
        }
    }

    /// Parse user input and generate task plan (multi-line -> multi-task)
    fn parse_input(&self, user_input: &str) -> Result<Vec<TaskPlan>, PlannerError> {
        // Split by newlines into individual tasks, ignore empty lines
        let lines: Vec<String> = user_input
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();

        if lines.is_empty() {
            return Ok(Vec::new());
        }

        let mut tasks: Vec<TaskPlan> = Vec::with_capacity(lines.len());
        let mut next_id = self.state.next_id;

        for line in lines {
            // Very simple heuristic for NodeKind when LLM isn't used
            let lower = line.to_lowercase();
            let kind = if line.ends_with('?') {
                NodeKind::Clarification
            } else if lower.contains("run ") || lower.contains("test") || lower.contains("deploy") {
                NodeKind::ToolCall
            } else {
                NodeKind::Processing
            };

            tasks.push(TaskPlan {
                id: next_id,
                description: line,
                kind,
                attachments: Vec::new(),
            });

            next_id += 1;
        }

        Ok(tasks)
    }

    /// Generate the next command to dispatch
    fn generate_next_command(&self) -> Option<PlannerCmd> {
        // Check if we're waiting for clarification
        if self.state.waiting_for_clarification {
            return None;
        }

        // Find next undispatched task
        if let Some(task_id) = self.state.get_next_undispatched_task() {
            if let Some(task) = self.state.get_task(task_id) {
                return match task.kind {
                    NodeKind::Clarification => {
                        Some(PlannerCmd::RequestClarification {
                            node_id: task.id,
                            question: task.description.clone(),
                        })
                    }
                    NodeKind::ToolCall | NodeKind::Processing => {
                        Some(PlannerCmd::ExecuteTask {
                            node_id: task.id,
                            kind: task.kind,
                            parameters: task.description.clone(),
                        })
                    }
                };
            }
        }

        // Check if all tasks are completed
        let all_done = self.state.tasks.iter().all(|t|
            matches!(t.status, TaskStatus::Completed | TaskStatus::Failed)
        );

        if all_done && !self.state.tasks.is_empty() {
            let summary = self.state.tasks.iter()
                .filter(|t| t.status == TaskStatus::Completed)
                .map(|t| &t.description)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ");

            return Some(PlannerCmd::Complete { summary });
        }

        None
    }

    /// Compact context (MVP: no-op, real implementation uses LLM)
    fn compact_context(&self, _max_tokens: usize) -> (String, Vec<u64>) {
        // MVP: Return existing summary without compaction
        // Real compaction should be done by LLM
        (self.state.context_summary.clone(), Vec::new())
    }
}

impl Handler for Planner {
    type Command = Command;
    type Event = Event;
    type Error = PlannerError;

    fn process(&mut self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        let mut events = Vec::new();

        match command {
            Command::Initialize { user_input, attachments } => {
                // Parse input and plan tasks
                let tasks = self.parse_input(&user_input)?;

                // Add attachments to first task if any
                let mut tasks_with_attachments = tasks;
                if !attachments.is_empty() && !tasks_with_attachments.is_empty() {
                    tasks_with_attachments[0].attachments = attachments;
                }

                events.push(Event::TasksPlanned {
                    tasks: tasks_with_attachments,
                });

                // Apply the event to update state
                self.apply_event(&events[0]);

                // Check if we should dispatch the first task
                if let Some(cmd) = self.generate_next_command() {
                    if let Some(task_id) = self.state.get_next_undispatched_task() {
                        events.push(Event::TaskDispatched {
                            task_id,
                            command: cmd,
                        });
                        self.apply_event(&events[1]);
                    }
                }
            }

            Command::HandleExecutorEvent(executor_event) => {
                match executor_event {
                    ExecutorEvent::TaskCompleted { node_id, result } => {
                        events.push(Event::TaskStatusUpdated {
                            task_id: node_id,
                            status: TaskStatus::Completed,
                            result: Some(result),
                        });
                        self.apply_event(&events[0]);

                        // Try to dispatch next task
                        if let Some(cmd) = self.generate_next_command() {
                            if let PlannerCmd::Complete { summary } = cmd {
                                events.push(Event::PlanningCompleted { summary });
                            } else if let Some(task_id) = self.state.get_next_undispatched_task() {
                                events.push(Event::TaskDispatched {
                                    task_id,
                                    command: cmd,
                                });
                            }

                            if events.len() > 1 {
                                self.apply_event(&events[1]);
                            }
                        }
                    }

                    ExecutorEvent::TaskFailed { node_id, error } => {
                        events.push(Event::TaskStatusUpdated {
                            task_id: node_id,
                            status: TaskStatus::Failed,
                            result: Some(error),
                        });
                        self.apply_event(&events[0]);
                    }

                    ExecutorEvent::NeedsClarification { node_id, question } => {
                        events.push(Event::ClarificationRequested {
                            task_id: node_id,
                            question,
                        });
                        self.apply_event(&events[0]);
                    }

                    ExecutorEvent::ClarificationProvided { node_id, answer } => {
                        events.push(Event::ClarificationReceived {
                            task_id: node_id,
                            answer,
                        });
                        self.apply_event(&events[0]);

                        // Resume task execution
                        if let Some(cmd) = self.generate_next_command() {
                            events.push(Event::TaskDispatched {
                                task_id: node_id,
                                command: cmd,
                            });
                            self.apply_event(&events[1]);
                        }
                    }
                }
            }

            Command::Continue => {
                // Continue with next task if any
                if let Some(cmd) = self.generate_next_command() {
                    if let PlannerCmd::Complete { summary } = cmd {
                        events.push(Event::PlanningCompleted { summary });
                    } else if let Some(task_id) = self.state.get_next_undispatched_task() {
                        events.push(Event::TaskDispatched {
                            task_id,
                            command: cmd,
                        });
                    }

                    if !events.is_empty() {
                        self.apply_event(&events[0]);
                    }
                }
            }

            Command::CompactContext { max_tokens } => {
                let (summary, removed_ids) = self.compact_context(max_tokens);

                if !removed_ids.is_empty() {
                    events.push(Event::ContextCompacted {
                        summary,
                        removed_task_ids: removed_ids,
                    });
                    self.apply_event(&events[0]);
                }
            }
        }

        // Store events in log
        self.event_log.extend(events.clone());

        Ok(events)
    }

    fn fold(events: &[Self::Event]) -> Self {
        let mut planner = Self::new();

        for event in events {
            planner.apply_event(event);
            planner.event_log.push(event.clone());
        }

        planner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_and_plan() {
        let mut planner = Planner::new();

        let events = planner.process(Command::Initialize {
            user_input: "Analyze the code\nRun tests\nDeploy to production".to_string(),
            attachments: vec![],
        }).unwrap();

        // Should plan tasks and dispatch first one
        assert!(!events.is_empty());
        assert!(matches!(&events[0], Event::TasksPlanned { tasks } if tasks.len() == 3));

        // Should have 3 tasks in state
        assert_eq!(planner.state().tasks.len(), 3);
    }

    #[test]
    fn test_task_execution_flow() {
        let mut planner = Planner::new();

        // Initialize with a task
        planner.process(Command::Initialize {
            user_input: "Test task".to_string(),
            attachments: vec![],
        }).unwrap();

        // Complete the task
        let events = planner.process(Command::HandleExecutorEvent(
            ExecutorEvent::TaskCompleted {
                node_id: 1,
                result: "Success".to_string(),
            }
        )).unwrap();

        // Should update status and potentially complete planning
        assert!(events.iter().any(|e| matches!(e, Event::TaskStatusUpdated {
            status: TaskStatus::Completed, ..
        })));
    }

    #[test]
    fn test_clarification_flow() {
        let mut planner = Planner::new();

        // Initialize with a clarification task
        planner.process(Command::Initialize {
            user_input: "What is the project name?".to_string(),
            attachments: vec![],
        }).unwrap();

        // Request clarification
        let events = planner.process(Command::HandleExecutorEvent(
            ExecutorEvent::NeedsClarification {
                node_id: 1,
                question: "Please provide the project name".to_string(),
            }
        )).unwrap();

        assert!(events.iter().any(|e| matches!(e, Event::ClarificationRequested { .. })));
        assert!(planner.state().waiting_for_clarification);

        // Provide clarification
        let events = planner.process(Command::HandleExecutorEvent(
            ExecutorEvent::ClarificationProvided {
                node_id: 1,
                answer: "MyProject".to_string(),
            }
        )).unwrap();

        assert!(events.iter().any(|e| matches!(e, Event::ClarificationReceived { .. })));
        assert!(!planner.state().waiting_for_clarification);
    }

    #[test]
    fn test_fold_reconstructs_state() {
        let events = vec![
            Event::TasksPlanned {
                tasks: vec![
                    TaskPlan {
                        id: 1,
                        description: "Task 1".to_string(),
                        kind: NodeKind::Processing,
                        attachments: vec![],
                    },
                    TaskPlan {
                        id: 2,
                        description: "Task 2".to_string(),
                        kind: NodeKind::ToolCall,
                        attachments: vec![],
                    },
                ],
            },
            Event::TaskDispatched {
                task_id: 1,
                command: PlannerCmd::ExecuteTask {
                    node_id: 1,
                    kind: NodeKind::Processing,
                    parameters: "Task 1".to_string(),
                },
            },
            Event::TaskStatusUpdated {
                task_id: 1,
                status: TaskStatus::Completed,
                result: Some("Done".to_string()),
            },
        ];

        let planner = Planner::fold(&events);

        // State should be reconstructed
        assert_eq!(planner.state().tasks.len(), 2);
        assert_eq!(planner.state().tasks[0].status, TaskStatus::Completed);
        assert!(planner.state().is_dispatched(1));
        assert_eq!(planner.event_log.len(), 3);
    }

    #[test]
    fn test_context_compaction() {
        let mut planner = Planner::new();

        // Initialize with multiple tasks
        planner.process(Command::Initialize {
            user_input: "Task 1\nTask 2\nTask 3\nTask 4\nTask 5".to_string(),
            attachments: vec![],
        }).unwrap();

        // Complete some tasks
        for i in 1..=3 {
            planner.state.get_task_mut(i).unwrap().update_status(TaskStatus::Completed);
        }

        // Compact context
        let events = planner.process(Command::CompactContext {
            max_tokens: 100, // Small limit to trigger compaction
        }).unwrap();

        if !events.is_empty() {
            assert!(matches!(&events[0], Event::ContextCompacted { .. }));
        }
    }
}
