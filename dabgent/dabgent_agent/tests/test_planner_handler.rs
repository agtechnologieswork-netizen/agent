//! Tests for planner handler module

use dabgent_agent::handler::Handler;
use dabgent_agent::planner::{
    Planner, Command, Event, TaskPlan, NodeKind, TaskStatus, 
    PlannerCmd, ExecutorEvent
};

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
}

#[test]
fn test_context_compaction() {
    let mut planner = Planner::new();

    // Initialize with multiple tasks
    planner.process(Command::Initialize {
        user_input: "Task 1\nTask 2\nTask 3\nTask 4\nTask 5".to_string(),
        attachments: vec![],
    }).unwrap();

    // Complete some tasks by processing events
    for i in 1..=3 {
        planner.process(Command::HandleExecutorEvent(
            ExecutorEvent::TaskCompleted {
                node_id: i,
                result: format!("Task {} done", i),
            }
        )).unwrap();
    }

    // Compact context
    let events = planner.process(Command::CompactContext {
        max_tokens: 100, // Small limit to trigger compaction
    }).unwrap();

    if !events.is_empty() {
        assert!(matches!(&events[0], Event::ContextCompacted { .. }));
    }
}
