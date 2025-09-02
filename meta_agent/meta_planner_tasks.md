# Implementation Tasks — Event‑Sourced Planner (meta_agent / meta_draft)

> Sequential implementation guide for building the event-sourced planner

## Phase 1: Foundation (Week 1)

### Milestone 1.1 — Core Types & Interfaces
- [ ] Define base enums in `src/types.rs`:
  - [ ] `NodeKind { Clarification, ToolCall, Processing }`
  - [ ] `TaskStatus { Planned, Running, Completed, NeedsClarification }`
- [ ] Define command/event types:
  - [ ] `PlannerCmd { ExecuteTask, RequestClarification }`
  - [ ] `ExecutorEvent { TaskCompleted, TaskFailed, NeedsClarification, ClarificationProvided }`
- [ ] Define data structures:
  - [ ] `Task` struct with id, description, kind, status, attachments
  - [ ] `PlannerState` with tasks, cursor, waiting flags, context_summary
  - [ ] `PlannerConfig` with system_prompt and profile

### Milestone 1.2 — Event Store Infrastructure
- [ ] Create `src/events/mod.rs`:
  - [ ] Define domain event enum with all planner events
  - [ ] Implement event versioning (start with v1)
  - [ ] Create event metadata structure (id, timestamp, aggregate_id, causation_id)
- [ ] Implement `src/events/store.rs`:
  - [ ] Simple append-only event store trait
  - [ ] In-memory implementation for testing
  - [ ] File-based implementation for development
- [ ] Add event serialization:
  - [ ] JSON serialization for events
  - [ ] Schema validation helpers

## Phase 2: Message Bus & Patterns (Week 1-2)

### Milestone 2.1 — Outbox/Inbox Implementation
- [ ] Create `src/messaging/outbox.rs`:
  - [ ] Outbox table/structure for pending events
  - [ ] Transaction-safe event persistence
  - [ ] Background publisher task
  - [ ] Retry logic with exponential backoff
- [ ] Create `src/messaging/inbox.rs`:
  - [ ] Message deduplication by message_id
  - [ ] Idempotency key tracking
  - [ ] Message acknowledgment logic

### Milestone 2.2 — Message Bus Integration
- [ ] Define `EventBus` trait in `src/messaging/bus.rs`:
  - [ ] publish() method for sending events
  - [ ] subscribe() method for receiving events
  - [ ] Topic/routing key support
- [ ] Implement in-memory bus for testing:
  - [ ] Simple channel-based implementation
  - [ ] Topic filtering support
- [ ] Add message envelope:
  - [ ] Headers: event_type, correlation_id, causation_id
  - [ ] Payload wrapping
  - [ ] Routing key generation

## Phase 3: Core Planner Logic (Week 2)

### Milestone 3.1 — Task Planning
- [ ] Implement `PlannerState::plan_tasks()`:
  - [ ] Text parsing into steps (split by bullets/sentences)
  - [ ] URL extraction via regex
  - [ ] NodeKind classification heuristics:
    - [ ] Commands/backticks → ToolCall
    - [ ] Questions → Clarification  
    - [ ] Default → Processing
  - [ ] Task ID generation
  - [ ] Emit `TaskPlanned` events for each task

### Milestone 3.2 — Event-Driven Step Function
- [ ] Implement `PlannerState::step()`:
  - [ ] Process incoming ExecutorEvent if present
  - [ ] Update task status based on event type
  - [ ] Emit appropriate PlannerCmd
  - [ ] Handle state transitions:
    - [ ] Planned → Running (emit ExecuteTask)
    - [ ] Running → Completed (advance cursor)
    - [ ] Running → NeedsClarification (pause, emit RequestClarification)
    - [ ] NeedsClarification → Planned (resume after answer)
- [ ] Add idempotency checks:
  - [ ] Track dispatched task IDs
  - [ ] Prevent duplicate ExecuteTask emissions

## Phase 4: State Management & Projections (Week 2-3)

### Milestone 4.1 — Event Replay & Projections
- [ ] Create `src/projections/planner_state.rs`:
  - [ ] Build PlannerState from event stream
  - [ ] Handle each domain event type
  - [ ] Maintain consistency during replay
- [ ] Implement replay logic:
  - [ ] Load events from store
  - [ ] Apply events in order
  - [ ] Validate final state
- [ ] Add snapshot support (optional optimization):
  - [ ] Periodic state snapshots
  - [ ] Replay from snapshot + subsequent events

### Milestone 4.2 — Context Compaction
- [ ] Define `Compactor` trait in `src/compaction/mod.rs`:
  - [ ] compact() method with budget parameter
  - [ ] Slot-based prioritization
- [ ] Implement basic compactor:
  - [ ] Chunk text into segments
  - [ ] Score by recency and relevance
  - [ ] Fit within token budget
  - [ ] Preserve key information (constraints, decisions)
- [ ] Wire into planner:
  - [ ] Call after each task completion
  - [ ] Update context_summary
  - [ ] Emit ContextCompacted event

## Phase 5: Integration & Testing (Week 3)

### Milestone 5.1 — Wire Everything Together
- [ ] Create `src/planner.rs` main struct:
  - [ ] Combine PlannerState, LLM, Compactor, EventBus
  - [ ] Initialize from config
  - [ ] Main run loop consuming from bus
- [ ] Integrate with actors.rs:
  - [ ] Handle user input → plan_tasks
  - [ ] Route ExecutorEvents → step
  - [ ] Publish PlannerCmds to bus
- [ ] Add logging and metrics:
  - [ ] Structured logging for events
  - [ ] Basic metrics (tasks/sec, completion rate)

### Milestone 5.2 — Comprehensive Testing
- [ ] Unit tests for each component:
  - [ ] Event store append/replay
  - [ ] Outbox/inbox patterns
  - [ ] Task planning parser
  - [ ] Step state transitions
- [ ] Integration tests:
  - [ ] Full planning → execution flow
  - [ ] Clarification pause/resume
  - [ ] Event replay determinism
  - [ ] Message deduplication
- [ ] End-to-end test:
  - [ ] Multi-task sequence with clarification
  - [ ] Context compaction under load
  - [ ] Recovery from crash via replay

## Phase 6: Production Readiness (Week 4)

### Milestone 6.1 — Persistence & Reliability
- [ ] Add database-backed event store:
  - [ ] PostgreSQL implementation
  - [ ] Efficient event queries
  - [ ] Index by aggregate_id, timestamp
- [ ] Implement production message bus:
  - [ ] RabbitMQ or Kafka adapter
  - [ ] Connection pooling
  - [ ] Circuit breaker pattern
- [ ] Add monitoring:
  - [ ] Health checks
  - [ ] Event lag metrics
  - [ ] Error rate tracking

### Milestone 6.2 — Performance & Scale
- [ ] Optimize hot paths:
  - [ ] Event batching
  - [ ] Projection caching
  - [ ] Parallel inbox processing
- [ ] Load testing:
  - [ ] Benchmark event throughput
  - [ ] Measure replay performance
  - [ ] Test under high clarification rate
- [ ] Documentation:
  - [ ] API documentation
  - [ ] Deployment guide
  - [ ] Troubleshooting runbook

## Acceptance Criteria
- [ ] System processes user input into tasks and executes them sequentially
- [ ] Clarification requests pause execution until answered
- [ ] Context stays within token limits via compaction
- [ ] State can be fully rebuilt from event log
- [ ] No duplicate task execution under at-least-once delivery
- [ ] All tests pass with >80% coverage
- [ ] Performance: <100ms p99 latency for step processing
- [ ] Reliability: Zero data loss on crash/restart

## Dependencies & Prerequisites
- Rust toolchain (1.70+)
- PostgreSQL or SQLite for event store
- RabbitMQ/Kafka for production (optional for dev)
- Existing meta_agent codebase

## Implementation Order
1. Start with Phase 1 (Foundation) - can be done in parallel
2. Phase 2 (Message Bus) depends on Phase 1
3. Phase 3 (Core Logic) can start after Phase 1
4. Phase 4 (State Management) depends on Phases 2 & 3
5. Phase 5 (Integration) requires all previous phases
6. Phase 6 (Production) is optional for MVP

## Notes for Implementers
- Use in-memory implementations first, then add persistence
- Write tests alongside implementation, not after
- Keep events small - externalize large payloads
- Design for idempotency from the start
- Use structured logging throughout
- Consider feature flags for gradual rollout