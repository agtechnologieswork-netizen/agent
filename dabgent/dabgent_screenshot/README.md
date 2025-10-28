# dabgent_screenshot

Rust screenshot sidecar for web applications using Playwright and Dagger.

## Overview

`dabgent_screenshot` is a hybrid Rust + TypeScript library that captures screenshots of web applications. It uses:
- **Rust** for orchestration, API, and CLI
- **TypeScript/Playwright** for browser automation (embedded in `playwright/` directory)
- **Dagger** for containerized execution

## Features

- Screenshot single apps from Dockerfile
- Batch screenshot multiple apps with controlled concurrency
- Environment variable injection
- Network idle waiting strategy
- Browser console log capture
- Both library API and CLI interface

## Usage

### As a Library

```rust
use dabgent_screenshot::{screenshot_app, ScreenshotOptions};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let options = ScreenshotOptions {
        port: 8000,
        wait_time_ms: 60000,
        env_vars: vec![
            ("DATABRICKS_HOST".to_string(), "https://...".to_string()),
            ("DATABRICKS_TOKEN".to_string(), "token".to_string()),
        ],
        ..Default::default()
    };

    dagger_sdk::connect(|client| async move {
        let app_source = client.host().directory("./my-app");
        let screenshots_dir = screenshot_app(&client, app_source, options).await?;
        screenshots_dir.export("./output").await?;
        Ok(())
    })
    .await?;

    Ok(())
}
```

### As a CLI

```bash
# Screenshot a single app
dabgent-screenshot app \
  --app-source ./my-app \
  --env-vars "DATABRICKS_HOST=https://...,DATABRICKS_TOKEN=token" \
  --port 8000 \
  --wait-time 60000 \
  --output ./screenshots

# Screenshot multiple apps in batch
dabgent-screenshot batch \
  --app-sources ./app1,./app2,./app3 \
  --env-vars "KEY=VALUE" \
  --concurrency 3 \
  --output ./screenshots
```

## API

### `screenshot_app`

Build and screenshot an app from a Dockerfile directory.

```rust
pub async fn screenshot_app(
    client: &DaggerConn,
    app_source: Directory,
    options: ScreenshotOptions,
) -> Result<Directory, ScreenshotError>
```

### `screenshot_service`

Screenshot a running Dagger service.

```rust
pub async fn screenshot_service(
    client: &DaggerConn,
    service: Service,
    options: ScreenshotOptions,
) -> Result<Directory, ScreenshotError>
```

### `screenshot_apps_batch`

Screenshot multiple apps in batch with controlled concurrency.

```rust
pub async fn screenshot_apps_batch(
    client: &DaggerConn,
    app_sources: Vec<Directory>,
    options: ScreenshotOptions,
    concurrency: usize,
) -> Result<Directory, ScreenshotError>
```

## Requirements

- Apps must have a `Dockerfile` in the root directory
- Apps should listen on the port specified by `options.port` (default: 8000)
- Apps should respond to HTTP requests on `/` (or custom URL via `options.url`)

## Output

The screenshot functions return a Dagger `Directory` containing:
- `screenshot.png` - Full page screenshot
- `logs.txt` - Browser console logs

For batch operations, output is organized in subdirectories:
- `app-0/screenshot.png`, `app-0/logs.txt`
- `app-1/screenshot.png`, `app-1/logs.txt`
- etc.

## How It Works

1. Rust code builds a Playwright container with embedded TypeScript tests
2. Rust builds your app container from Dockerfile
3. Rust injects environment variables
4. Rust starts your app as a Dagger service
5. Playwright container connects to your app service
6. Playwright navigates and waits for network idle
7. Playwright captures screenshot and logs

## Tests

```bash
# Unit tests
cargo test -p dabgent_screenshot --lib

# CLI tests
cargo test -p dabgent_screenshot --bin dabgent-screenshot

# Integration tests (requires Dagger)
cargo test -p dabgent_screenshot --test integration -- --ignored
```

## Architecture

```
dabgent_screenshot/
├── src/
│   ├── lib.rs          # Public API
│   ├── main.rs         # CLI entrypoint
│   ├── screenshot.rs   # Core screenshotting logic
│   ├── playwright.rs   # Playwright container builder
│   └── types.rs        # Types and errors
├── playwright/         # Embedded TypeScript/Playwright tests
│   ├── screenshot.spec.ts
│   ├── batch-screenshot.spec.ts
│   └── playwright.*.config.ts
└── tests/
    └── integration.rs  # Integration tests
```

## Migrated from TypeScript

This crate is a Rust port of the TypeScript Dagger module `screenshot-sidecar`, with the following changes:
- **Orchestration**: TypeScript → Rust
- **Browser automation**: TypeScript/Playwright (unchanged, embedded)
- **API**: CLI → Rust library + CLI
- **Integration**: Subprocess → Native Rust function calls
