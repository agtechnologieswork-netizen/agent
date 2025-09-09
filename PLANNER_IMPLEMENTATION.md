# Dabgent Planner - Unified Implementation Plan

## 🎯 Current Status

### ✅ Completed (MVP Shipped)
- **Core Handler Pattern** - Event sourcing with process/fold
- **LLM Task Parsing** - Natural language → structured tasks  
- **DabGent MQ Integration** - Event persistence and subscriptions
- **Minimal Runner** - 81 lines with timeout support
- **Tests** - 6 tests passing (integration + e2e)
- **Examples** - `examples/planning.rs` 
- **Documentation** - Minimal README with API usage

### 📊 Metrics
- **Total Lines**: 1,461 (handler: 567, types: 340, llm: 419, runner: 81, mq: 27, mod: 27)
- **Optimization**: Saved 12 lines in LLM module through deduplication
- **Test Coverage**: Integration and e2e tests passing
- **Simplification**: Removed 97 lines (CLI), consolidated docs

## 🔥 Priority 1: Deduplication (Immediate)

### Event Processing Pattern (Save ~50 lines)
**Current Duplication:**
- `agent.rs`: Event loop with subscription
- `planner/runner.rs`: Similar event loop

**Action:**
```rust
// Create dabgent_mq/src/utils/event_loop.rs
pub async fn process_events<E, F>(
    store: &impl EventStore,
    query: Query,
    handler: F,
) -> Result<()>
where
    E: Event,
    F: Fn(E) -> Result<()>
```

### LLM Completion Helper (Save ~30 lines)
**Current Duplication:**
- Multiple `self.llm.completion().await?` calls in `planner/llm.rs`

**Action:**
```rust
// Add to planner/llm.rs
async fn llm_complete(&self, prompt: String) -> Result<String> {
    let completion = /* build completion */;
    let response = self.llm.completion(completion).await?;
    extract_text(response)
}
```

## 🚀 Priority 2: MVP Completion (Today)

### 1. Simplify Error Handling (15 min)
- Consolidate error types in `planner/handler.rs`
- Use `eyre::Result` consistently
- Remove unused error variants

### 2. Complete CLI Demo (30 min)
```rust
// examples/planner_cli.rs
use dabgent_agent::planner;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<()> {
    let input = prompt_user("What would you like to build? ")?;
    planner::runner::run(llm, store, preamble, tools, input).await?;
    Ok(())
}
```

### 3. Add Minimal Documentation (15 min)
```markdown
# Planner Usage

## Quick Start
cargo run --example planner_cli

## API
planner::runner::run(llm, store, preamble, tools, input).await

## Events
- TasksPlanned: Initial plan created
- TaskDispatched: Task sent for execution
- PlanningCompleted: All done
```

## 📋 Priority 3: Clean Architecture (This Week)

### 1. Extract Event Store Utils
```rust
// dabgent_mq/src/utils.rs
pub trait EventStoreExt {
    async fn push_simple(&self, stream: &str, id: &str, event: impl Event) -> Result<()>;
    fn subscribe_simple<E: Event>(&self, stream: &str, id: &str) -> Result<Receiver<E>>;
}
```

### 2. Consolidate Query Building
```rust
// dabgent_mq/src/query_builder.rs
impl Query {
    pub fn for_stream(stream: &str) -> Self { ... }
    pub fn with_aggregate(mut self, id: &str) -> Self { ... }
    pub fn with_event_type(mut self, event_type: &str) -> Self { ... }
}
```

### 3. Remove Test Duplication
- Extract mock LLM client to test utils
- Share event store setup across tests

## ❌ Not Now (Phase 2+)

### Complex Features (2-4 months)
- Task execution framework
- Dependency analysis  
- Parallel execution
- Context compaction
- Vector stores / RAG
- Monitoring / metrics
- UI / visualization

### Keep It Simple
- No new abstractions
- No feature creep
- No premature optimization

## 📈 Success Metrics

### Code Quality
- [ ] Under 1,400 total lines (currently 1,416)
- [ ] No logical duplication
- [ ] All tests passing

### Functionality  
- [ ] CLI demo works end-to-end
- [ ] Can parse any user request
- [ ] Events properly persisted

### Documentation
- [ ] 10-line README
- [ ] Example that runs
- [ ] Clear API docs

## 🎬 Implementation Steps (Next 2 Hours)

### Hour 1: Deduplication
1. [ ] Create event loop utility (15 min)
2. [ ] Apply to runner.rs (10 min)
3. [ ] Create LLM helper (15 min)
4. [ ] Apply to llm.rs (10 min)
5. [ ] Test everything (10 min)

### Hour 2: MVP Polish  
1. [ ] Create CLI example (30 min)
2. [ ] Write minimal docs (15 min)
3. [ ] Clean up errors (15 min)

## 🚦 Go/No-Go Criteria

### Ship If:
- ✅ Tests pass
- ✅ Example runs
- ✅ Under 1,400 lines

### Don't Ship If:
- ❌ Adding new features
- ❌ Over-engineering
- ❌ Breaking existing code

## 📝 Final Checklist

- [x] Deduplication complete (12 lines saved in LLM)
- [x] Documentation exists (`PLANNER_README.md`)
- [x] Tests passing (6 tests, all green)
- [x] Example runs (`planning.rs` )
- [x] No feature creep (stayed focused on MVP)

---

**Next Action**: Start with event loop deduplication in dabgent_mq
