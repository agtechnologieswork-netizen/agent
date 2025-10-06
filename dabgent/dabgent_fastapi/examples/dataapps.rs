use dabgent_agent::processor::agent::{Agent, AgentState, Command, Event};
use dabgent_agent::processor::databricks::{self, DatabricksToolHandler};
use dabgent_agent::processor::link::{Link, Runtime, link_runtimes};
use dabgent_agent::processor::finish::FinishHandler;
use dabgent_agent::processor::llm::{LLMConfig, LLMHandler};
use dabgent_agent::processor::tools::{TemplateConfig, ToolHandler};
use dabgent_agent::processor::utils::{LogHandler, ShutdownHandler};
use dabgent_fastapi::toolset::{dataapps_toolset, explore_databricks_tool_definition};
use dabgent_fastapi::validator::DataAppsValidator;
use dabgent_integrations::databricks::DatabricksRestClient;
use dabgent_mq::listener::PollingQueue;
use dabgent_mq::{create_store, Envelope, Event as MQEvent, EventStore, Handler, StoreConfig};
use dabgent_sandbox::SandboxHandle;
use eyre::Result;
use rig::client::ProviderClient;
use rig::message::{ToolCall, ToolResult, ToolResultContent};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::oneshot;


#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    run_worker().await.unwrap();
}

pub async fn run_worker() -> Result<()> {
    let store = create_store(Some(StoreConfig::from_env())).await?;
    let store = PollingQueue::new(store);

    // ========================================================================
    // DataApps Agent Setup
    // ========================================================================

    let tools = dataapps_toolset(DataAppsValidator::new());

    let sandbox_handle = SandboxHandle::new(Default::default());
    let template_config = TemplateConfig::new("./dabgent_fastapi".to_string(), "fastapi.Dockerfile".to_string())
        .with_template("../dataapps/template_minimal".to_string());

    // Collect tool definitions including the explore_databricks_catalog delegation tool
    let mut tool_definitions: Vec<_> = tools.iter().map(|tool| tool.definition()).collect();
    tool_definitions.push(explore_databricks_tool_definition());

    let llm = LLMHandler::new(
        Arc::new(rig::providers::anthropic::Client::from_env()),
        LLMConfig {
            model: MODEL.to_string(),
            preamble: Some(SYSTEM_PROMPT.to_string()),
            tools: Some(tool_definitions),
            ..Default::default()
        },
    );

    let tool_handler = ToolHandler::new(
        tools,
        sandbox_handle.clone(),
        template_config.clone(),
    );

    let mut dataapps_runtime = Runtime::<AgentState<DataAppsAgent>, _>::new(store.clone(), ())
        .with_handler(llm)
        .with_handler(tool_handler);

    // Wipe and prepare export path
    let export_path = "/tmp/data_app";
    if std::path::Path::new(export_path).exists() {
        std::fs::remove_dir_all(export_path)?;
    }

    let tools_for_finish = dataapps_toolset(DataAppsValidator::new());
    let finish_handler = FinishHandler::new(
        sandbox_handle,
        export_path.to_string(),
        tools_for_finish,
        template_config,
    );
    dataapps_runtime = dataapps_runtime.with_handler(finish_handler);

    // Setup shutdown handler to trigger on Shutdown event
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let shutdown_handler = ShutdownHandler::new(shutdown_tx);
    dataapps_runtime = dataapps_runtime.with_handler(shutdown_handler);

    // ========================================================================
    // Databricks Worker Setup
    // ========================================================================

    let databricks_tools = databricks::toolbox();
    let databricks_client = Arc::new(
        DatabricksRestClient::new().map_err(|e| eyre::eyre!("Failed to create Databricks client: {}", e))?
    );

    let databricks_llm = LLMHandler::new(
        Arc::new(rig::providers::anthropic::Client::from_env()),
        LLMConfig {
            model: MODEL.to_string(),
            preamble: Some(DATABRICKS_PROMPT.to_string()),
            tools: Some(databricks_tools.iter().map(|tool| tool.definition()).collect()),
            ..Default::default()
        },
    );

    let databricks_tool_handler = DatabricksToolHandler::new(databricks_client, databricks_tools);

    let mut databricks_runtime = Runtime::<AgentState<DatabricksWorker>, _>::new(store.clone(), ())
        .with_handler(databricks_llm)
        .with_handler(databricks_tool_handler)
        .with_handler(LogHandler);

    // ========================================================================
    // Link the runtimes
    // ========================================================================

    link_runtimes(&mut dataapps_runtime, &mut databricks_runtime, DataAppsDatabricksLink);

    // Add LogHandler after linking
    let dataapps_runtime = dataapps_runtime.with_handler(LogHandler);

    // Send initial command before starting runtime
    let command = Command::PutUserMessage {
        content: rig::OneOrMany::one(rig::message::UserContent::text(USER_PROMPT)),
    };
    dataapps_runtime.handler.execute("dataapps", command).await?;

    // ========================================================================
    // Run both runtimes
    // ========================================================================

    let dataapps_handle = tokio::spawn(async move { dataapps_runtime.start().await });
    let databricks_handle = tokio::spawn(async move { databricks_runtime.start().await });

    // Run with graceful shutdown on completion
    tokio::select! {
        result = dataapps_handle => {
            result?
        },
        result = databricks_handle => {
            result?
        },
        _ = shutdown_rx => {
            tracing::info!("Graceful shutdown triggered");
            Ok(())
        }
    }
}

const SYSTEM_PROMPT: &str = "
You are a FastAPI and React developer creating data applications.

Workspace Setup:
- You have a pre-configured DataApps project structure in /app with backend and frontend directories
- Backend is in /app/backend with Python, FastAPI, and uv package management
- Frontend is in /app/frontend with React Admin and TypeScript
- Use 'uv run' for all Python commands (e.g., 'uv run python main.py')

Databricks Integration:
- When you need to explore Databricks Unity Catalog to understand schema, use `explore_databricks_catalog` tool
- This delegates to a specialist that will provide detailed table and column information
- Use this BEFORE building data APIs to understand the actual data structure
- The specialist will return comprehensive schema info including table names, column types, and sample data

Your Task:
1. If working with Databricks data, FIRST explore the catalog to understand schema
2. Create a data API based on the actual schema (or sample data if no Databricks)
3. Configure React Admin UI to display this data in a table
4. Add proper logging and debugging throughout
5. Ensure CORS is properly configured for React Admin

Implementation Details:
- Add /api/{resource} endpoints in backend/main.py based on actual data schema
- Create Pydantic models matching the database schema
- Update frontend/src/App.tsx to add Resources with proper field configuration
- Include X-Total-Count header for React Admin pagination
- Add debug logging in both backend (print/logging) and frontend (console.log)

Quality Requirements:
- Follow React Admin patterns for data providers
- Use proper REST API conventions (/api/resource)
- Handle errors gracefully with clear messages
- Run all linters and tests before completion

Start by exploring the current project structure (or Databricks catalog if relevant), then implement the required features.
Use the tools available to you as needed.
";

const USER_PROMPT: &str = "
Create a DataApp for the bakery sales data in Databricks:

1. First, explore the 'main' catalog in Databricks to find bakery or sales-related tables
2. Based on the discovered schema, create a FastAPI backend with endpoints for the relevant tables
3. Create React Admin frontend that displays the data in tables with proper columns matching the schema
4. Include debug logging in both backend and frontend
5. Make sure the data provider can fetch and display the actual data

The app should be functional and ready to connect to Databricks.
";

const MODEL: &str = "claude-sonnet-4-5-20250929";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataAppsAgent {
    pub done_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataAppsEvent {
    Finished,
}

impl MQEvent for DataAppsEvent {
    fn event_type(&self) -> String {
        match self {
            DataAppsEvent::Finished => "finished".to_string(),
        }
    }

    fn event_version(&self) -> String {
        "1.0".to_string()
    }
}

#[derive(Debug)]
pub enum DataAppsError {}

impl std::fmt::Display for DataAppsError {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl std::error::Error for DataAppsError {}

impl Agent for DataAppsAgent {
    const TYPE: &'static str = "dataapps_worker";
    type AgentCommand = ();
    type AgentEvent = DataAppsEvent;
    type AgentError = DataAppsError;
    type Services = ();

    async fn handle_tool_results(
        state: &AgentState<Self>,
        _: &Self::Services,
        incoming: Vec<ToolResult>,
    ) -> Result<Vec<Event<Self::AgentEvent>>, Self::AgentError> {
        let completed = state.merge_tool_results(&incoming);
        if let Some(done_id) = &state.agent.done_call_id {
            if let Some(result) = completed.iter().find(|r| done_id == &r.id) {
                let is_done = result.content.iter().any(|c| match c {
                    ToolResultContent::Text(text) => text.text.contains("success"),
                    _ => false,
                });
                if is_done {
                    return Ok(vec![Event::Agent(DataAppsEvent::Finished)]);
                }
            }
        }
        Ok(vec![state.results_passthrough(&incoming)])
    }

    fn apply_event(state: &mut AgentState<Self>, event: Event<Self::AgentEvent>) {
        match event {
            Event::ToolCalls { ref calls } => {
                for call in calls {
                    if call.function.name == "done" {
                        state.agent.done_call_id = Some(call.id.clone());
                        break;
                    }
                }
            }
            _ => {}
        }
    }
}

// ============================================================================
// Databricks Worker Agent
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatabricksWorker {
    pub parent_id: Option<String>,
    pub parent_call: Option<ToolCall>,
    pub finished: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabricksEvent {
    Grabbed {
        parent_id: String,
        call: ToolCall,
    },
    Finished {
        parent_id: String,
        call: ToolCall,
        summary: String,
    },
}

impl MQEvent for DatabricksEvent {
    fn event_type(&self) -> String {
        match self {
            DatabricksEvent::Grabbed { .. } => "grabbed".to_string(),
            DatabricksEvent::Finished { .. } => "finished".to_string(),
        }
    }

    fn event_version(&self) -> String {
        "1.0".to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabricksCommand {
    Explore { parent_id: String, call: ToolCall },
}

#[derive(Debug)]
pub enum DatabricksError {}

impl std::fmt::Display for DatabricksError {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl std::error::Error for DatabricksError {}

impl Agent for DatabricksWorker {
    const TYPE: &'static str = "databricks_worker";
    type AgentCommand = DatabricksCommand;
    type AgentEvent = DatabricksEvent;
    type AgentError = DatabricksError;
    type Services = ();

    async fn handle_tool_results(
        state: &AgentState<Self>,
        _: &Self::Services,
        incoming: Vec<ToolResult>,
    ) -> Result<Vec<Event<Self::AgentEvent>>, Self::AgentError> {
        let completed = state.merge_tool_results(&incoming);

        // Check if finish_delegation was called
        for result in &completed {
            for content in result.content.iter() {
                if let ToolResultContent::Text(text) = content {
                    if state.calls.keys().any(|id| *id == result.id) {
                        if let (Some(parent_id), Some(parent_call)) =
                            (&state.agent.parent_id, &state.agent.parent_call)
                        {
                            return Ok(vec![Event::Agent(DatabricksEvent::Finished {
                                parent_id: parent_id.clone(),
                                call: parent_call.clone(),
                                summary: text.text.clone(),
                            })]);
                        }
                    }
                }
            }
        }
        Ok(vec![state.results_passthrough(&incoming)])
    }

    async fn handle_command(
        _state: &AgentState<Self>,
        cmd: Self::AgentCommand,
        _: &Self::Services,
    ) -> Result<Vec<Event<Self::AgentEvent>>, Self::AgentError> {
        match cmd {
            DatabricksCommand::Explore { parent_id, call } => {
                let catalog = call
                    .function
                    .arguments
                    .get("catalog")
                    .and_then(|v| v.as_str())
                    .unwrap_or("main");
                let prompt = call
                    .function
                    .arguments
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let description = format!("Explore catalog '{}': {}", catalog, prompt);
                let content = rig::OneOrMany::one(rig::message::UserContent::text(description));

                Ok(vec![
                    Event::Agent(DatabricksEvent::Grabbed {
                        parent_id: parent_id.clone(),
                        call: call.clone(),
                    }),
                    Event::UserCompletion { content },
                ])
            }
        }
    }

    fn apply_event(state: &mut AgentState<Self>, event: Event<Self::AgentEvent>) {
        match event {
            Event::Agent(DatabricksEvent::Grabbed { parent_id, call }) => {
                state.agent.parent_id = Some(parent_id);
                state.agent.parent_call = Some(call);
                state.agent.finished = false;
            }
            Event::Agent(DatabricksEvent::Finished { .. }) => {
                state.agent.finished = true;
            }
            _ => {}
        }
    }
}

// ============================================================================
// Link between DataAppsAgent and DatabricksWorker
// ============================================================================

#[derive(Clone)]
pub struct DataAppsDatabricksLink;

impl<ES: EventStore> Link<ES> for DataAppsDatabricksLink {
    type AggregateA = AgentState<DataAppsAgent>;
    type AggregateB = AgentState<DatabricksWorker>;

    async fn forward(
        &self,
        envelope: &Envelope<AgentState<DataAppsAgent>>,
        _handler: &Handler<AgentState<DataAppsAgent>, ES>,
    ) -> Option<(String, Command<DatabricksCommand>)> {
        match &envelope.data {
            Event::ToolCalls { calls } => {
                if let Some(call) = calls
                    .iter()
                    .find(|c| c.function.name == "explore_databricks_catalog")
                {
                    let worker_id = format!("databricks_{}", call.id);
                    return Some((
                        worker_id,
                        Command::Agent(DatabricksCommand::Explore {
                            parent_id: envelope.aggregate_id.clone(),
                            call: call.clone(),
                        }),
                    ));
                }
                None
            }
            _ => None,
        }
    }

    async fn backward(
        &self,
        envelope: &Envelope<AgentState<DatabricksWorker>>,
        _handler: &Handler<AgentState<DatabricksWorker>, ES>,
    ) -> Option<(String, Command<()>)> {
        use dabgent_agent::toolbox::ToolCallExt;
        match &envelope.data {
            Event::Agent(DatabricksEvent::Finished {
                parent_id,
                call,
                summary,
            }) => {
                let result = serde_json::to_value(summary).unwrap();
                let result = call.to_result(Ok(result));
                let command = Command::PutToolResults {
                    results: vec![result],
                };
                Some((parent_id.clone(), command))
            }
            _ => None,
        }
    }
}

const DATABRICKS_PROMPT: &str = "
You are a Databricks catalog explorer. Explore Unity Catalog to understand data structures.

## Your Task
Explore the specified catalog and provide comprehensive summary of:
- Available schemas and purposes
- Tables with descriptions
- Column structures including names, types, sample values
- Relationships between tables

## Focus
- Look for business-relevant data
- Identify primary/foreign keys
- Use `databricks_describe_table` for full column details
- Note columns for API fields

## Completion
When done, call `finish_delegation` with comprehensive summary including:
- Overview of discoveries
- Key schemas and table counts
- Detailed table structures with column specs
- API endpoint recommendations with column mappings

IMPORTANT: Always use `databricks_describe_table` to get complete column details.
";

