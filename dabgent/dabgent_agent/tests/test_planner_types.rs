//! Tests for planner types module

use dabgent_agent::planner::{Task, TaskStatus, NodeKind, PlannerState};

#[test]
fn test_task_creation() {
    let task = Task::new(1, "Test task".to_string(), NodeKind::Processing);
    assert_eq!(task.id, 1);
    assert_eq!(task.status, TaskStatus::Planned);
    assert_eq!(task.kind, NodeKind::Processing);
}

#[test]
fn test_task_status_update() {
    let mut task = Task::new(1, "Test task".to_string(), NodeKind::Processing);
    task.update_status(TaskStatus::Running);
    assert_eq!(task.status, TaskStatus::Running);
    assert!(task.updated_at >= task.created_at);
}

#[test]
fn test_planner_state_task_management() {
    let mut state = PlannerState::default();
    let id1 = state.add_task("First task".to_string(), NodeKind::Processing);
    let id2 = state.add_task("Second task".to_string(), NodeKind::ToolCall);

    assert_eq!(state.tasks.len(), 2);
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);

    let task = state.get_task(id1).unwrap();
    assert_eq!(task.description, "First task");
}

#[test]
fn test_planner_state_dispatch_tracking() {
    let mut state = PlannerState::default();
    let id = state.add_task("Test task".to_string(), NodeKind::Processing);
    assert!(!state.is_dispatched(id));
    state.mark_dispatched(id);
    assert!(state.is_dispatched(id));
    assert_eq!(state.get_next_undispatched_task(), None);
}

#[test]
fn test_planner_state_clarification() {
    let mut state = PlannerState::default();
    let id = state.add_task("Test task".to_string(), NodeKind::Clarification);
    assert!(!state.waiting_for_clarification);
    assert_eq!(state.pending_clarification_for, None);
    state.set_clarification(id);
    assert!(state.waiting_for_clarification);
    assert_eq!(state.pending_clarification_for, Some(id));
    state.clear_clarification();
    assert!(!state.waiting_for_clarification);
    assert_eq!(state.pending_clarification_for, None);
}
