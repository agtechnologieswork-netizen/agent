use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Classification for routing & tooling (v1 minimal set)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// Explicit user Q/A
    Clarification,
    /// External tool execution
    ToolCall,
    /// Generic planning/analysis/implementation
    Processing,
}

/// Task execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task is planned but not yet started
    Planned,
    /// Task is currently being executed
    Running,
    /// Task has completed successfully
    Completed,
    /// Task needs user clarification
    NeedsClarification,
    /// Task has failed
    Failed,
}

/// Commands emitted by the planner to the executor (published to bus)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlannerCmd {
    /// Execute a specific task
    ExecuteTask {
        node_id: u64,
        kind: NodeKind,
        parameters: String,
    },
    /// Request clarification from user
    RequestClarification {
        node_id: u64,
        question: String,
    },
    /// Signal completion of all tasks
    Complete {
        summary: String,
    },
}

/// Events received by the planner from the executor/UI (consumed from bus)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutorEvent {
    /// Task completed successfully
    TaskCompleted {
        node_id: u64,
        result: String,
    },
    /// Task failed with error
    TaskFailed {
        node_id: u64,
        error: String,
    },
    /// Task needs clarification
    NeedsClarification {
        node_id: u64,
        question: String,
    },
    /// User provided clarification
    ClarificationProvided {
        node_id: u64,
        answer: String,
    },
}

// Attachments dropped for MVP simplicity

/// Individual task in the execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier
    pub id: u64,
    /// Human-readable task description
    pub description: String,
    /// Task classification for routing
    pub kind: NodeKind,
    /// Current execution status
    pub status: TaskStatus,
    /// Timestamp when task was created
    pub created_at: u64,
    /// Timestamp when task was last updated
    pub updated_at: u64,
}

impl Task {
    /// Create a new task with the given parameters
    pub fn new(id: u64, description: String, kind: NodeKind) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id,
            description,
            kind,
            status: TaskStatus::Planned,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update task status and timestamp
    pub fn update_status(&mut self, status: TaskStatus) {
        self.status = status;
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    // Attachments removed
}

/// Planner state that can be rebuilt from events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerState {
    /// All tasks in the execution plan
    pub tasks: Vec<Task>,
    /// Current position in task sequence
    pub cursor: usize,
    /// Whether planner is waiting for clarification
    pub waiting_for_clarification: bool,
    /// Task ID waiting for clarification
    pub pending_clarification_for: Option<u64>,
    /// Next available task ID
    pub next_id: u64,
    /// Compacted context summary
    pub context_summary: String,
    /// Track which tasks have been dispatched (for idempotency)
    pub dispatched_tasks: HashMap<u64, u64>, // task_id -> timestamp
}

impl Default for PlannerState {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            cursor: 0,
            waiting_for_clarification: false,
            pending_clarification_for: None,
            next_id: 1,
            context_summary: String::new(),
            dispatched_tasks: HashMap::new(),
        }
    }
}

impl PlannerState {
    /// Get a task by ID
    pub fn get_task(&self, id: u64) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Get a mutable task by ID
    pub fn get_task_mut(&mut self, id: u64) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    /// Get the next undispatched task
    pub fn get_next_undispatched_task(&self) -> Option<u64> {
        if self.cursor >= self.tasks.len() {
            return None;
        }

        let task = &self.tasks[self.cursor];
        if task.status == TaskStatus::Planned && !self.dispatched_tasks.contains_key(&task.id) {
            Some(task.id)
        } else {
            None
        }
    }

    /// Check if a task has been dispatched
    pub fn is_dispatched(&self, task_id: u64) -> bool {
        self.dispatched_tasks.contains_key(&task_id)
    }

    /// Mark a task as dispatched
    pub fn mark_dispatched(&mut self, task_id: u64) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.dispatched_tasks.insert(task_id, timestamp);
    }

    /// Advance to the next task
    pub fn advance_cursor(&mut self) {
        if self.cursor < self.tasks.len() {
            self.cursor += 1;
        }
    }

    /// Reset clarification state
    pub fn clear_clarification(&mut self) {
        self.waiting_for_clarification = false;
        self.pending_clarification_for = None;
    }

    /// Set clarification state
    pub fn set_clarification(&mut self, task_id: u64) {
        self.waiting_for_clarification = true;
        self.pending_clarification_for = Some(task_id);
    }

    /// Allocate a new task ID
    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Add a new task to the plan
    pub fn add_task(&mut self, description: String, kind: NodeKind) -> u64 {
        let id = self.alloc_id();
        let task = Task::new(id, description, kind);
        self.tasks.push(task);
        id
    }

    /// Get conversation thread for compaction
    pub fn get_thread(&self) -> Vec<String> {
        let mut thread = Vec::new();

        if !self.context_summary.is_empty() {
            thread.push(format!("Context: {}", self.context_summary));
        }

        for task in &self.tasks {
            if task.status == TaskStatus::Completed {
                thread.push(format!("Task {}: {}", task.id, task.description));
            }
        }

        thread
    }
}

/// Planner configuration (MVP: minimal config)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerConfig {
    /// Maximum tokens for context
    pub token_budget: usize,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            token_budget: 4000,
        }
    }
}

