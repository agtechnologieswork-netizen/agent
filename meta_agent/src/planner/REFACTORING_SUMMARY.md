# Planner Refactoring: Handler Trait Pattern

## Overview

This refactoring implements grekun's suggested Handler trait pattern to provide a clean separation between the event-sourced planner logic and the bus/messaging infrastructure.

## Core Handler Trait

```rust
pub trait Handler {
    type Command;
    type Event;
    type Error;

    fn process(&mut self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error>;
    fn fold(events: &[Self::Event]) -> Self;
}
```

This simple trait provides:
- **`process`**: Handles commands and emits events
- **`fold`**: Reconstructs state from events (Event Sourcing)

## Implementation Structure

### 1. **Commands** (Input)
- `Initialize`: Start planning with user input
- `HandleExecutorEvent`: Process executor feedback
- `Continue`: Resume planning
- `CompactContext`: Manage token limits

### 2. **Events** (Output)
- `TasksPlanned`: Tasks have been parsed and planned
- `TaskDispatched`: Task sent for execution
- `TaskStatusUpdated`: Task state changed
- `ClarificationRequested/Received`: User interaction needed
- `ContextCompacted`: History summarized
- `PlanningCompleted`: All tasks done

### 3. **State Management**
The `Planner` maintains:
- Current state (`PlannerState`)
- Event log for audit/debugging
- State is modified only through events

## Key Benefits

### 1. **Separation of Concerns**
```rust
// Planner only handles business logic
let mut planner = Planner::new();
let events = planner.process(command)?;

// Infrastructure handles delivery
for event in events {
    bus.publish(event).await;
}
```

### 2. **Event Sourcing**
```rust
// Save events to storage
storage.append(events);

// Later: Rebuild complete state
let historical_events = storage.load_all();
let planner = Planner::fold(&historical_events);
```

### 3. **Testability**
```rust
// Test without any infrastructure
let mut planner = Planner::new();
let events = planner.process(Command::Initialize { 
    user_input: "test".to_string(),
    attachments: vec![],
})?;
assert!(matches!(events[0], Event::TasksPlanned { .. }));
```

### 4. **Integration Flexibility**
The planner can work with:
- Synchronous code (direct function calls)
- Async message buses (Kafka, Redis, etc.)
- HTTP/gRPC servers
- CLI applications
- Test harnesses

## Migration Path

### Current Usage
```rust
// Before: Planner tightly coupled with bus
planner.send_to_bus(command);
let response = bus.receive();
```

### New Usage
```rust
// After: Clean separation
let events = planner.process(command)?;
// Bus handling is external
```

## Event Flow Example

```
User Input → Command::Initialize
    ↓
Planner.process()
    ↓
Events: [TasksPlanned, TaskDispatched]
    ↓
External Bus (publishes events)
    ↓
Executor receives TaskDispatched
    ↓
Executor completes task
    ↓
ExecutorEvent::TaskCompleted → Command::HandleExecutorEvent
    ↓
Planner.process()
    ↓
Events: [TaskStatusUpdated, TaskDispatched] (next task)
    ↓
... continues until PlanningCompleted
```

## File Structure

```
src/planner/
├── mod.rs              # Module exports
├── types.rs            # Core types (unchanged)
├── handler.rs          # Handler trait and Planner implementation
├── example_usage.rs    # Usage examples and patterns
└── REFACTORING_SUMMARY.md  # This document
```

## Testing

The implementation includes comprehensive tests:
- Unit tests for command processing
- Event sourcing/fold tests
- Clarification flow tests
- Context compaction tests
- Integration examples

## Future Enhancements

1. **Persistence Layer**: Add event store adapter
2. **Snapshots**: Periodic state snapshots for faster recovery
3. **Event Versioning**: Support schema evolution
4. **Parallel Execution**: Track multiple concurrent tasks
5. **LLM Integration**: Replace mock parsing with actual LLM calls

## Summary

This refactoring successfully addresses grekun's feedback by:
- ✅ Implementing the exact Handler trait suggested
- ✅ Separating planner logic from bus infrastructure
- ✅ Enabling event sourcing with the `fold` method
- ✅ Providing clean command → event flow
- ✅ Making the planner easily testable and reusable

The planner is now a pure domain component that can be integrated with any infrastructure, making it more maintainable, testable, and flexible.
