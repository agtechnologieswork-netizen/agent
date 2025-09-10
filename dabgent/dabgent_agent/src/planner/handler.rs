use crate::handler::Handler;
use crate::planner::types::{ExecutorEvent, PlannerCmd, PlannerState, NodeKind, TaskStatus, Task};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum PlannerError {
    #[error("Planner error: {0}")]
    Error(String),
}

impl From<eyre::Report> for PlannerError {
    fn from(e: eyre::Report) -> Self {
        PlannerError::Error(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Command {
    Initialize { tasks: Vec<TaskPlan> },  // Keep LLM-parsed tasks
    HandleExecutorEvent(ExecutorEvent),
    Continue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskPlan { pub id: u64, pub description: String, pub kind: NodeKind }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Event {
    TasksPlanned { tasks: Vec<TaskPlan> },
    TaskDispatched { task_id: u64, command: PlannerCmd },
    TaskStatusUpdated { task_id: u64, status: TaskStatus, result: Option<String> },
    ClarificationRequested { task_id: u64, question: String },
    ClarificationReceived { task_id: u64, answer: String },
    PlanningCompleted { summary: String },
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Planner { 
    pub state: PlannerState, 
    pub event_log: Vec<Event> 
}

impl Planner {
    pub fn new() -> Self { Self::default() }
    
    pub fn state(&self) -> &PlannerState { &self.state }
    pub fn events(&self) -> &[Event] { &self.event_log }

    fn apply_event(&mut self, e: &Event) {
        match e {
            Event::TasksPlanned { tasks } => {
                for t in tasks {
                    self.state.tasks.push(Task { 
                        id: t.id, 
                        description: t.description.clone(), 
                        kind: t.kind, 
                        status: TaskStatus::Planned 
                    });
                    if self.state.next_id <= t.id { 
                        self.state.next_id = t.id + 1; 
                    }
                }
            }
            Event::TaskDispatched { task_id, .. } => {
                if let Some(t) = self.state.get_task_mut(*task_id) { 
                    t.update_status(TaskStatus::Running); 
                }
                self.state.mark_dispatched(*task_id);
            }
            Event::TaskStatusUpdated { task_id, status, .. } => {
                if let Some(t) = self.state.get_task_mut(*task_id) { 
                    t.update_status(*status); 
                }
            }
            Event::ClarificationRequested { task_id, .. } => {
                self.state.set_clarification(*task_id);
                if let Some(t) = self.state.get_task_mut(*task_id) {
                    t.update_status(TaskStatus::NeedsClarification);
                }
            }
            Event::ClarificationReceived { .. } => {
                self.state.clear_clarification();
            }
            Event::PlanningCompleted { .. } => {
                for task in &mut self.state.tasks {
                    if task.status == TaskStatus::Planned || task.status == TaskStatus::Running {
                        task.update_status(TaskStatus::Completed);
                    }
                }
            }
        }
    }

    fn maybe_complete(&self) -> bool {
        !self.state.waiting_for_clarification &&
        !self.state.tasks.iter().any(|t| matches!(t.status, TaskStatus::Planned | TaskStatus::Running))
    }
}

impl Handler for Planner {
    type Command = Command;
    type Event = Event;
    type Error = PlannerError;

    fn process(&mut self, cmd: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        let mut out = Vec::new();
        
        match cmd {
            Command::Initialize { tasks } => {
                if self.state.waiting_for_clarification { 
                    return Err(PlannerError::Error("clarification pending".into())); 
                }
                if !self.state.tasks.is_empty() { 
                    return Err(PlannerError::Error("already initialized".into())); 
                }
                if tasks.is_empty() { 
                    return Err(PlannerError::Error("no tasks".into())); 
                }
                
                let e = Event::TasksPlanned { tasks: tasks.clone() };
                out.push(e.clone());
                self.apply_event(&e);
                
                // Dispatch first task
                if let Some(first) = tasks.first() {
                    let e = Event::TaskDispatched {
                        task_id: first.id,
                        command: PlannerCmd::ExecuteTask { 
                            node_id: first.id, 
                            kind: first.kind, 
                            parameters: first.description.clone() 
                        }
                    };
                    out.push(e.clone());
                    self.apply_event(&e);
                }
            }
            
            Command::HandleExecutorEvent(ev) => match ev {
                ExecutorEvent::TaskCompleted { node_id, result } => {
                    let e = Event::TaskStatusUpdated { 
                        task_id: node_id, 
                        status: TaskStatus::Completed, 
                        result: Some(result) 
                    };
                    out.push(e.clone());
                    self.apply_event(&e);
                    
                    // Dispatch next task
                    if let Some(task_id) = self.state.get_next_undispatched_task() {
                        if let Some(t) = self.state.get_task(task_id) {
                            let e = Event::TaskDispatched {
                                task_id: t.id,
                                command: PlannerCmd::ExecuteTask { 
                                    node_id: t.id, 
                                    kind: t.kind, 
                                    parameters: t.description.clone() 
                                }
                            };
                            out.push(e.clone());
                            self.apply_event(&e);
                        }
                    } else if self.maybe_complete() {
                        out.push(Event::PlanningCompleted { 
                            summary: "all tasks completed".into() 
                        });
                    }
                }
                
                ExecutorEvent::TaskFailed { node_id, error } => {
                    let e = Event::TaskStatusUpdated { 
                        task_id: node_id, 
                        status: TaskStatus::Failed, 
                        result: Some(error) 
                    };
                    out.push(e.clone());
                    self.apply_event(&e);
                }
                
                ExecutorEvent::NeedsClarification { node_id, question } => {
                    let e = Event::ClarificationRequested { 
                        task_id: node_id, 
                        question 
                    };
                    out.push(e.clone());
                    self.apply_event(&e);
                }
                
                ExecutorEvent::ClarificationProvided { node_id, answer } => {
                    let e = Event::ClarificationReceived { 
                        task_id: node_id, 
                        answer 
                    };
                    out.push(e.clone());
                    self.apply_event(&e);
                    
                    // Mark task as completed and dispatch next
                    if let Some(t) = self.state.get_task_mut(node_id) {
                        t.update_status(TaskStatus::Completed);
                    }
                    
                    if let Some(task_id) = self.state.get_next_undispatched_task() {
                        if let Some(t) = self.state.get_task(task_id) {
                            let e = Event::TaskDispatched {
                                task_id: t.id,
                                command: PlannerCmd::ExecuteTask { 
                                    node_id: t.id, 
                                    kind: t.kind, 
                                    parameters: t.description.clone() 
                                }
                            };
                            out.push(e.clone());
                            self.apply_event(&e);
                        }
                    } else if self.maybe_complete() {
                        out.push(Event::PlanningCompleted { 
                            summary: "planning done".into() 
                        });
                    }
                }
            },
            
            Command::Continue => {
                if let Some(task_id) = self.state.get_next_undispatched_task() {
                    if let Some(t) = self.state.get_task(task_id) {
                        let e = Event::TaskDispatched {
                            task_id: t.id,
                            command: PlannerCmd::ExecuteTask { 
                                node_id: t.id, 
                                kind: t.kind, 
                                parameters: t.description.clone() 
                            }
                        };
                        out.push(e.clone());
                        self.apply_event(&e);
                    }
                } else if self.maybe_complete() {
                    out.push(Event::PlanningCompleted { 
                        summary: "planning done".into() 
                    });
                }
            }
        }
        
        self.event_log.extend(out.clone());
        Ok(out)
    }

    fn fold(events: &[Self::Event]) -> Self {
        let mut p = Self::new();
        for e in events { 
            p.apply_event(e); 
            p.event_log.push(e.clone()); 
        }
        p
    }
}