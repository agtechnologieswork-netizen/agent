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

## Phase 2: DabGent MQ Integration (Current Priority)

### Milestone 2.1 — Replace Minimal Infrastructure ⭐
- [x] Basic event store trait and in-memory implementation (now superseded by DabGent MQ)
- [ ] **HIGH PRIORITY**: Implement `dabgent_mq::models::Event` for planner events
- [ ] **HIGH PRIORITY**: Create DabGent MQ adapter for planner persistence
- [ ] **HIGH PRIORITY**: Add planner event → DabGent MQ integration
- [ ] **HIGH PRIORITY**: Replace `InMemoryEventStore` usage with `SqliteStore`
- [ ] Add correlation_id and causation_id to planner operations

### Milestone 2.2 — Real-time Event Streaming
- [ ] **MEDIUM PRIORITY**: Create event subscription handlers for executor integration
- [ ] **MEDIUM PRIORITY**: Set up stream_id and query patterns for planner events
- [ ] **MEDIUM PRIORITY**: Add real-time task dispatch via EventStream subscriptions
- [ ] **MEDIUM PRIORITY**: Implement fan-out pattern for multiple subscribers (audit, metrics, etc.)

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

### 🚧 Next Steps (Revised with DabGent MQ)
1. **DabGent MQ Integration** - Replace our minimal traits with production infrastructure
2. **Real-time Event Streaming** - Leverage subscription capabilities for reactive architecture
3. **LLM Enhancement** - Replace simple parser with LLM-based planning
4. **Advanced Features** - Add sophisticated event processing patterns

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

## Architecture Gaps Analysis

### What We Built vs What's Available

**✅ Our Strengths:**
- **Handler Trait**: Clean separation of business logic from infrastructure
- **Event Sourcing**: Proper fold() implementation for state reconstruction  
- **Command/Event Pattern**: Well-structured planner domain logic
- **Comprehensive Tests**: Full test coverage for planner behavior

**🎯 Integration Opportunities with DabGent MQ:**
- **Production Event Store**: Replace our `InMemoryEventStore` with `SqliteStore`/`PostgresStore`
- **Real-time Streaming**: Add reactive event processing via subscriptions
- **Rich Metadata**: Enhance event tracing with correlation_id/causation_id
- **Performance**: Leverage benchmarked throughput capabilities
- **Migrations**: Database schema evolution support

**🔄 Architecture Evolution:**
```
Before: Handler → InMemoryEventStore → Manual polling
After:  Handler → DabGent MQ → Real-time EventStream → Reactive processing
```

### Immediate Integration Tasks

**Phase 2A: Core Integration (Week 1)**
1. Add `dabgent_mq` dependency to `meta_agent/Cargo.toml`
2. Implement `Event` trait for our planner events  
3. Create adapter layer: `planner::Event` ↔ `dabgent_mq::Event`
4. Replace `InMemoryEventStore` with `SqliteStore` in examples
5. Update tests to use production event store

**Phase 2B: Streaming Architecture (Week 2)**  
1. Create event subscription handlers for planner → executor communication
2. Implement reactive task dispatch via `EventStream`
3. Add metadata enrichment (correlation_id for tracing)
4. Create fan-out pattern for audit, metrics, monitoring

**Phase 2C: Production Readiness (Week 3)**
1. Add database migrations for planner events table
2. Performance testing with DabGent MQ benchmarks
3. Add proper error handling and retry logic
4. Create deployment documentation

### Integration Benefits

**Immediate:**
- Production-ready persistence with ACID guarantees
- Real-time event processing without custom polling
- Rich metadata for debugging and tracing
- Proven performance characteristics

**Long-term:**
- Multi-aggregate event sourcing patterns
- Event replay and projection capabilities
- Horizontal scaling via multiple subscribers  
- Integration with other services via shared event store

## Dependencies & Prerequisites
- Rust toolchain (1.70+) 
- **DabGent MQ**: Full event sourcing and messaging system (already merged!)
- SQLite or PostgreSQL for production event storage
- Existing meta_agent codebase

## Implementation Philosophy
- **Leverage existing infrastructure** - Don't rebuild what DabGent MQ already provides
- Maintain clean separation between business logic and infrastructure via Handler trait
- Write tests alongside implementation, using production infrastructure in tests
- Use DabGent MQ as the integration boundary for all persistence and messaging
- Focus on planner domain logic; let DabGent MQ handle event sourcing infrastructure