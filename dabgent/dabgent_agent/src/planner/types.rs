use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind { Clarification, ToolCall, Processing }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus { Planned, Running, Completed, Failed, NeedsClarification }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub description: String,
    pub kind: NodeKind,
    pub status: TaskStatus,
}

impl Task {
    pub fn new(id: u64, description: String, kind: NodeKind) -> Self {
        Self { id, description, kind, status: TaskStatus::Planned }
    }
    
    pub fn update_status(&mut self, status: TaskStatus) {
        self.status = status;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlannerState {
    pub tasks: Vec<Task>,
    pub next_id: u64,
    pub waiting_for_clarification: bool,
    pub pending_clarification_for: Option<u64>,
    pub cursor: usize,
    pub dispatched_tasks: std::collections::HashMap<u64, u64>,
}

impl Default for PlannerState {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            next_id: 1,  // Start IDs at 1
            waiting_for_clarification: false,
            pending_clarification_for: None,
            cursor: 0,
            dispatched_tasks: std::collections::HashMap::new(),
        }
    }
}

impl PlannerState {
    pub fn get_task(&self, id: u64) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }
    
    pub fn get_task_mut(&mut self, id: u64) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }
    
    pub fn next_planned_index(&self) -> Option<usize> {
        self.tasks.iter().position(|t| matches!(t.status, TaskStatus::Planned))
    }
    
    pub fn set_clarification(&mut self, id: u64) {
        self.waiting_for_clarification = true;
        self.pending_clarification_for = Some(id);
    }
    
    pub fn clear_clarification(&mut self) {
        self.waiting_for_clarification = false;
        self.pending_clarification_for = None;
    }
    
    pub fn is_dispatched(&self, task_id: u64) -> bool {
        self.dispatched_tasks.contains_key(&task_id)
    }
    
    pub fn mark_dispatched(&mut self, task_id: u64) {
        self.dispatched_tasks.insert(task_id, 0);
    }
    
    pub fn get_next_undispatched_task(&self) -> Option<u64> {
        self.tasks.iter()
            .find(|t| t.status == TaskStatus::Planned && !self.is_dispatched(t.id))
            .map(|t| t.id)
    }
    
    pub fn add_task(&mut self, description: String, kind: NodeKind) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let task = Task::new(id, description, kind);
        self.tasks.push(task);
        id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlannerCmd {
    ExecuteTask { node_id: u64, kind: NodeKind, parameters: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutorEvent {
    TaskCompleted { node_id: u64, result: String },
    TaskFailed { node_id: u64, error: String },
    NeedsClarification { node_id: u64, question: String },
    ClarificationProvided { node_id: u64, answer: String },
}