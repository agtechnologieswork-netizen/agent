# Event-Sourced LLM Planner — MVP Design

**Goal**: Barebone LLM-powered planner with event sourcing. Nothing more.

## Core MVP Scope (What We Built)

### 1. Handler Pattern ✅
```rust
pub trait Handler {
    fn process(command) -> Result<Vec<Event>>;
    fn fold(events) -> Self;
}
```

### 2. LLM Task Parsing ✅
- Natural language → structured tasks
- Single endpoint: `parse_tasks()`
- XML format for reliability

### 3. Event Sourcing ✅
- Commands in, Events out
- State rebuilt from events
- DabGent MQ for persistence

## What's Cut from MVP

❌ **Cut Features:**
- Attachment processing beyond basic extraction
- Context compaction strategies
- Dependency analysis
- Task routing logic
- Parallel execution
- Checkpoint/restore
- Multi-agent coordination
- Vector stores
- RAG integration
- Monitoring/metrics
- A/B testing
- Time-travel debugging beyond basic replay

## Minimal Working System

```
User Input → LLM Parser → Events → DabGent MQ
                ↓
            Task List
                ↓
         Execute (external)
                ↓
            Results → Events
```

## Three Essential Files

### 1. `handler.rs` - Core Pattern
- Handler trait
- Command/Event enums
- Basic state management

### 2. `llm.rs` - Intelligence
- Parse natural language
- Return structured tasks
- That's it

### 3. `mq.rs` - Persistence
- Save events
- Load events
- Subscribe to streams

## Usage

```rust
// That's all folks
let planner = LLMPlanner::new(llm, "gpt-4");
let tasks = planner.parse_tasks("Build a web app").await?;
let events = vec![Event::TasksPlanned { tasks }];
store.push_event("planner", session_id, &events[0]).await?;
```

## Not in MVP

Everything else. Seriously. If it's not:
1. Parsing text to tasks (LLM)
2. Storing/loading events (DabGent MQ)
3. Basic state tracking (Handler)

Then it's not in MVP.

## Next After MVP

Only after MVP works end-to-end:
1. Task execution integration
2. Result handling
3. Context management
4. Everything else in the grand vision

## Success Criteria

✅ Can parse "Build X" into tasks
✅ Can save tasks as events
✅ Can rebuild state from events
✅ Tests pass

That's MVP. Ship it.
