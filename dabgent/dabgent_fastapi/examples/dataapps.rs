use dabgent_agent::pipeline::PipelineBuilder;
use dabgent_fastapi::{toolset::dataapps_toolset, validator::DataAppsValidator};
use dabgent_mq::{EventStore, create_store, StoreConfig};
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::Sandbox;
use eyre::Result;
use rig::client::ProviderClient;
use std::path::Path;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    const STREAM_ID: &str = "dataapps";
    const AGGREGATE_ID: &str = "thread";

    println!("🔧 Setting up Dagger connection...");
    let opts = ConnectOpts::default();
    opts.connect(|client| async move {
        let llm = rig::providers::anthropic::Client::from_env();
        let sandbox = sandbox(&client).await?;
        let store = create_store(Some(StoreConfig::from_env())).await?;
        let tools = dataapps_toolset(DataAppsValidator::new());

        push_prompt(&store, STREAM_ID, AGGREGATE_ID, USER_PROMPT).await?;

        tracing::info!("Starting DataApps pipeline with model: {}", MODEL);

        let pipeline = PipelineBuilder::new()
            .llm(llm)
            .store(store)
            .sandbox(sandbox.boxed())
            .model(MODEL.to_owned())
            .preamble(SYSTEM_PROMPT.to_owned())
            .tools(tools)
            .build()?;

        tracing::info!("Pipeline configured, starting execution...");

        pipeline
            .run(STREAM_ID.to_owned(), AGGREGATE_ID.to_owned())
            .await
    })
    .await
    .unwrap();
}

const SYSTEM_PROMPT: &str = "
You are a FastAPI and React developer creating data applications.

Workspace Setup:
- You have a pre-configured DataApps project structure in /app with backend and frontend directories
- Backend is in /app/backend with Python, FastAPI, and uv package management
- Frontend is in /app/frontend with React Admin and TypeScript
- Use 'uv run' for all Python commands (e.g., 'uv run python main.py')

Your Task:
1. Create a simple data API with one endpoint that returns sample data
2. Configure React Admin UI to display this data in a table
3. Add proper logging and debugging throughout
4. Ensure CORS is properly configured for React Admin

Implementation Details:
- Add /api/items endpoint in backend/main.py that returns a list of sample items
- Each item should have: id, name, description, category, created_at fields
- Update frontend/src/App.tsx to add a Resource for items with ListGuesser
- Include X-Total-Count header for React Admin pagination
- Add debug logging in both backend (print/logging) and frontend (console.log)

Quality Requirements:
- Follow React Admin patterns for data providers
- Use proper REST API conventions (/api/resource)
- Handle errors gracefully with clear messages
- Run all linters and tests before completion

Start by exploring the current project structure, then implement the required features.
";

const USER_PROMPT: &str = "
Create a simple DataApp with:

1. Backend API endpoint `/api/items` that returns a list of sample items (each item should have id, name, description, category, created_at fields)
2. React Admin frontend that displays these items in a table with proper columns
3. Include debug logging in both backend and frontend
4. Make sure the React Admin data provider can fetch and display the items

The app should be functional with proper error handling and logging.
";

const MODEL: &str = "claude-sonnet-4-20250514";

async fn sandbox(client: &dagger_sdk::DaggerConn) -> Result<DaggerSandbox> {
    tracing::info!("Setting up sandbox with DataApps template...");

    // Build container from fastapi.Dockerfile
    let opts = dagger_sdk::ContainerBuildOptsBuilder::default()
        .dockerfile("fastapi.Dockerfile")
        .build()?;

    let ctr = client
        .container()
        .build_opts(client.host().directory("./dabgent_fastapi"), opts);

    ctr.sync().await?;
    let mut sandbox = DaggerSandbox::from_container(ctr, client.clone());

    // Seed template files
    tracing::info!("Seeding template_minimal files to sandbox...");
    seed_dataapps_template(&mut sandbox).await?;

    tracing::info!("Sandbox ready for DataApps development");
    Ok(sandbox)
}


async fn push_prompt<S: EventStore>(
    store: &S,
    stream_id: &str,
    aggregate_id: &str,
    prompt: &str,
) -> Result<()> {
    tracing::info!("Pushing initial prompt to event store...");
    let event = dabgent_agent::thread::Event::Prompted(prompt.to_owned());
    store
        .push_event(stream_id, aggregate_id, &event, &Default::default())
        .await
        .map_err(Into::into)
}

async fn seed_dataapps_template(sandbox: &mut DaggerSandbox) -> Result<()> {
    // Path to template_minimal relative to dabgent directory
    let template_path = Path::new("../dataapps/template_minimal");

    if !template_path.exists() {
        return Err(eyre::eyre!("Template path does not exist: {:?}", template_path));
    }

    tracing::info!("Collecting template files from {:?}", template_path);
    let files = collect_files_recursive(template_path, "/app")?;

    tracing::info!("Writing {} files to sandbox", files.len());
    let files_refs: Vec<(&str, &str)> = files.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    sandbox.write_files(files_refs).await?;

    Ok(())
}

fn collect_files_recursive(template_path: &Path, base_sandbox_path: &str) -> Result<Vec<(String, String)>> {
    use std::fs;

    let mut files = Vec::new();
    let skip_dirs = ["node_modules", ".git", ".venv", "target", "dist", "build"];

    fn collect_dir(
        dir_path: &Path,
        template_root: &Path,
        base_sandbox_path: &str,
        files: &mut Vec<(String, String)>,
        skip_dirs: &[&str],
    ) -> Result<()> {
        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let dir_name = path.file_name().unwrap().to_string_lossy();
                if skip_dirs.contains(&dir_name.as_ref()) {
                    continue;
                }
                collect_dir(&path, template_root, base_sandbox_path, files, skip_dirs)?;
            } else if path.is_file() {
                // Get relative path from template root
                let rel_path = path.strip_prefix(template_root)?;
                let sandbox_path = format!("{}/{}", base_sandbox_path, rel_path.to_string_lossy());

                // Read file content if it's a text file
                if let Ok(content) = fs::read_to_string(&path) {
                    files.push((sandbox_path, content));
                } else {
                    tracing::warn!("Skipping binary file: {:?}", path);
                }
            }
        }
        Ok(())
    }

    collect_dir(template_path, template_path, base_sandbox_path, &mut files, &skip_dirs)?;
    Ok(files)
}

