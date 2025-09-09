# Planning Worker Simplification Analysis

## Comparison of Approaches

### Original PlanningWorker (256 lines)
Complex class with internal state management and direct task execution.

### Simplified PlanningWorker (210 lines)
Following dabgent patterns - spawning workers and using event streams.

### Even Simpler: Just Functions (40 lines in functions module)
Pure functions that coordinate existing workers, no new classes needed.

## Key Simplifications Based on Dabgent Patterns

### 1. **Event-Driven Coordination**
Instead of managing execution directly, the simplified version:
- Spawns independent workers
- Uses event streams for coordination
- Follows the basic.rs example pattern

### 2. **Reuse Existing Workers**
```rust
// Original: Complex internal execution
async fn execute_task(&mut self, task_id: u64, ...) {
    // Direct thread management
    let mut thread = Thread::new();
    // ... complex logic
}

// Simplified: Reuse Worker
tokio::spawn(async move {
    let _ = worker.run(&thread_stream, &aggregate_id).await;
});
```

### 3. **Functional Approach**
The simplest version is just functions:
```rust
// Initialize planning
start_planning(&store, user_input, stream_id, &aggregate_id).await?;

// Spawn worker
let worker = Worker::new(llm, store.clone(), preamble, tools);
tokio::spawn(async move {
    let _ = worker.run(stream_id, &aggregate_id).await;
});

// Monitor completion
let mut receiver = store.subscribe::<PlannerEvent>(&query)?;
// ...
```

## Dabgent Pattern Analysis

### What the Basic Example Shows
```rust
// 1. Spawn workers independently
tokio::spawn(async move {
    let _ = llm_worker.run("basic", "thread").await;
});

// 2. Push initial event
store.push_event("basic", "thread", &event, &Default::default()).await?;

// 3. Monitor progress via subscription
let mut receiver = store.subscribe::<Event>(&query)?;
while let Some(event) = receiver.next().await {
    // Handle events
}
```

### How Simplified Planning Follows This
```rust
// 1. Initialize planning (push events)
start_planning(&store, user_input, stream_id, &aggregate_id).await?;

// 2. Spawn workers
tokio::spawn(async move {
    let _ = worker.run(stream_id, &aggregate_id).await;
});

// 3. Monitor via subscription
let mut receiver = store.subscribe::<PlannerEvent>(&query)?;
```

## Recommendation: Use Functions

The **functions module approach** is the simplest and most aligned with dabgent:

### Advantages
1. **No new classes** - Just coordinate existing components
2. **Follows basic.rs pattern** - Proven approach
3. **Minimal code** - ~40 lines vs 200+
4. **Easy to understand** - Just spawning and monitoring
5. **Flexible** - Can compose functions as needed

### Example Usage
```rust
use dabgent_agent::planning_worker_simplified::functions;

// Simple function call
functions::run_with_planning(
    llm,
    store,
    preamble,
    tools,
    "Build a web app".to_string(),
).await?;
```

## Architecture Comparison

### Complex (Original PlanningWorker)
```
PlanningWorker
├── Internal Worker
├── Internal Planner  
├── Complex state management
├── Direct execution logic
└── 256 lines of code
```

### Simple (Functions)
```
Functions
├── start_planning() - Initialize
├── run_with_planning() - Coordinate
├── Uses existing Worker
├── Uses existing event store
└── 40 lines of code
```

## MQ Library Patterns We Should Follow

### 1. **Event Store as Central Hub**
- All communication via events
- No direct component coupling
- Workers subscribe to relevant events

### 2. **Spawn and Forget**
- Workers run independently
- No complex lifecycle management
- Natural concurrency

### 3. **Simple Subscriptions**
```rust
let query = Query {
    stream_id: "session".to_owned(),
    event_type: Some("TaskDispatched".to_owned()),
    aggregate_id: Some(id.to_owned()),
};
let mut receiver = store.subscribe::<Event>(&query)?;
```

## Final Recommendation

**Delete both PlanningWorker classes** and use the functional approach:

```rust
// planning_functions.rs - The entire implementation
pub async fn plan_and_execute<T, E>(
    llm: T,
    store: E,
    preamble: String,
    tools: Vec<Box<dyn ToolDyn>>,
    user_input: String,
) -> Result<()> 
where
    T: LLMClient + Clone + Send + 'static,
    E: EventStore + Clone + Send + 'static,
{
    let session = Uuid::new_v4().to_string();
    
    // Initialize
    let mut planner = Planner::new();
    let events = planner.process(Initialize { user_input, attachments: vec![] })?;
    for event in events {
        store.push_event("planner", &session, &event, &Default::default()).await?;
    }
    
    // Spawn worker
    let worker = Worker::new(llm, store.clone(), preamble, tools);
    tokio::spawn(async move {
        worker.run("planner", &session).await
    });
    
    // Monitor
    let query = Query {
        stream_id: "planner".to_owned(),
        event_type: Some("PlanningCompleted".to_owned()),
        aggregate_id: Some(session),
    };
    
    let mut rx = store.subscribe::<PlannerEvent>(&query)?;
    if let Some(Ok(PlannerEvent::PlanningCompleted { summary })) = rx.next().await {
        tracing::info!("Done: {}", summary);
    }
    
    Ok(())
}
```

This is **30 lines** of clear, simple code that follows dabgent patterns perfectly.
