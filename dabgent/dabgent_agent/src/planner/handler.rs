use crate::handler::Handler;
use crate::planner::types::{ExecutorEvent, PlannerCmd, PlannerState, NodeKind, TaskStatus, Task};
use serde::{Deserialize, Serialize};

/// Commands that the planner can process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    /// Initialize planner with user input
    Initialize {
        user_input: String,
    },
    /// Process an event from the executor
    HandleExecutorEvent(ExecutorEvent),
    /// Continue planning after a pause
    Continue,
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
                    let task = Task::new(plan.id, plan.description.clone(), plan.kind);
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

}

impl Handler for Planner {
    type Command = Command;
    type Event = Event;
    type Error = PlannerError;

    fn process(&mut self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        let mut events = Vec::new();

        match command {
            Command::Initialize { user_input } => {
                // Parse input and plan tasks
                let tasks = self.parse_input(&user_input)?;
                events.push(Event::TasksPlanned {
                    tasks,
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

