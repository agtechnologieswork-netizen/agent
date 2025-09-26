use dabgent_agent::processor::{CompactProcessor, DelegationProcessor, FinishProcessor, Pipeline, Processor, ThreadProcessor, ToolProcessor};
use dabgent_agent::toolbox::ToolDyn;
use dabgent_agent::toolbox::databricks::databricks_toolset;
use dabgent_fastapi::{toolset::dataapps_toolset, validator::DataAppsValidator, artifact_preparer::DataAppsArtifactPreparer};
use dabgent_fastapi::templates::{EMBEDDED_TEMPLATES, DEFAULT_TEMPLATE_PATH};
use dabgent_mq::{EventStore, create_store, StoreConfig};
use dabgent_sandbox::{Sandbox, NoOpSandbox, dagger::{ConnectOpts, Sandbox as DaggerSandbox}};
use eyre::Result;
use rig::client::ProviderClient;


#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    const STREAM_ID: &str = "dataapps";
    const AGGREGATE_ID: &str = "thread";

    let opts = ConnectOpts::default();
    opts.connect(|client| async move {
        let llm = rig::providers::gemini::Client::from_env();
        let store = create_store(Some(StoreConfig::from_env())).await?;
        tracing::info!("Event store initialized successfully");
        let sandbox = create_sandbox(&client).await?;
        let tool_processor_tools = dataapps_toolset(DataAppsValidator::new());
        let finish_processor_tools = dataapps_toolset(DataAppsValidator::new());

        push_llm_config(&store, STREAM_ID, AGGREGATE_ID, &tool_processor_tools).await?;

        // Use embedded templates in release mode, filesystem in debug mode
        let template_path = if cfg!(debug_assertions) {
            DEFAULT_TEMPLATE_PATH
        } else {
            EMBEDDED_TEMPLATES
        };

        push_seed_sandbox(&store, STREAM_ID, AGGREGATE_ID, template_path, "/app").await?;
        push_prompt(&store, STREAM_ID, AGGREGATE_ID, USER_PROMPT).await?;

        tracing::info!("Starting DataApps pipeline with model: {}", MODEL);

        let thread_processor = ThreadProcessor::new(llm.clone(), store.clone());

        // Create export directory path with timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let export_path = format!("/tmp/dataapps_output_{}", timestamp);

        // Fork sandbox for completion processor
        let completion_sandbox = sandbox.fork().await?;
        let tool_processor = ToolProcessor::new(dabgent_sandbox::Sandbox::boxed(sandbox), store.clone(), tool_processor_tools, None);

        let databricks_tools = databricks_toolset()
            .map_err(|e| eyre::eyre!("Failed to get databricks tools: {}", e))?;

        let databricks_tool_processor = ToolProcessor::new(
            NoOpSandbox::new().boxed(),  // NoOpSandbox for external API calls
            store.clone(),
            databricks_tools,
            Some("databricks_worker".to_string()),  // Only listen to delegated threads
        );

        let compact_processor = CompactProcessor::new(
            store.clone(),
            2048,
            "gemini-flash-latest".to_string(),  // Use same model as main pipeline
        );

        let delegation_processor = DelegationProcessor::new(
            store.clone(),
            "gemini-flash-lite-latest".to_string(),
        );

        // FixMe: FinishProcessor should have no state, including export path
        let finish_processor = FinishProcessor::new_with_preparer(
            dabgent_sandbox::Sandbox::boxed(completion_sandbox),
            store.clone(),
            export_path.clone(),
            finish_processor_tools,
            DataAppsArtifactPreparer,
        );

        let pipeline = Pipeline::new(
            store.clone(),
            vec![
                thread_processor.boxed(),
                tool_processor.boxed(),           // Handles main thread tools (recipient: None)
                databricks_tool_processor.boxed(), // Handles delegated thread tools (recipient: "databricks_worker")
                delegation_processor.boxed(),
                compact_processor.boxed(),
                finish_processor.boxed(),
            ],
        );

        tracing::info!("Artifacts will be exported to: {}", export_path);
        tracing::info!("Pipeline configured, starting execution...");

        pipeline.run(STREAM_ID.to_owned()).await?;
        Ok(())
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

Data Sources:
- You have access to Databricks Unity Catalog with bakery business data
- Use the 'explore_databricks_catalog' tool to discover available tables and schemas
- The catalog contains real business data about products, sales, customers, and orders
- Once you explore the data, use the actual schema and sample data for your API design

Your Task:
1. First, explore the Databricks catalog to understand available bakery data
2. Create a data API that serves real data from Databricks tables
3. Configure React Admin UI to display this data in tables
4. Add proper logging and debugging throughout
5. Ensure CORS is properly configured for React Admin

Implementation Details:
- Start by exploring the Databricks catalog to find relevant tables
- Design API endpoints based on the actual data structure you discover
- Each endpoint should return data with fields matching the Databricks schema
- Update frontend/src/App.tsx to add Resources for the discovered data
- Include X-Total-Count header for React Admin pagination
- Add debug logging in both backend (print/logging) and frontend (console.log)

Quality Requirements:
- Follow React Admin patterns for data providers
- Use proper REST API conventions (/api/resource)
- Handle errors gracefully with clear messages
- Design APIs around real data structures from Databricks
- Run all linters and tests before completion

Start by exploring the Databricks catalog, then implement the required features based on the actual data you find.
Use the tools available to you as needed.
";

const USER_PROMPT: &str = "
Create a bakery business DataApp by:

1. First, explore the Databricks catalog to discover available bakery data tables
2. Based on what you find, create backend API endpoints that serve the real data
3. Build React Admin frontend that displays the discovered data in tables
4. Include debug logging in both backend and frontend
5. Make sure the React Admin data provider can fetch and display the data properly

Focus on creating a functional DataApp that showcases real bakery business data from Databricks.
";

const MODEL: &str = "gemini-flash-latest";

async fn create_sandbox(client: &dagger_sdk::DaggerConn) -> Result<DaggerSandbox> {
    tracing::info!("Setting up sandbox with DataApps template...");

    // Build container from fastapi.Dockerfile
    let opts = dagger_sdk::ContainerBuildOptsBuilder::default()
        .dockerfile("fastapi.Dockerfile")
        .build()?;

    let ctr = client
        .container()
        .build_opts(client.host().directory("./dabgent_fastapi"), opts);

    ctr.sync().await?;
    let sandbox = DaggerSandbox::from_container(ctr, client.clone());
    tracing::info!("Sandbox ready for DataApps development");
    Ok(sandbox)
}

async fn push_llm_config<S: EventStore>(
    store: &S,
    stream_id: &str,
    aggregate_id: &str,
    tools: &[Box<dyn ToolDyn>],
) -> Result<()> {
    tracing::info!("Pushing LLM configuration to event store...");

    // Extract tool definitions from the tools
    let tool_definitions: Vec<rig::completion::ToolDefinition> = tools
        .iter()
        .map(|tool| tool.definition())
        .collect();

    let event = dabgent_agent::event::Event::LLMConfig {
        model: MODEL.to_owned(),
        temperature: 0.0,
        max_tokens: 8192,
        preamble: Some(SYSTEM_PROMPT.to_owned()),
        tools: Some(tool_definitions),
        recipient: None,
        parent: None,
    };
    store
        .push_event(stream_id, aggregate_id, &event, &Default::default())
        .await
        .map_err(Into::into)
}

async fn push_seed_sandbox<S: EventStore>(
    store: &S,
    stream_id: &str,
    aggregate_id: &str,
    template_path: &str,
    base_path: &str,
) -> Result<()> {
    tracing::info!("Pushing seed sandbox event: {}", template_path);
    let event = dabgent_agent::event::Event::SeedSandboxFromTemplate {
        template_path: template_path.to_owned(),
        base_path: base_path.to_owned(),
    };
    store
        .push_event(stream_id, aggregate_id, &event, &Default::default())
        .await
        .map_err(Into::into)
}

async fn push_prompt<S: EventStore>(
    store: &S,
    stream_id: &str,
    aggregate_id: &str,
    prompt: &str,
) -> Result<()> {
    tracing::info!("Pushing initial prompt to event store...");
    let content = rig::message::UserContent::Text(rig::message::Text { text: prompt.to_owned() });
    let event = dabgent_agent::event::Event::UserMessage(rig::OneOrMany::one(content));
    store
        .push_event(stream_id, aggregate_id, &event, &Default::default())
        .await
        .map_err(Into::into)
}
