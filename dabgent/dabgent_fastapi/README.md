# dabgent_fastapi

DataApps Agent Implementation for FastAPI + React Admin applications.

## Overview

This crate provides a specialized agent implementation for generating and evolving FastAPI applications with React Admin frontends, built on top of the `dabgent_agent` framework.

## Features

- **Template Seeding**: Automatically seeds sandbox with DataApps template from `agent/dataapps/template/`
- **Comprehensive Validation**: Multi-stage validator checking Python dependencies, imports, linting, and optional frontend build
- **Export System**: Exports final application artifacts to host filesystem
- **Optimized System Prompt**: Tailored for FastAPI + React Admin development patterns

## Usage

### Running the DataApps Example

```bash
cargo run --example dataapps
```

This will:
1. Build a Docker container with Python 3.12 + Node.js 20 + uv
2. Seed the template files into the sandbox
3. Start LLM and Tool workers
4. Process a sample prompt ("Add a /health endpoint that returns {'status': 'ok'}")
5. Export results to `/tmp/fastapi_output/`

### Customizing the Validator

```rust
use dabgent_fastapi::validator::DataAppsValidator;

let validator = DataAppsValidator::new()
    .with_frontend_check(true)  // Enable frontend build validation
    .with_tests_check(true);    // Enable pytest execution
```

## Architecture

- **Validator**: `src/validator.rs` - Comprehensive DataApps validation logic
- **Example**: `examples/dataapps.rs` - Complete agent runner with seeding and export
- **Dockerfile**: `fastapi.Dockerfile` - Container image with Python + Node.js

## Integration

This crate is designed to be used with:
- `dabgent_agent` - Core agent framework
- `dabgent_sandbox` - Dagger-based sandbox execution
- `dabgent_mq` - Event store for agent communication

## System Prompt

The included system prompt is optimized for:
- React Admin SimpleRestProvider compatibility
- Pydantic models and FastAPI routers
- Incremental, validated changes
- Polars-based data processing patterns