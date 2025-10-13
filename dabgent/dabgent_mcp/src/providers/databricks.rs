use dabgent_agent::processor::databricks::{
    DatabricksDescribeTableArgs, DatabricksExecuteQueryArgs, DatabricksListCatalogsArgs,
    DatabricksListSchemasArgs, DatabricksListTablesArgs,
};
use dabgent_integrations::{
    DatabricksRestClient, DescribeTableRequest, ExecuteSqlRequest, ListSchemasRequest,
    ListTablesRequest, TableInfo,
};
use eyre::Result;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler};
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone)]
pub struct DatabricksProvider {
    client: Arc<DatabricksRestClient>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl DatabricksProvider {
    pub fn new() -> Result<Self> {
        let client = DatabricksRestClient::new()
            .map_err(|e| eyre::eyre!("Failed to create Databricks client: {}", e))?;
        Ok(Self {
            client: Arc::new(client),
            tool_router: Self::tool_router(),
        })
    }
}

#[tool_handler]
impl ServerHandler for DatabricksProvider {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "dabgent-mcp-databricks".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Dabgent MCP - Databricks".to_string()),
                website_url: None,
                icons: None,
            },
            instructions: Some(
                "MCP server providing Databricks integration tools for querying data, exploring catalogs, schemas, and tables.".to_string(),
            ),
        }
    }
}

// Helper for pagination
fn apply_pagination<T>(items: Vec<T>, limit: usize, offset: usize) -> (Vec<T>, String) {
    let total = items.len();
    let paginated: Vec<T> = items.into_iter().skip(offset).take(limit).collect();
    let shown = paginated.len();

    let pagination_info = if total > limit + offset {
        format!(
            "Showing {} items (offset {}, limit {}). Total: {}",
            shown, offset, limit, total
        )
    } else if offset > 0 {
        format!("Showing {} items (offset {}). Total: {}", shown, offset, total)
    } else if total > limit {
        format!("Showing {} items (limit {}). Total: {}", shown, limit, total)
    } else {
        format!("Showing all {} items", total)
    };

    (paginated, pagination_info)
}

// Response types
#[derive(Serialize)]
pub struct ExecuteSqlResult {
    pub rows: Vec<serde_json::Value>,
}

#[derive(Serialize)]
pub struct ListCatalogsResult {
    pub catalogs: Vec<String>,
}

#[derive(Serialize)]
pub struct ListSchemasResult {
    pub schemas: Vec<String>,
    pub pagination: String,
}

#[derive(Serialize)]
pub struct ListTablesResult {
    pub tables: Vec<TableInfo>,
}

// Tool implementation methods
impl DatabricksProvider {
    #[tool(description = "Execute SQL query in Databricks")]
    pub async fn execute_sql(
        &self,
        args: DatabricksExecuteQueryArgs,
    ) -> Result<CallToolResult, ErrorData> {
        let request = ExecuteSqlRequest {
            query: args.query,
        };
        let result = self
            .client
            .execute_sql_request(&request)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let rows: Vec<serde_json::Value> = result
            .into_iter()
            .map(|row| serde_json::to_value(row).unwrap())
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&ExecuteSqlResult { rows })
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        )]))
    }

    #[tool(description = "List all available Databricks catalogs")]
    pub async fn list_catalogs(
        &self,
        _args: DatabricksListCatalogsArgs,
    ) -> Result<CallToolResult, ErrorData> {
        let catalogs = self
            .client
            .list_catalogs()
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&ListCatalogsResult { catalogs })
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        )]))
    }

    #[tool(description = "List all schemas in a Databricks catalog with pagination support")]
    pub async fn list_schemas(
        &self,
        args: DatabricksListSchemasArgs,
    ) -> Result<CallToolResult, ErrorData> {
        let request = ListSchemasRequest {
            catalog_name: args.catalog_name,
        };
        let mut schemas = self
            .client
            .list_schemas_request(&request)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        // Apply filter if provided
        if let Some(filter) = &args.filter {
            let filter_lower = filter.to_lowercase();
            schemas.retain(|s| s.to_lowercase().contains(&filter_lower));
        }

        let (schemas, pagination) = apply_pagination(schemas, args.limit, args.offset);

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&ListSchemasResult { schemas, pagination })
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        )]))
    }

    #[tool(description = "List tables in a Databricks catalog and schema")]
    pub async fn list_tables(
        &self,
        args: DatabricksListTablesArgs,
    ) -> Result<CallToolResult, ErrorData> {
        let request = ListTablesRequest {
            catalog_name: args.catalog_name,
            schema_name: args.schema_name,
            exclude_inaccessible: args.exclude_inaccessible,
        };
        let tables = self
            .client
            .list_tables_request(&request)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&ListTablesResult { tables })
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        )]))
    }

    #[tool(
        description = "Get detailed information about a Databricks table including schema and optional sample data"
    )]
    pub async fn describe_table(
        &self,
        args: DatabricksDescribeTableArgs,
    ) -> Result<CallToolResult, ErrorData> {
        let request = DescribeTableRequest {
            table_full_name: args.table_full_name,
            sample_size: args.sample_size,
        };
        let details = self
            .client
            .describe_table_request(&request)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&details)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        )]))
    }
}
