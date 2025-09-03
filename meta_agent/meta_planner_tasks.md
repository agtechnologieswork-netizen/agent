# Implementation Tasks — Handler-Based Planner (meta_agent)

> Implementation guide for the Handler trait pattern planner

## Phase 1: Core Implementation ✅ COMPLETED

### Milestone 1.1 — Handler Trait & Core Types ✅
- [x] Define Handler trait in `src/planner/handler.rs`:
  - [x] `trait Handler { process(), fold() }`
  - [x] Associated types: Command, Event, Error
- [x] Define command types:
  - [x] `Command { Initialize, HandleExecutorEvent, Continue, CompactContext }`
- [x] Define event types:
  - [x] `Event { TasksPlanned, TaskDispatched, TaskStatusUpdated, ... }`
- [x] Define planner types in `src/planner/types.rs`:
  - [x] `NodeKind { Clarification, ToolCall, Processing }`
  - [x] `TaskStatus { Planned, Running, Completed, NeedsClarification, Failed }`
  - [x] `Task` struct with id, description, kind, status, attachments
  - [x] `PlannerState` with tasks, cursor, waiting flags, context_summary
  - [x] `PlannerConfig` with system_prompt and profile

### Milestone 1.2 — Planner Implementation ✅
- [x] Implement `Planner` struct in `src/planner/handler.rs`:
  - [x] State management (PlannerState)
  - [x] Event log for audit/debugging
- [x] Implement Handler trait for Planner:
  - [x] `process()` method for command handling
  - [x] `fold()` method for event sourcing
  - [x] Event application logic
- [x] Add helper methods:
  - [x] `parse_input()` for task planning
  - [x] `generate_next_command()` for task dispatch
  - [x] `compact_context()` for token management
  - [x] `apply_event()` for state updates

### Milestone 1.3 — Testing ✅
- [x] Unit tests for command processing
- [x] Event sourcing tests (fold/replay)
- [x] Clarification flow tests
- [x] Context compaction tests
- [x] Task execution flow tests

## Phase 2: Infrastructure Integration (Future)

### Milestone 2.1 — Event Store Integration
- [ ] Create event store trait:
  - [ ] `trait EventStore { append(), load_all(), load_from() }`
  - [ ] In-memory implementation for testing
  - [ ] File-based implementation for development
- [ ] Add event persistence:
  - [ ] Save events after each `process()` call
  - [ ] Load events on startup
  - [ ] Support for event replay

### Milestone 2.2 — Message Bus Adapter
- [ ] Create bus adapter for async integration:
  - [ ] Convert Commands from bus messages
  - [ ] Publish Events to appropriate topics
  - [ ] Handle ExecutorEvents from bus
- [ ] Implement routing:
  - [ ] Map PlannerCmd in events to executor topics
  - [ ] Subscribe to executor event topics
  - [ ] Route clarification requests to UI

## Phase 3: LLM Integration

### Milestone 3.1 — Enhanced Task Planning
- [ ] Replace simple parser with LLM-based planning:
  - [ ] Use LLM to parse user input into structured tasks
  - [ ] Better NodeKind classification
  - [ ] Extract dependencies between tasks
  - [ ] Identify required attachments/resources

### Milestone 3.2 — Context Compaction
- [ ] Integrate with existing compaction utilities:
  - [ ] `compact_error_message()` for error reduction
  - [ ] `compact_thread()` for conversation compaction
- [ ] Configure compaction:
  - [ ] Set token_budget in PlannerConfig
  - [ ] Set error_char_limit for error messages
  - [ ] Choose compaction profile ("coding", "analysis")
- [ ] Wire into planner:
  - [ ] Call compaction when context exceeds budget
  - [ ] Store compacted summaries
  - [ ] Use summaries in future prompts

## Phase 4: Integration with meta_agent

### Milestone 4.1 — Wire into Existing System
- [ ] Integrate with actors.rs:
  - [ ] Create planner instance
  - [ ] Route user input → Command::Initialize
  - [ ] Handle ExecutorEvents → Command::HandleExecutorEvent
  - [ ] Extract PlannerCmd from events
- [ ] Connect to executor:
  - [ ] Map NodeKind to appropriate actors
  - [ ] Route task execution results back
  - [ ] Handle clarification flows

### Milestone 4.2 — Example Applications
- [ ] Create example usage patterns:
  - [ ] Synchronous CLI application
  - [ ] Async web service integration
  - [ ] Test harness for development
- [ ] Document integration patterns:
  - [ ] How to handle events
  - [ ] How to persist/restore state
  - [ ] How to integrate with message bus

## Phase 5: Production Enhancements (Future)

### Milestone 5.1 — Advanced Features
- [ ] Parallel task execution support
- [ ] Task dependency graphs
- [ ] Checkpoint/restore functionality
- [ ] Retry policies with backoff
- [ ] Task cancellation support

### Milestone 5.2 — Monitoring & Observability
- [ ] Add metrics:
  - [ ] Task completion rates
  - [ ] Average task duration
  - [ ] Clarification frequency
- [ ] Add tracing:
  - [ ] Command/event correlation
  - [ ] Performance bottlenecks
  - [ ] Error tracking
- [ ] Health checks:
  - [ ] Planner state validity
  - [ ] Event log consistency

## Current Status

### ✅ Completed (Phase 1)
- Handler trait implementation
- Command/Event types
- Core Planner logic
- Event sourcing via fold()
- Comprehensive test suite
- Example usage patterns

### 🚧 Next Steps
1. **Infrastructure Integration** - Connect to existing meta_agent system
2. **LLM Enhancement** - Replace simple parser with LLM-based planning
3. **Production Features** - Add persistence, monitoring, advanced features

## Key Design Benefits

### Handler Trait Pattern
- **Separation of Concerns**: Business logic isolated from infrastructure
- **Testability**: Easy to test without mocking infrastructure
- **Flexibility**: Works with any messaging/storage backend
- **Event Sourcing**: Full audit trail and state reconstruction via fold()

### Clean Architecture
```
Commands → Handler.process() → Events
             ↓
        State Update
             ↓
     Infrastructure Layer
```

## Usage Examples

### Direct Usage
```rust
let mut planner = Planner::new();
let events = planner.process(command)?;
```

### With Event Sourcing
```rust
let planner = Planner::fold(&historical_events);
let events = planner.process(Command::Continue)?;
```

### Async Integration
```rust
async fn handle(planner: Arc<Mutex<Planner>>, cmd: Command) {
    let events = planner.lock().await.process(cmd)?;
    for event in events {
        bus.publish(event).await;
    }
}
```

## Dependencies & Prerequisites
- Rust toolchain (1.70+)
- Existing meta_agent codebase
- Optional: Message bus for async integration
- Optional: Event store for persistence

## Implementation Philosophy
- Start simple, add complexity as needed
- Maintain clean separation between business logic and infrastructure
- Write tests alongside implementation
- Use the Handler trait as the integration boundary
- Let infrastructure handle persistence, messaging, monitoring