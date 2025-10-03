# FinishHandler - Artifact Export for Dabgent Agents

The `FinishHandler` automatically exports artifacts created by your agent when it completes its task.

## Features

- **Automatic Export**: Triggers when agent emits a "finished" or "done" event
- **Sandbox State Replay**: Rebuilds complete sandbox state by replaying all tool executions
- **Git-Aware Export**: Respects `.gitignore` files using git checkout-index
- **Fallback Strategy**: Falls back to direct copy if git export fails
- **Detailed Logging**: Comprehensive trace/debug logging for troubleshooting

## How It Works

1. **Event Detection**: Listens for agent events with "finished" or "done" in the event type
2. **Sandbox Retrieval**: Gets existing sandbox or creates fresh one from template
3. **State Replay**: Replays all tool executions to rebuild files/state
4. **Git Export**: Uses git to filter out ignored files (node_modules, .pyc, etc.)
5. **Artifact Export**: Copies filtered files to host filesystem

## Usage

### Basic Setup

```rust
use dabgent_agent::processor::finish::FinishHandler;
use dabgent_agent::processor::tools::TemplateConfig;
use dabgent_sandbox::SandboxHandle;

// Create your tools
let tools = toolset(MyValidator);
let tools_for_finish = toolset(MyValidator);  // Separate copy for finish handler

// Configure sandbox and template
let sandbox_handle = SandboxHandle::new(Default::default());
let template_config = TemplateConfig::default_dir("./examples");

// Create finish handler
let finish_handler = FinishHandler::new(
    sandbox_handle.clone(),
    "./output".to_string(),           // Export path
    tools_for_finish,
    template_config.clone(),
);

// Add to runtime
let runtime = Runtime::<MyAgent, _>::new(store, ())
    .with_handler(llm_handler)
    .with_handler(tool_handler)
    .with_handler(finish_handler)     // Add finish handler
    .with_handler(log_handler);
```

### Agent Events

Your agent must emit a "finished" or "done" event:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MyAgentEvent {
    Finished,  // or Done, TaskComplete, etc.
}

impl MQEvent for MyAgentEvent {
    fn event_type(&self) -> String {
        match self {
            MyAgentEvent::Finished => "finished".to_string(),
        }
    }

    fn event_version(&self) -> String {
        "1.0".to_string()
    }
}

// In handle_tool_results, emit the finished event:
async fn handle_tool_results(
    state: &AgentState<Self>,
    _: &Self::Services,
    incoming: Vec<ToolResult>,
) -> Result<Vec<Event<Self::AgentEvent>>, Self::AgentError> {
    // Check if "done" tool was called
    if let Some(done_id) = &state.agent.done_call_id {
        if let Some(result) = incoming.iter().find(|r| &r.id == done_id) {
            return Ok(vec![Event::Agent(MyAgentEvent::Finished)]);
        }
    }
    // ... continue processing
}
```

## Export Process Details

### 1. Sandbox State Replay

The FinishHandler replays all tool executions to ensure complete state:

```rust
// Get or create sandbox
let mut sandbox = match self.sandbox_handle.get(aggregate_id).await? {
    Some(s) => s,  // Use existing
    None => {
        // Create fresh from template
        self.sandbox_handle.create_from_directory(
            aggregate_id,
            &self.template_config.host_dir,
            &self.template_config.dockerfile,
        ).await?
    }
};

// Replay all events
let events = handler.store().load_events::<AgentState<A>>(aggregate_id).await?;
let mut replayer = SandboxReplayer::new(&mut sandbox, &self.tools);
replayer.apply_all(&events).await?;
```

### 2. Git-Aware Export

Uses git to filter out ignored files:

```bash
# Inside sandbox:
git -C /app init
git -C /app add -A                                    # Respects .gitignore
git -C /app checkout-index --all --prefix=/output/   # Copy tracked files
```

### 3. Fallback Strategy

If git export fails, falls back to direct copy:

```bash
cp -r /app/* /output/
```

## Tool Replay Control

Tools can opt out of replay with `needs_replay()`:

```rust
impl Tool for MyReadOnlyTool {
    fn needs_replay(&self) -> bool {
        false  // Don't replay read-only operations
    }

    // ... other methods
}
```

Only tools that modify sandbox state (write files, run commands) need replay.

## Configuration

### Export Path

Specify where artifacts are exported:

```rust
FinishHandler::new(
    sandbox_handle,
    "./output".to_string(),        // Relative to working directory
    tools,
    template_config,
);
```

### Template Configuration

Configure the sandbox template:

```rust
let template_config = TemplateConfig::new(
    "./my-template".to_string(),   // Directory with Dockerfile
    "Dockerfile".to_string(),       // Dockerfile name
);
```

## Example: Complete Agent with Export

See `examples/basic_with_export.rs` for a complete working example:

```bash
# Run the example
cd dabgent_agent
export ANTHROPIC_API_KEY=your_key
cargo run --example basic_with_export

# Check exported artifacts
ls -la output/
cat output/main.py
```

## Troubleshooting

### No artifacts exported

Check that:
1. Agent emits a "finished" or "done" event
2. Event type contains "finished" or "done" in the string
3. Tools have `needs_replay() = true` for state-modifying operations

### Export path errors

Ensure parent directories exist or use absolute paths:

```rust
let export_path = std::env::current_dir()?
    .join("output")
    .to_string_lossy()
    .to_string();
```

### Enable detailed logging

```bash
RUST_LOG=dabgent_agent=debug cargo run --example basic_with_export
```

This shows:
- Sandbox state replay progress
- Git commands and their output
- File contents before/after export
- Export success/failure details

## Advanced Usage

### Custom Export Logic

Extend `FinishHandler` or implement your own `EventHandler`:

```rust
impl<A: Agent, ES: EventStore> EventHandler<A, ES> for MyCustomFinisher {
    async fn process(
        &mut self,
        handler: &Handler<AgentState<A>, ES>,
        envelope: &Envelope<AgentState<A>>,
    ) -> Result<()> {
        // Custom logic here
        Ok(())
    }
}
```

### Multiple Export Locations

Create multiple finish handlers with different export paths:

```rust
let finish_code = FinishHandler::new(sandbox.clone(), "./code", tools.clone(), cfg.clone());
let finish_docs = FinishHandler::new(sandbox.clone(), "./docs", tools.clone(), cfg.clone());

runtime
    .with_handler(finish_code)
    .with_handler(finish_docs)
```

## Integration with CI/CD

The exported artifacts can be used in CI/CD pipelines:

```bash
# After agent completes
cargo run --release --example my_agent

# Artifacts in output/
cd output
git init
git add .
git commit -m "Generated by agent"
git push origin generated-code
```

## Performance Considerations

- **Replay Time**: Proportional to number of tool executions
- **Sandbox Creation**: First export is slower (creates sandbox from template)
- **Export Size**: Filtered by .gitignore, typically much smaller than full sandbox

## Security

- Sandbox isolation prevents access to host filesystem during execution
- Only specified export directory receives artifacts
- Git filtering prevents accidental export of secrets in .env, etc.
- All tool executions are logged in event store for audit

## See Also

- `replay.rs` - Tool execution replay logic
- `tools.rs` - ToolHandler and sandbox management
- `examples/basic_with_export.rs` - Complete working example
