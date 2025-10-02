use dabgent_agent::processor::agent::{
    Agent, AgentState, Command, Event, Request, Response, Runtime,
};
use dabgent_agent::processor::databricks::DatabricksToolHandler;
use dabgent_agent::processor::link::{Link, link_runtimes};
use dabgent_agent::processor::llm::{LLMConfig, LLMHandler};
use dabgent_agent::processor::utils::LogHandler;
use dabgent_integrations::databricks::DatabricksRestClient;
use dabgent_mq::db::sqlite::SqliteStore;
use dabgent_mq::listener::PollingQueue;
use dabgent_mq::{Event as MQEvent, EventStore, Handler};
use eyre::Result;
use rig::client::ProviderClient;
use rig::completion::ToolDefinition;
use rig::message::{ToolCall, ToolResult, ToolResultContent, UserContent};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const MODEL: &str = "claude-sonnet-4-20250514";

const MAIN_PROMPT: &str = "
You are an AI assistant that helps users understand Databricks catalogs.
Use the 'explore_databricks_catalog' tool to delegate exploration tasks to a specialist.
";

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

const USER_PROMPT: &str = "
Explore the 'main' catalog in Databricks and tell me about any bakery or sales data.
";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    run_databricks_worker().await.unwrap();
}

pub async fn run_databricks_worker() -> Result<()> {
    let store = store().await;

    // Main agent setup
    let main_llm = LLMHandler::new(
        Arc::new(rig::providers::anthropic::Client::from_env()),
        LLMConfig {
            model: MODEL.to_string(),
            preamble: Some(MAIN_PROMPT.to_string()),
            tools: Some(vec![explore_databricks_tool_definition()]),
            ..Default::default()
        },
    );
    let mut main_runtime = Runtime::<MainAgent, _>::new(store.clone(), ())
        .with_handler(main_llm)
        .with_handler(LogHandler);

    // Databricks worker setup
    let databricks_client =
        Arc::new(DatabricksRestClient::new().map_err(|e| eyre::eyre!("{}", e))?);
    let databricks_llm = LLMHandler::new(
        Arc::new(rig::providers::anthropic::Client::from_env()),
        LLMConfig {
            model: MODEL.to_string(),
            preamble: Some(DATABRICKS_PROMPT.to_string()),
            tools: Some(databricks_tool_definitions()),
            ..Default::default()
        },
    );
    let databricks_tool_handler = DatabricksToolHandler::new(databricks_client);
    let mut databricks_runtime = Runtime::<DatabricksWorker, _>::new(store.clone(), ())
        .with_handler(databricks_llm)
        .with_handler(databricks_tool_handler)
        .with_handler(LogHandler);

    link_runtimes(&mut main_runtime, &mut databricks_runtime, DatabricksLink);

    // Send initial task
    let command = Command::SendRequest(Request::Completion {
        content: rig::OneOrMany::one(UserContent::text(USER_PROMPT)),
    });
    main_runtime.handler.execute("main", command).await?;

    let main_handle = tokio::spawn(async move { main_runtime.start().await });
    let databricks_handle = tokio::spawn(async move { databricks_runtime.start().await });

    tokio::select! {
        _ = main_handle => {},
        _ = databricks_handle => {},
    }

    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MainAgent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MainEvent {}

impl MQEvent for MainEvent {
    fn event_type(&self) -> String {
        "main".to_string()
    }

    fn event_version(&self) -> String {
        "1.0".to_string()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MainError {}

impl Agent for MainAgent {
    const TYPE: &'static str = "main";
    type AgentCommand = ();
    type AgentEvent = MainEvent;
    type AgentError = MainError;
    type Services = ();

    async fn handle_tool_results(
        state: &AgentState<Self>,
        _: &Self::Services,
        incoming: Vec<ToolResult>,
    ) -> Result<Vec<Event<Self::AgentEvent>>, Self::AgentError> {
        let completed = state.merge_tool_results(incoming);
        let content = completed.into_iter().map(UserContent::ToolResult);
        let content = rig::OneOrMany::many(content).unwrap();
        Ok(vec![Event::Request(Request::Completion { content })])
    }

    fn apply_event(_state: &mut AgentState<Self>, _event: Event<Self::AgentEvent>) {}
}

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

#[derive(Debug, thiserror::Error)]
pub enum DatabricksError {}

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
        let completed = state.merge_tool_results(incoming);

        // Check if finish_delegation was called
        for result in &completed {
            for content in result.content.iter() {
                if let ToolResultContent::Text(text) = content {
                    // If we have a result for finish_delegation, emit Finished event
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

        let content = completed.into_iter().map(UserContent::ToolResult);
        let content = rig::OneOrMany::many(content).unwrap();
        Ok(vec![Event::Request(Request::Completion { content })])
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
                let content = rig::OneOrMany::one(UserContent::text(description));

                Ok(vec![
                    Event::Agent(DatabricksEvent::Grabbed {
                        parent_id: parent_id.clone(),
                        call: call.clone(),
                    }),
                    Event::Request(Request::Completion { content }),
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

#[derive(Clone)]
pub struct DatabricksLink;

impl<ES: EventStore> Link<ES> for DatabricksLink {
    type RuntimeA = MainAgent;
    type RuntimeB = DatabricksWorker;

    async fn forward(
        &self,
        a_id: &str,
        event: &Event<MainEvent>,
        _handler: &Handler<AgentState<MainAgent>, ES>,
    ) -> Option<(String, Command<DatabricksCommand>)> {
        match event {
            Event::Request(Request::ToolCalls { calls }) => {
                if let Some(call) = calls
                    .iter()
                    .find(|c| c.function.name == "explore_databricks_catalog")
                {
                    let worker_id = format!("databricks_{}", call.id);
                    return Some((
                        worker_id,
                        Command::Agent(DatabricksCommand::Explore {
                            parent_id: a_id.to_owned(),
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
        _b_id: &str,
        event: &Event<DatabricksEvent>,
        _handler: &Handler<AgentState<DatabricksWorker>, ES>,
    ) -> Option<(String, Command<()>)> {
        use dabgent_agent::toolbox::ToolCallExt;
        match event {
            Event::Agent(DatabricksEvent::Finished {
                parent_id,
                call,
                summary,
            }) => {
                let result = serde_json::to_value(summary).unwrap();
                let result = call.to_result(Ok(result));
                let command = Command::SendResponse(Response::ToolResults {
                    results: vec![result],
                });
                Some((parent_id.clone(), command))
            }
            _ => None,
        }
    }
}

fn explore_databricks_tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "explore_databricks_catalog".to_string(),
        description: "Explore a Databricks catalog to understand data structure".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "catalog": {
                    "type": "string",
                    "description": "Catalog name to explore"
                },
                "prompt": {
                    "type": "string",
                    "description": "What to look for in the catalog"
                }
            },
            "required": ["catalog", "prompt"]
        }),
    }
}

fn databricks_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "databricks_list_catalogs".to_string(),
            description: "List all available catalogs in Unity Catalog".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": [],
            }),
        },
        ToolDefinition {
            name: "databricks_list_schemas".to_string(),
            description: "List all schemas in a specific catalog".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "catalog_name": {
                        "type": "string",
                        "description": "Name of the catalog"
                    }
                },
                "required": ["catalog_name"],
            }),
        },
        ToolDefinition {
            name: "databricks_list_tables".to_string(),
            description: "List tables in a catalog and schema".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "catalog_name": {"type": "string"},
                    "schema_name": {"type": "string"},
                    "exclude_inaccessible": {"type": "boolean", "default": true}
                },
                "required": ["catalog_name", "schema_name"],
            }),
        },
        ToolDefinition {
            name: "databricks_describe_table".to_string(),
            description: "Get comprehensive table details including metadata, columns, sample data"
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "table_full_name": {
                        "type": "string",
                        "description": "Full table name in format 'catalog.schema.table'"
                    },
                    "sample_size": {"type": "integer", "default": 10}
                },
                "required": ["table_full_name"],
            }),
        },
        ToolDefinition {
            name: "databricks_execute_query".to_string(),
            description: "Execute a SELECT query on Databricks".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "timeout": {"type": "integer", "default": 45}
                },
                "required": ["query"],
            }),
        },
        ToolDefinition {
            name: "finish_delegation".to_string(),
            description: "Complete databricks exploration and return comprehensive summary"
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "summary": {
                        "type": "string",
                        "description": "Comprehensive summary of databricks exploration"
                    }
                },
                "required": ["summary"]
            }),
        },
    ]
}

async fn store() -> PollingQueue<SqliteStore> {
    let pool = sqlx::SqlitePool::connect(":memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool, "agent");
    store.migrate().await;
    PollingQueue::new(store)
}
