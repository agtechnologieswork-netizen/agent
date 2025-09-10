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
        tasks: vec![
            TaskPlan {
                id: 1,
                description: "Analyze the code".to_string(),
                kind: NodeKind::Processing,
            },
            TaskPlan {
                id: 2,
                description: "Run tests".to_string(),
                kind: NodeKind::ToolCall,
            },
            TaskPlan {
                id: 3,
                description: "Deploy to production".to_string(),
                kind: NodeKind::ToolCall,
            },
        ],
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
        tasks: vec![
            TaskPlan {
                id: 1,
                description: "Test task".to_string(),
                kind: NodeKind::Processing,
            },
        ],
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
        tasks: vec![
            TaskPlan {
                id: 1,
                description: "What is the project name?".to_string(),
                kind: NodeKind::Clarification,
            },
        ],
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
                },
                TaskPlan {
                    id: 2,
                    description: "Task 2".to_string(),
                    kind: NodeKind::ToolCall,
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

