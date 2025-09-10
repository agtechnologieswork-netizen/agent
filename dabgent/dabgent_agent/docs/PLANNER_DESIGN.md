# Dabgent Planner Design

## Overview

The planner is an event-sourced, LLM-powered task planning and execution system integrated into dabgent_agent. It breaks down natural language requests into structured tasks, orchestrates their execution, and maintains state through event sourcing.

## Architecture

### Module Structure
```
dabgent_agent/src/planner/
├── handler.rs      # Event-sourced planner with Handler trait (256 lines)
├── types.rs        # Core type definitions (106 lines)  
├── llm.rs          # LLM integration for task parsing (208 lines)
├── runner.rs       # Orchestration runner (166 lines)
├── executor.rs     # Task executor handler (185 lines)
├── executor_mq.rs  # Executor event persistence (18 lines)
├── mq.rs          # Planner event persistence (19 lines)
└── mod.rs         # Module exports (10 lines)
Total: 968 lines
```

## Core Components

### 1. Planner (handler.rs)
- Implements `Handler` trait for event sourcing
- Manages task planning state and workflow
- Dispatches tasks and handles executor feedback
- Commands: Initialize, HandleExecutorEvent, Continue
- Events: TasksPlanned, TaskDispatched, TaskStatusUpdated, PlanningCompleted

### 2. Executor (executor.rs)
- Implements `Handler` trait for task execution
- Processes planner commands (PlannerCmd)
- Simulates task execution based on NodeKind
- Handles clarifications and task completion
- Maintains execution state

### 3. LLM Integration (llm.rs)
- Parses natural language into structured tasks using XML
- Classifies task types: Processing, ToolCall, Clarification
- System prompt engineering for consistent parsing

### 4. Runner (runner.rs)
- Orchestrates planner and executor interaction
- Manages event flow between components
- Handles timeout and completion
- Stores events in separate streams

### 5. Types (types.rs)
- NodeKind: Task classification (Processing, ToolCall, Clarification)
- TaskStatus: Planned, Running, Completed, Failed
- PlannerCmd: Commands sent to executor
- ExecutorEvent: Feedback from executor

## Event Flow

```
1. User Input → LLM Parser → TaskPlan[]
2. Planner.Initialize → Event::TasksPlanned
3. Planner → Event::TaskDispatched → Executor
4. Executor.ExecuteTask → ExecutorEventOutput
5. ExecutorEvent → Planner.HandleExecutorEvent
6. Loop until Event::PlanningCompleted
```

## Usage

### Simple Runner
```rust
use dabgent_agent::planner;

// With default 5 minute timeout
planner::runner::run(llm, store, preamble, tools, input).await?

// With custom timeout (seconds)
planner::runner::run_with_timeout(llm, store, preamble, tools, input, 60).await?
```

### Direct Integration
```rust
use dabgent_agent::planner::{Planner, Executor, Command, ExecutorCommand};

// Initialize components
let mut planner = Planner::new();
let mut executor = Executor::new();

// Parse tasks with LLM
let tasks = llm_planner.parse_tasks(&input).await?;

// Initialize planner
let events = planner.process(Command::Initialize { tasks })?;

// Execute dispatched tasks
for event in events {
    if let Event::TaskDispatched { command, .. } = event {
        executor.process(ExecutorCommand::ExecuteTask(command))?;
    }
}
```

## Testing

- Unit tests: `test_planner_handler.rs`, `test_planner_types.rs`
- Integration tests: `test_planner_integration.rs`, `test_planner_executor.rs`
- E2E tests: `test_planner_e2e.rs`
- Examples: `examples/planning.rs`, `examples/planning_full.rs`

## Design Principles

1. **Event Sourcing**: All state changes through events
2. **Separation of Concerns**: Planning vs Execution
3. **Type Safety**: Strong typing with proper error handling
4. **Minimal Dependencies**: Uses existing dabgent infrastructure
5. **Testability**: Comprehensive test coverage