# Dabgent Codebase Deduplication Plan

## Overview
Analysis of the dabgent codebase revealed multiple logical duplications that can be consolidated to reduce maintenance burden and improve consistency.

## Priority 1: Major Structural Duplications

### 1. Event Processing Loop Pattern
**Impact:** High - Core functionality duplicated
**Files Affected:**
- `dabgent_agent/src/agent.rs:26-54`
- `dabgent_agent/src/planner/runner.rs:65-77`

**Current Duplication:**
```rust
// Pattern repeated in multiple places
let query = dabgent_mq::db::Query { ... };
let mut receiver = store.subscribe::<Event>(&query)?;
while let Some(event) = receiver.next().await {
    // Event processing logic
    store.push_event(stream_id, aggregate_id, event, &Default::default()).await?;
}
```

**Solution:**
- Create `dabgent_mq::utils::EventProcessor` helper
- Extract common event loop patterns
- Standardize error handling

### 2. Handler Implementation Boilerplate
**Impact:** High - Structural pattern duplication
**Files Affected:**
- `dabgent_agent/src/thread.rs:5-50`
- `dabgent_agent/src/planner/handler.rs:249-540`

**Current Duplication:**
```rust
impl Handler for X {
    type Command = Command;
    type Event = Event;
    type Error = XError;
    
    fn process(&mut self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        match command { ... }
    }
    
    fn fold(events: &[Self::Event]) -> Self {
        let mut instance = Self::new();
        for event in events { ... }
        instance
    }
}
```

**Solution:**
- Create Handler implementation macros
- Extract common fold patterns
- Standardize state machine patterns

## Priority 2: API Usage Duplications

### 3. Event Store Operations
**Impact:** Medium - Repeated API calls
**Files Affected:**
- `dabgent_agent/src/agent.rs:45-48`
- `dabgent_agent/src/planner/runner.rs:53`
- Multiple test files

**Current Duplication:**
```rust
// Repeated everywhere
store.push_event(stream_id, aggregate_id, event, &Default::default()).await?;
store.load_events::<Event>(&query, None).await?;
store.subscribe::<Event>(&query)?;
```

**Solution:**
- Create `EventStoreExt` trait with helper methods
- Add `push_event_simple()`, `load_all_events()`, `subscribe_simple()`
- Standardize metadata usage

### 4. Query Construction Pattern
**Impact:** Medium - Repeated boilerplate
**Files Affected:**
- `dabgent_agent/src/agent.rs:27-31`
- `dabgent_agent/src/planner/runner.rs:66-70`
- Test files

**Current Duplication:**
```rust
let query = dabgent_mq::db::Query {
    stream_id: stream_id.to_owned(),
    event_type: Some/None,
    aggregate_id: Some(aggregate_id.to_owned()),
};
```

**Solution:**
- Create `QueryBuilder` with fluent API
- Add convenience constructors
- Reduce string cloning

### 5. LLM Completion Pattern
**Impact:** Medium - API usage duplication
**Files Affected:**
- `dabgent_agent/src/agent.rs:66`
- `dabgent_agent/src/planner/llm.rs:118,233,292,329`

**Current Duplication:**
```rust
let response = self.llm.completion(completion).await?;
```

**Solution:**
- Create LLM helper with error handling
- Standardize response processing
- Add retry logic if needed

## Priority 3: Minor Duplications

### 6. UUID Session Generation
**Impact:** Low - Simple pattern
**Files Affected:**
- `dabgent_agent/src/planner/runner.rs:44`
- Various tests

**Current Duplication:**
```rust
let id = Uuid::new_v4().to_string();
```

**Solution:**
- Create session ID utility function
- Standardize format (with/without hyphens)

### 7. Error Enum Structures
**Impact:** Low - Similar patterns
**Files Affected:**
- `dabgent_agent/src/planner/handler.rs:72-82`
- Likely similar in thread.rs

**Current Duplication:**
```rust
#[derive(Debug, thiserror::Error)]
pub enum XError {
    #[error("Invalid command: {0}")]
    InvalidCommand(String),
    // ...
}
```

**Solution:**
- Create common error types where appropriate
- Standardize error message formats

### 8. Timeout Handling Pattern
**Impact:** Low - Could be reused
**Files Affected:**
- `dabgent_agent/src/planner/runner.rs:79-81`

**Current Pattern:**
```rust
timeout(Duration::from_secs(timeout_secs), fut)
    .await
    .map_err(|_| eyre::eyre!("Timeout after {} seconds", timeout_secs))?
```

**Solution:**
- Create timeout utility function
- Standardize timeout error messages

## Implementation Strategy

### Phase 1: Infrastructure (Week 1)
1. Create `dabgent_mq::utils` module
2. Implement `EventProcessor` helper
3. Create `EventStoreExt` trait
4. Add `QueryBuilder`

### Phase 2: Core Deduplication (Week 2)
1. Refactor Agent and Planner to use new utilities
2. Update Handler implementations
3. Consolidate LLM completion patterns

### Phase 3: Polish (Week 3)
1. Update tests to use new patterns
2. Clean up minor duplications
3. Update documentation

## Expected Benefits

### Code Reduction
- **Estimated 200-300 lines removed** across duplications
- **Improved maintainability** - changes in one place
- **Consistent error handling** across modules

### Performance
- **Reduced allocations** in query building
- **Standardized connection pooling** in event store
- **Consistent timeout handling**

### Developer Experience
- **Less boilerplate** when adding new Handlers
- **Consistent APIs** across modules
- **Better error messages**

## Risk Assessment

### Low Risk
- Event store utilities (backwards compatible)
- Query builder (additive)
- UUID utilities (simple)

### Medium Risk
- Handler implementation changes (affects core logic)
- Event processing refactor (touches main loops)

### Mitigation
- Implement incrementally
- Maintain backwards compatibility
- Extensive testing at each phase
- Feature flags for new utilities

## Success Metrics

1. **Lines of Code:** 200-300 line reduction
2. **Test Coverage:** Maintain 100% coverage
3. **Performance:** No regression in benchmarks
4. **API Consistency:** All modules use same patterns
5. **Documentation:** Updated examples and docs

## Files to Create

1. `dabgent_mq/src/utils/mod.rs` - Event processing utilities
2. `dabgent_mq/src/utils/event_processor.rs` - Event loop helper
3. `dabgent_mq/src/utils/query_builder.rs` - Query construction helper
4. `dabgent_agent/src/utils/mod.rs` - Agent utilities
5. `dabgent_agent/src/utils/llm_helpers.rs` - LLM completion helpers
6. `dabgent_agent/src/utils/session.rs` - Session ID utilities

## Migration Path

Each deduplication will be implemented as:
1. **Add new utility** (backwards compatible)
2. **Update one module** to use new utility
3. **Test thoroughly**
4. **Update remaining modules**
5. **Remove old duplicated code**
6. **Update documentation**

This ensures no breaking changes and allows rollback at any step.
