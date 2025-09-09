# Event-Sourced LLM Planner - MVP

Minimal LLM-powered task planner with event sourcing.

## What It Does

Parses natural language into tasks and saves them as events.

## Quick Start

```bash
# With LLM
cargo run --features mq

# Run tests
cargo test --features mq
```

## Architecture

```
Input → LLM → Tasks → Events → DabGent MQ
```

## Example

```rust
use meta_agent::planner::llm::LLMPlanner;

let planner = LLMPlanner::new(llm, "gpt-4");
let tasks = planner.parse_tasks("Build a web app").await?;
```

That's it. Ship it.
