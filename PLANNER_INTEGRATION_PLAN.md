# Planner Integration Plan: Merging meta_agent Planner with dabgent Architecture

## Current State Analysis

### ✅ What's Compatible

**Handler Pattern Alignment:**
- Both use the same `Handler` trait with `process()` and `fold()` methods
- Event sourcing approach is identical
- DabGent MQ integration is already implemented in planner

**Architecture Synergy:**
- Planner uses `dabgent_mq::EventStore` for persistence
- Event types implement `dabgent_mq::models::Event` trait
- Both systems use async/await patterns

**Dependencies:**
- No conflicts in Cargo.toml dependencies
- Both use `rig-core` for LLM interactions
- Shared use of `serde`, `tokio`, `eyre`

### ⚠️ Integration Challenges

**Dual Handler Traits:**
- `dabgent_agent/src/handler.rs` defines base `Handler` trait
- `dabgent_agent/src/planner/handler.rs` redefines `Handler` trait
- **Issue**: Trait conflict and duplication

**Event System Overlap:**
- Thread system has its own `Command`/`Event` enums
- Planner has separate `Command`/`Event` enums  
- **Issue**: Two parallel event systems without coordination

**Worker Architecture Gap:**
- Current `Worker<T, E>` and `ToolWorker<E>` are Thread-focused
- No integration point for Planner events
- **Issue**: Planner events won't trigger existing worker infrastructure

**Module Organization:**
- Planner is self-contained module
- No integration with existing `agent.rs` worker patterns
- **Issue**: Isolated functionality without system integration

## Integration Strategy

### Phase 1: Foundation Alignment (2-4 hours)

#### 1.1 Resolve Handler Trait Conflict
```rust
// Option A: Unify traits (RECOMMENDED)
// Move to dabgent_agent/src/handler.rs
pub trait Handler {
    type Command;
    type Event;
    type Error: std::error::Error + Send + Sync + 'static;

    fn process(&mut self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error>;
    fn fold(events: &[Self::Event]) -> Self;
}

// Option B: Namespace separation
pub mod thread {
    pub trait Handler { /* existing */ }
}
pub mod planner {
    pub trait Handler { /* planner version */ }
}
```

#### 1.2 Update lib.rs Module Structure
```rust
// dabgent_agent/src/lib.rs
pub mod agent;
pub mod handler;    // Unified handler trait
pub mod llm;
pub mod thread;
pub mod toolbox;
pub mod planner;    // Add planner module
```

#### 1.3 Add Planner Feature Flag
```toml
# Cargo.toml
[features]
default = []
mq = ["dabgent_mq"]
planner = ["mq"]    # Planner requires MQ
```

### Phase 2: Event System Integration (4-6 hours)

#### 2.1 Create Event Router
```rust
// dabgent_agent/src/event_router.rs
pub enum SystemEvent {
    Thread(thread::Event),
    Planner(planner::Event),
}

pub struct EventRouter<E: EventStore> {
    event_store: E,
}

impl<E: EventStore> EventRouter<E> {
    pub async fn route_event(&self, stream_id: &str, aggregate_id: &str, event: SystemEvent) -> Result<()> {
        match event {
            SystemEvent::Thread(e) => {
                self.event_store.push_event(stream_id, aggregate_id, &e, &Default::default()).await
            }
            SystemEvent::Planner(e) => {
                self.event_store.push_event(stream_id, aggregate_id, &e, &Default::default()).await
            }
        }
    }
}
```

#### 2.2 Extend Worker Architecture
```rust
// dabgent_agent/src/agent.rs - Add PlannerWorker
pub struct PlannerWorker<T: LLMClient, E: EventStore> {
    llm: T,
    event_store: E,
    planner: planner::Planner,
}

impl<T: LLMClient, E: EventStore> PlannerWorker<T, E> {
    pub async fn run(&mut self, stream_id: &str, aggregate_id: &str) -> Result<()> {
        let query = dabgent_mq::db::Query {
            stream_id: stream_id.to_owned(),
            event_type: Some("PlannerEvent".to_owned()),
            aggregate_id: Some(aggregate_id.to_owned()),
        };
        
        let mut receiver = self.event_store.subscribe::<planner::Event>(&query)?;
        while let Some(event) = receiver.next().await {
            match event? {
                planner::Event::TaskDispatched { task_id, command } => {
                    // Route to appropriate executor based on NodeKind
                    self.dispatch_to_executor(task_id, command).await?;
                }
                _ => continue,
            }
        }
        Ok(())
    }
    
    async fn dispatch_to_executor(&self, task_id: u64, command: planner::PlannerCmd) -> Result<()> {
        match command {
            planner::PlannerCmd::ExecuteTask { kind: planner::NodeKind::ToolCall, parameters, .. } => {
                // Convert to Thread system
                let thread_command = thread::Command::Prompt(parameters);
                // Trigger existing ToolWorker...
            }
            planner::PlannerCmd::RequestClarification { question, .. } => {
                // Handle clarification request...
            }
            _ => {}
        }
        Ok(())
    }
}
```

### Phase 3: Coordinator Integration (6-8 hours)

#### 3.1 Create System Coordinator
```rust
// dabgent_agent/src/coordinator.rs
pub struct SystemCoordinator<T: LLMClient, E: EventStore> {
    llm_worker: Worker<T, E>,
    tool_worker: ToolWorker<E>,
    planner_worker: PlannerWorker<T, E>,
    event_router: EventRouter<E>,
}

impl<T: LLMClient, E: EventStore> SystemCoordinator<T, E> {
    pub async fn run(&mut self, stream_id: &str, aggregate_id: &str) -> Result<()> {
        // Start all workers concurrently
        tokio::try_join!(
            self.llm_worker.run(stream_id, aggregate_id),
            self.tool_worker.run(stream_id, aggregate_id),
            self.planner_worker.run(stream_id, aggregate_id),
        )?;
        Ok(())
    }
    
    pub async fn initialize_planning(&mut self, user_input: String) -> Result<()> {
        let command = planner::Command::Initialize {
            user_input,
            attachments: vec![],
        };
        
        let events = self.planner_worker.planner.process(command)?;
        for event in events {
            self.event_router.route_event("planner", "session", SystemEvent::Planner(event)).await?;
        }
        Ok(())
    }
}
```

#### 3.2 Bridge Planner → Thread Execution
```rust
// dabgent_agent/src/bridges/planner_thread.rs
pub struct PlannerThreadBridge<E: EventStore> {
    event_store: E,
}

impl<E: EventStore> PlannerThreadBridge<E> {
    pub async fn handle_task_dispatch(&self, task_id: u64, command: planner::PlannerCmd) -> Result<()> {
        match command {
            planner::PlannerCmd::ExecuteTask { parameters, kind, .. } => {
                match kind {
                    planner::NodeKind::ToolCall | planner::NodeKind::Processing => {
                        // Convert to Thread system
                        let thread_event = thread::Event::Prompted(parameters);
                        self.event_store.push_event("thread", &task_id.to_string(), &thread_event, &Default::default()).await?;
                    }
                    planner::NodeKind::Clarification => {
                        // Handle clarification differently
                        self.handle_clarification_request(task_id, parameters).await?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
```

### Phase 4: API Integration (4-6 hours)

#### 4.1 Update CLI to Support Both Systems
```rust
// dabgent_agent/src/main.rs
#[derive(clap::Parser)]
struct Args {
    #[clap(subcommand)]
    mode: Mode,
}

#[derive(clap::Subcommand)]
enum Mode {
    /// Run traditional thread-based agent
    Thread {
        prompt: String,
    },
    /// Run planner-based agent (MVP)
    Plan {
        input: String,
    },
    /// Run full coordinated system
    Coordinate {
        input: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    match args.mode {
        Mode::Thread { prompt } => {
            // Existing thread-based execution
            run_thread_mode(prompt).await
        }
        Mode::Plan { input } => {
            // Planner-only execution
            run_planner_mode(input).await
        }
        Mode::Coordinate { input } => {
            // Full coordinated execution
            run_coordinated_mode(input).await
        }
    }
}
```

#### 4.2 Create Unified API
```rust
// dabgent_agent/src/api.rs
pub struct DabgentAgent<T: LLMClient, E: EventStore> {
    coordinator: SystemCoordinator<T, E>,
}

impl<T: LLMClient, E: EventStore> DabgentAgent<T, E> {
    pub async fn plan_and_execute(&mut self, input: String) -> Result<String> {
        // Initialize planning
        self.coordinator.initialize_planning(input).await?;
        
        // Run coordinated execution
        self.coordinator.run("session", "main").await?;
        
        // Return results
        self.get_execution_summary().await
    }
    
    pub async fn simple_execute(&mut self, prompt: String) -> Result<String> {
        // Direct thread execution (existing behavior)
        // ...
    }
}
```

## Implementation Priorities

### 🚀 Immediate (This Week)
1. **Resolve Handler trait conflict** - Critical blocker
2. **Add planner module to lib.rs** - Basic integration
3. **Create feature flag for planner** - Optional functionality
4. **Basic CLI integration** - Demonstrate functionality

### 📅 Next Sprint (Week 2)
1. **Event router implementation** - System coordination
2. **PlannerWorker creation** - Bridge to existing workers
3. **Basic planner → thread bridge** - Task execution
4. **Integration tests** - Verify end-to-end flow

### 🔄 Future Iterations (Month 2)
1. **Full coordinator implementation** - Production ready
2. **Advanced bridging logic** - Complex task routing
3. **Performance optimization** - Concurrent execution
4. **Monitoring and observability** - Production deployment

## Risk Mitigation

### Technical Risks
- **Event system complexity**: Start with simple routing, add complexity gradually
- **Performance impact**: Use feature flags to enable/disable planner
- **State synchronization**: Use DabGent MQ as single source of truth

### Integration Risks
- **Breaking changes**: Maintain backward compatibility for existing Thread system
- **Test coverage**: Add integration tests before major refactoring
- **Documentation**: Keep both systems documented during transition

## Success Criteria

### Phase 1 Success
- [ ] No compilation errors
- [ ] Both systems can run independently
- [ ] Feature flags work correctly
- [ ] Basic CLI supports both modes

### Phase 2 Success
- [ ] Planner events trigger thread execution
- [ ] Task dispatch works end-to-end
- [ ] Event routing handles both event types
- [ ] Integration tests pass

### Phase 3 Success
- [ ] Full coordinated execution works
- [ ] Complex tasks execute correctly
- [ ] System handles failures gracefully
- [ ] Performance is acceptable

### Final Success
- [ ] Single API supports both simple and complex execution
- [ ] Production ready with monitoring
- [ ] Documentation complete
- [ ] Migration path clear for existing users

## Migration Strategy

### For Existing Thread Users
1. **No breaking changes** - existing API continues to work
2. **Opt-in planner features** - enable via feature flags
3. **Gradual migration** - move complex tasks to planner over time

### For New Users
1. **Unified API** - single entry point for all functionality
2. **Smart routing** - system chooses best execution path
3. **Progressive complexity** - start simple, add planning as needed

This integration plan provides a clear path to merge the planner functionality into the existing dabgent architecture while maintaining backward compatibility and enabling future enhancements.
