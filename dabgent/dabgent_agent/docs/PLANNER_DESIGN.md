# Dabgent Planner Design (MVP)

## Overview

The planner is an event-sourced, LLM-powered task planning system integrated into dabgent_agent. It breaks down natural language requests into structured tasks using event sourcing for state management.

## Architecture

### Module Structure
```
dabgent_agent/src/planner/
├── handler.rs    # Event-sourced planner with Handler trait (379 lines)
├── types.rs      # Core type definitions (228 lines)  
├── llm.rs        # LLM integration for task parsing (208 lines)
├── runner.rs     # Minimal execution runner (80 lines)
├── mod.rs        # Module exports (27 lines)
└── mq.rs         # Event persistence layer (26 lines)
Total: 948 lines
```

## Core Components

### 1. Handler Pattern
```rust
pub trait Handler {
    fn process(command) -> Result<Vec<Event>>;
    fn fold(events) -> Self;
}
```

### 2. LLM Task Parsing
- Converts natural language into structured tasks
- Uses XML format for reliable parsing
- Classifies task types (Processing, ToolCall, Clarification)

### 3. Event Persistence
- Uses DabGent MQ (SQLite/PostgreSQL)
- Event subscription and replay
- Audit trail

### 4. Simple Runner
```rust
// Default 5 minute timeout
planner::runner::run(llm, store, preamble, tools, input).await?

// Custom timeout (seconds)
planner::runner::run_with_timeout(llm, store, preamble, tools, input, 60).await?
```

## Event Flow

```
User Input → Planner → Events → Event Store
                         ↓
                    Worker → Task Execution
```

## Key Events

- `Initialize`: Start planning with user input
- `TasksPlanned`: Initial task list created
- `TaskDispatched`: Task sent for execution
- `PlanningCompleted`: All tasks done

## Integration

- Works with existing Worker unchanged
- Uses standard DabGent MQ patterns
- Compatible with SQLite and PostgreSQL

## API Usage

```rust
use dabgent_agent::planner;

// Setup
let llm = rig::providers::anthropic::Client::from_env();
let store = SqliteStore::new(pool);

// Run planner
planner::runner::run(
    llm,
    store, 
    "You are a helpful assistant".to_string(),
    vec![], // tools
    "Build a todo app".to_string()
).await?;
```

## Testing

- 6 tests passing (integration + e2e)
- Event persistence verified
- Timeout handling tested
