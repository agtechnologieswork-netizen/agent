use dabgent_agent::agent::{self};
use dabgent_agent::handler::Handler;
use dabgent_agent::thread::{self};
use dabgent_fastapi::{validator::DataAppsValidator, toolset::dataapps_toolset};
use dabgent_mq::EventStore;
use dabgent_mq::db::{Query, sqlite::SqliteStore};
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::Sandbox;
use eyre::Result;
use rig::client::ProviderClient;
use std::path::Path;
use std::fs;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();
    run().await;
}

async fn run() {
    ConnectOpts::default().connect(|client| async move {
        run_main_logic(client).await
    })
    .await
    .unwrap();
}

async fn run_main_logic(client: dagger_sdk::DaggerConn) -> Result<()> {
        tracing::info!("Connected to Dagger, creating sandbox...");

        let mut sandbox_instance = sandbox(&client).await?;
        tracing::info!("Sandbox created successfully");
        let store = store().await;
        tracing::info!("Event store initialized");

        // Seed template files into sandbox
        tracing::info!("Starting template seeding...");
        seed_dataapps_template(&mut sandbox_instance).await?;
        tracing::info!("Template seeding completed successfully");

        let anthropic_client = rig::providers::anthropic::Client::from_env();

        let validator = DataAppsValidator::new();
        let tools = dataapps_toolset(validator.clone());
        let llm_worker = agent::Worker::new(anthropic_client, store.clone(), DATAAPPS_SYSTEM_PROMPT.to_owned(), tools.clone());

        let sandbox_boxed = sandbox_instance.boxed();
        let mut sandbox_worker = agent::ToolWorker::new(sandbox_boxed, store.clone(), tools);

        // No need for a separate export sandbox anymore - ToolWorker will handle it

        tokio::spawn(async move {
            let _ = llm_worker.run("dataapps", "thread").await;
        });
        tokio::spawn(async move {
            let _ = sandbox_worker.run("dataapps", "thread").await;
        });

        // Start the CompactWorker to process ToolCompletedRaw events
        let mut compact_worker = agent::CompactWorker::new(store.clone());
        tokio::spawn(async move {
            let _ = compact_worker.run("dataapps", "thread").await;
        });

        let event = thread::Event::Prompted(
            "Build a simple counter web app where users can create, increment, and decrement a single global counter".to_owned(),
        );
        store
            .push_event("dataapps", "thread", &event, &Default::default())
            .await?;

        let query = Query {
            stream_id: "dataapps".to_owned(),
            event_type: None,
            aggregate_id: Some("thread".to_owned()),
        };

        let mut receiver = store.subscribe::<thread::Event>(&query)?;
        let mut events = store.load_events(&query, None).await?;
        while let Some(event) = receiver.next().await {
            let event = event?;
            events.push(event.clone());
            let thread = thread::Thread::fold(&events);
            tracing::info!(?thread.state, ?event, "event");
            match thread.state {
                thread::State::Done | thread::State::UserWait => {
                    // Trigger export via synthetic tool call event
                    let export_call = rig::message::ToolCall {
                        id: "export_task".to_string(),
                        call_id: None,
                        function: rig::message::ToolFunction {
                            name: "export_artifacts".to_string(),
                            arguments: serde_json::json!({
                                "path": "/tmp/fastapi_output"
                            }),
                        },
                    };

                    let export_event = thread::Event::LlmCompleted(dabgent_agent::llm::CompletionResponse {
                        choice: rig::OneOrMany::one(rig::message::AssistantContent::ToolCall(export_call)),
                        finish_reason: dabgent_agent::llm::FinishReason::ToolUse,
                        output_tokens: 0,
                    });

                    store
                        .push_event("dataapps", "thread", &export_event, &Default::default())
                        .await?;

                    // Wait for export to complete before exiting
                    tracing::info!("Waiting for export to complete...");
                },
                thread::State::Tool if thread.messages.last().map(|m| {
                    if let rig::message::Message::User { content } = m {
                        content.iter().any(|c| {
                            if let rig::message::UserContent::ToolResult(tr) = c {
                                tr.id == "export_task"
                            } else {
                                false
                            }
                        })
                    } else {
                        false
                    }
                }).unwrap_or(false) => {
                    // Export completed, we can exit
                    tracing::info!("Export completed successfully");
                    break;
                },
                _ => continue,
            }
        }

        Ok(())
}

async fn sandbox(client: &dagger_sdk::DaggerConn) -> Result<DaggerSandbox> {
    tracing::info!("Building container from Dockerfile...");
    let opts = dagger_sdk::ContainerBuildOptsBuilder::default()
        .dockerfile("fastapi.Dockerfile")
        .build()?;
    let ctr = client
        .container()
        .build_opts(client.host().directory("dabgent_fastapi"), opts);
    tracing::info!("Syncing container...");
    ctr.sync().await?;
    tracing::info!("Container sync completed");
    let sandbox = DaggerSandbox::from_container(ctr);
    Ok(sandbox)
}

async fn store() -> SqliteStore {
    let pool = sqlx::SqlitePool::connect(":memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}

async fn seed_dataapps_template(sandbox: &mut DaggerSandbox) -> Result<()> {
    let template_path = Path::new("../dataapps/template_minimal");

    // Verify template path exists
    if !template_path.exists() {
        return Err(eyre::eyre!("Template path does not exist: {:?}", template_path.canonicalize()));
    }

    tracing::info!("Seeding template from: {:?}", template_path.canonicalize());

    // Collect all files first, then write them in bulk
    let mut files = Vec::new();
    collect_files_recursive(template_path, "/app", &mut files)?;

    tracing::info!("Collected {} files for bulk write", files.len());

    sandbox.write_files_bulk(files).await?;
    tracing::info!("Template files written");

    Ok(())
}

fn collect_files_recursive(
    host_path: &Path,
    sandbox_base_path: &str,
    files: &mut Vec<(String, String)>,
) -> Result<()> {
    if host_path.is_dir() {
        for entry in fs::read_dir(host_path)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            let host_file_path = entry.path();
            let sandbox_file_path = format!("{}/{}", sandbox_base_path, file_name);

            let bad_patterns = ["node_modules", ".git", ".venv", "__pycache__"];
            if bad_patterns.iter().any(|pattern| file_name.contains(pattern)) {
                continue; // Skip unwanted directories
            }

            if host_file_path.is_dir() {
                // Recursively collect files from subdirectories
                collect_files_recursive(&host_file_path, &sandbox_file_path, files)?;
            } else {
                // Read file as bytes first, then check if it's valid UTF-8
                match fs::read(&host_file_path) {
                    Ok(bytes) => {
                        match String::from_utf8(bytes) {
                            Ok(content) => {
                                // File is valid UTF-8, add it to the collection
                                files.push((sandbox_file_path, content));
                            }
                            Err(_) => {
                                // File is binary, skip it for now
                                tracing::info!("Skipping binary file: {}", host_file_path.display());
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read file {}: {}", host_file_path.display(), e);
                    }
                }
            }
        }
    }
    Ok(())
}

// Export functionality moved to toolbox::basic::export_artifacts
// and is now triggered via events

const DATAAPPS_SYSTEM_PROMPT: &str = r#"
You are a senior FastAPI + React Admin application engineer with expertise in Polars, Pydantic, and modern Python practices.

WORKING DIRECTORY: /app (contains the FastAPI + React Admin template)

YOUR TASK:
Make targeted, incremental edits to build data-driven web applications. The template includes:
- backend/ - FastAPI application with example customer resource
- frontend/reactadmin/ - React Admin UI (pre-built)
- Root files: requirements.txt, package.json, app.yaml (Databricks Apps config)

CRITICAL CONSTRAINTS:
1. **React Admin Compatibility**: ALL list endpoints MUST return proper headers:
   - Content-Range: "items {start}-{end}/{total}"
   - X-Total-Count: {total_count}

2. **Data Models**: Use Pydantic models with proper typing. Follow existing patterns in backend/models.py

3. **Routers**: Use structured FastAPI routers in backend/routers/. Keep customer router as reference.

4. **File Operations**:
   - Prefer edit_file for small changes (find/replace pattern)
   - Use write_file only when creating new files
   - WriteFile format: {"path": "...", "contents": "..."}

5. **Validation**: Always call "done" tool when you believe your implementation is complete.
   The validator will check:
   - Python import success (backend.main:app)
   - Code linting (ruff)
   - Dependencies install (uv sync)

6. **Preserve Infrastructure**:
   - Keep app.mount("/", StaticFiles(...)) for serving React Admin
   - Don't modify package.json or app.yaml unless explicitly required
   - Maintain uv/requirements.txt for Python dependencies

DEVELOPMENT APPROACH:
- Start with backend changes (models, routers)
- Test incrementally using the "done" tool
- Use Polars for data processing when working with datasets
- Follow existing code patterns and naming conventions

Remember: Small, working changes that pass validation are better than large changes that fail.
"#;
