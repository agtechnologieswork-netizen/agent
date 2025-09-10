//! Tests for planner-executor integration

use dabgent_agent::handler::Handler;
use dabgent_agent::planner::{
    Executor, ExecutorCommand, ExecutorEventOutput,
    Planner, Command, Event, TaskPlan, NodeKind, PlannerCmd,
    ExecutorEvent
};

#[test]
fn test_executor_processing_task() {
    let mut executor = Executor::new();
    
    // Execute a processing task
    let events = executor.process(ExecutorCommand::ExecuteTask(
        PlannerCmd::ExecuteTask {
            node_id: 1,
            kind: NodeKind::Processing,
            parameters: "Analyze data".to_string(),
        }
    )).unwrap();
    
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], ExecutorEventOutput::TaskStarted { task_id: 1 }));
    assert!(matches!(events[1], ExecutorEventOutput::TaskCompleted { task_id: 1, .. }));
}

#[test]
fn test_executor_tool_call() {
    let mut executor = Executor::new();
    
    // Execute a tool call task
    let events = executor.process(ExecutorCommand::ExecuteTask(
        PlannerCmd::ExecuteTask {
            node_id: 2,
            kind: NodeKind::ToolCall,
            parameters: "run tests".to_string(),
        }
    )).unwrap();
    
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], ExecutorEventOutput::TaskStarted { task_id: 2 }));
    assert!(matches!(events[1], ExecutorEventOutput::TaskCompleted { task_id: 2, .. }));
}

#[test]
fn test_executor_clarification() {
    let mut executor = Executor::new();
    
    // Execute a clarification task
    let events = executor.process(ExecutorCommand::ExecuteTask(
        PlannerCmd::ExecuteTask {
            node_id: 3,
            kind: NodeKind::Clarification,
            parameters: "Which API to use?".to_string(),
        }
    )).unwrap();
    
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], ExecutorEventOutput::TaskStarted { task_id: 3 }));
    assert!(matches!(events[1], ExecutorEventOutput::NeedsClarification { task_id: 3, .. }));
}

#[test]
fn test_planner_executor_integration() {
    let mut planner = Planner::new();
    let mut executor = Executor::new();
    
    // Initialize planner with tasks
    let tasks = vec![
        TaskPlan {
            id: 1,
            description: "Process data".to_string(),
            kind: NodeKind::Processing,
        },
        TaskPlan {
            id: 2,
            description: "Run tests".to_string(),
            kind: NodeKind::ToolCall,
        },
    ];
    
    let planner_events = planner.process(Command::Initialize { tasks }).unwrap();
    
    // Should have planned tasks and dispatched first one
    assert!(planner_events.iter().any(|e| matches!(e, Event::TasksPlanned { .. })));
    
    // Find the dispatched task
    let dispatched = planner_events.iter().find_map(|e| {
        if let Event::TaskDispatched { task_id, command } = e {
            Some((task_id, command))
        } else {
            None
        }
    });
    
    assert!(dispatched.is_some());
    let (_task_id, command) = dispatched.unwrap();
    
    // Execute the dispatched task
    let exec_events = executor.process(ExecutorCommand::ExecuteTask(command.clone())).unwrap();
    
    // Should have started and completed
    assert!(exec_events.iter().any(|e| matches!(e, ExecutorEventOutput::TaskStarted { .. })));
    assert!(exec_events.iter().any(|e| matches!(e, ExecutorEventOutput::TaskCompleted { .. })));
    
    // Feed completion back to planner
    if let Some(ExecutorEventOutput::TaskCompleted { task_id, result }) = 
        exec_events.iter().find(|e| matches!(e, ExecutorEventOutput::TaskCompleted { .. })) 
    {
        let planner_response = planner.process(Command::HandleExecutorEvent(
            ExecutorEvent::TaskCompleted { 
                node_id: *task_id, 
                result: result.clone() 
            }
        )).unwrap();
        
        // Should dispatch next task or complete
        assert!(
            planner_response.iter().any(|e| matches!(e, Event::TaskDispatched { .. })) ||
            planner_response.iter().any(|e| matches!(e, Event::PlanningCompleted { .. }))
        );
    }
}

#[test]
fn test_full_planning_execution_cycle() {
    let mut planner = Planner::new();
    let mut executor = Executor::new();
    
    // Initialize with a single task
    let tasks = vec![
        TaskPlan {
            id: 1,
            description: "Simple task".to_string(),
            kind: NodeKind::Processing,
        },
    ];
    
    let init_events = planner.process(Command::Initialize { tasks }).unwrap();
    assert!(init_events.iter().any(|e| matches!(e, Event::TasksPlanned { .. })));
    
    // Get dispatched command
    let command = init_events.iter().find_map(|e| {
        if let Event::TaskDispatched { command, .. } = e {
            Some(command.clone())
        } else {
            None
        }
    }).unwrap();
    
    // Execute task
    let exec_events = executor.process(ExecutorCommand::ExecuteTask(command)).unwrap();
    
    // Get completion event
    let completion = exec_events.iter().find_map(|e| {
        if let ExecutorEventOutput::TaskCompleted { task_id, result } = e {
            Some(ExecutorEvent::TaskCompleted { 
                node_id: *task_id, 
                result: result.clone() 
            })
        } else {
            None
        }
    }).unwrap();
    
    // Feed back to planner
    let final_events = planner.process(Command::HandleExecutorEvent(completion)).unwrap();
    
    // Should complete planning
    assert!(final_events.iter().any(|e| matches!(e, Event::PlanningCompleted { .. })));
}

#[test]
fn test_clarification_flow() {
    let mut planner = Planner::new();
    let mut executor = Executor::new();
    
    // Initialize with a clarification task
    let tasks = vec![
        TaskPlan {
            id: 1,
            description: "Which API should we use?".to_string(),
            kind: NodeKind::Clarification,
        },
    ];
    
    let init_events = planner.process(Command::Initialize { tasks }).unwrap();
    
    // Get dispatched command
    let command = init_events.iter().find_map(|e| {
        if let Event::TaskDispatched { command, .. } = e {
            Some(command.clone())
        } else {
            None
        }
    }).unwrap();
    
    // Execute clarification task
    let exec_events = executor.process(ExecutorCommand::ExecuteTask(command)).unwrap();
    
    // Should need clarification
    let clarification_request = exec_events.iter().find_map(|e| {
        if let ExecutorEventOutput::NeedsClarification { task_id, question } = e {
            Some((*task_id, question.clone()))
        } else {
            None
        }
    }).unwrap();
    
    // Feed clarification request to planner
    let planner_response = planner.process(Command::HandleExecutorEvent(
        ExecutorEvent::NeedsClarification {
            node_id: clarification_request.0,
            question: clarification_request.1,
        }
    )).unwrap();
    
    // Planner should request clarification
    assert!(planner_response.iter().any(|e| matches!(e, Event::ClarificationRequested { .. })));
    
    // Provide clarification
    let clarification_events = executor.process(ExecutorCommand::ProvideClarification {
        task_id: clarification_request.0,
        answer: "Use OpenWeatherMap API".to_string(),
    }).unwrap();
    
    assert!(clarification_events.iter().any(|e| 
        matches!(e, ExecutorEventOutput::ClarificationProvided { .. })
    ));
    
    // Feed clarification back to planner
    let final_events = planner.process(Command::HandleExecutorEvent(
        ExecutorEvent::ClarificationProvided {
            node_id: clarification_request.0,
            answer: "Use OpenWeatherMap API".to_string(),
        }
    )).unwrap();
    
    // Should complete or continue
    assert!(
        final_events.iter().any(|e| matches!(e, Event::PlanningCompleted { .. })) ||
        final_events.iter().any(|e| matches!(e, Event::ClarificationReceived { .. }))
    );
}
