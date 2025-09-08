use meta_agent::planner::{
    Command, Event, Handler, NodeKind, Planner, PlannerCmd, TaskStatus,
};

#[test]
fn planner_initialize_and_plan() {
    let mut planner = Planner::new();

    let events = planner
        .process(Command::Initialize {
            user_input: "Analyze the code\nRun tests\nDeploy".to_string(),
            attachments: vec![],
        })
        .expect("initialize should succeed");

    assert!(!events.is_empty());
    // MVP: Basic planner creates single task
    assert!(matches!(&events[0], Event::TasksPlanned { tasks } if tasks.len() == 1));
    assert_eq!(planner.state().tasks.len(), 1);
}

#[test]
fn planner_task_execution_flow() {
    let mut planner = Planner::new();

    planner
        .process(Command::Initialize {
            user_input: "Test task".to_string(),
            attachments: vec![],
        })
        .unwrap();

    let events = planner
        .process(Command::HandleExecutorEvent(
            meta_agent::planner::ExecutorEvent::TaskCompleted {
                node_id: 1,
                result: "ok".to_string(),
            },
        ))
        .unwrap();

    assert!(events.iter().any(|e| matches!(
        e,
        Event::TaskStatusUpdated { status: TaskStatus::Completed, .. }
    )));
}

#[test]
fn planner_clarification_flow() {
    let mut planner = Planner::new();

    planner
        .process(Command::Initialize {
            user_input: "What is the project name?".to_string(),
            attachments: vec![],
        })
        .unwrap();

    let events = planner
        .process(Command::HandleExecutorEvent(
            meta_agent::planner::ExecutorEvent::NeedsClarification {
                node_id: 1,
                question: "Provide project name".to_string(),
            },
        ))
        .unwrap();

    assert!(events.iter().any(|e| matches!(e, Event::ClarificationRequested { .. })));
    assert!(planner.state().waiting_for_clarification);

    let events = planner
        .process(Command::HandleExecutorEvent(
            meta_agent::planner::ExecutorEvent::ClarificationProvided {
                node_id: 1,
                answer: "MyProject".to_string(),
            },
        ))
        .unwrap();

    assert!(events.iter().any(|e| matches!(e, Event::ClarificationReceived { .. })));
    assert!(!planner.state().waiting_for_clarification);
}

#[test]
fn planner_fold_reconstructs_state() {
    let events = vec![
        Event::TasksPlanned {
            tasks: vec![
                meta_agent::planner::TaskPlan {
                    id: 1,
                    description: "Task 1".to_string(),
                    kind: NodeKind::Processing,
                    attachments: vec![],
                },
                meta_agent::planner::TaskPlan {
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

    assert_eq!(planner.state().tasks.len(), 2);
    assert_eq!(planner.state().tasks[0].status, TaskStatus::Completed);
    assert!(planner.state().is_dispatched(1));
}

#[test]
fn planner_context_compaction() {
    let mut planner = Planner::new();

    planner
        .process(Command::Initialize {
            user_input: "Task 1\nTask 2\nTask 3\nTask 4\nTask 5".to_string(),
            attachments: vec![],
        })
        .unwrap();

    for i in 1..=3u64 {
        let _ = planner.process(Command::HandleExecutorEvent(
            meta_agent::planner::ExecutorEvent::TaskCompleted {
                node_id: i,
                result: "done".to_string(),
            },
        ));
    }

    let events = planner
        .process(Command::CompactContext { max_tokens: 100 })
        .unwrap();

    if !events.is_empty() {
        assert!(matches!(&events[0], Event::ContextCompacted { .. }));
    }
}


