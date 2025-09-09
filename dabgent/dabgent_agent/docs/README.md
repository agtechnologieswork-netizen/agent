# Dabgent Planner Documentation

## Quick Start

The planner is an LLM-powered module that converts natural language into executable tasks.

```rust
use dabgent_agent::planner;

// Run planner
planner::runner::run(llm, store, preamble, tools, input).await?
```

## Documentation

- **[PLANNER_DESIGN.md](./PLANNER_DESIGN.md)** - Architecture and design (94 lines)
- **[PLANNER_TASKS.md](./PLANNER_TASKS.md)** - MVP status and metrics (72 lines)

## Status

✅ **MVP Complete** - 1,461 lines of production-ready code

The planner is integrated, tested, and ready to use.
