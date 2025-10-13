use super::{databricks, google_sheets};
use dabgent_agent::processor::databricks::{
    DatabricksDescribeTableArgs, DatabricksExecuteQueryArgs, DatabricksListCatalogsArgs,
    DatabricksListSchemasArgs, DatabricksListTablesArgs,
};
use eyre::Result;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{CallToolResult, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler};
use std::sync::Arc;

/// Unified provider that composes multiple integration providers
#[derive(Clone)]
pub struct UnifiedProvider {
    databricks: Option<Arc<databricks::DatabricksProvider>>,
    google_sheets: Option<Arc<google_sheets::GoogleSheetsProvider>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl UnifiedProvider {
    pub async fn new() -> Result<Self> {
        use tracing::{info, warn};

        // try to initialize Databricks
        let databricks = match databricks::DatabricksProvider::new() {
            Ok(provider) => {
                info!("✓ Databricks integration enabled");
                Some(Arc::new(provider))
            }
            Err(e) => {
                warn!("✗ Databricks integration disabled: {}", e);
                None
            }
        };

        // try to initialize Google Sheets
        let google_sheets = match google_sheets::GoogleSheetsProvider::new().await {
            Ok(provider) => {
                info!("✓ Google Sheets integration enabled");
                Some(Arc::new(provider))
            }
            Err(e) => {
                warn!("✗ Google Sheets integration disabled: {}", e);
                None
            }
        };

        // ensure at least one integration is available
        if databricks.is_none() && google_sheets.is_none() {
            return Err(eyre::eyre!(
                "No integrations available. Please configure at least one:\n\
                 - Databricks: Set DATABRICKS_HOST and DATABRICKS_TOKEN\n\
                 - Google Sheets: Place credentials at ~/.config/gspread/credentials.json"
            ));
        }

        Ok(Self {
            databricks,
            google_sheets,
            tool_router: Self::tool_router(),
        })
    }
}

// forward all databricks tools
impl UnifiedProvider {
    #[tool(description = "Execute SQL query in Databricks")]
    async fn execute_sql(
        &self,
        args: DatabricksExecuteQueryArgs,
    ) -> Result<CallToolResult, ErrorData> {
        match &self.databricks {
            Some(db) => db.execute_sql(args).await,
            None => Err(ErrorData::invalid_request(
                "Databricks integration not enabled",
                None,
            )),
        }
    }

    #[tool(description = "List all available Databricks catalogs")]
    async fn list_catalogs(
        &self,
        args: DatabricksListCatalogsArgs,
    ) -> Result<CallToolResult, ErrorData> {
        match &self.databricks {
            Some(db) => db.list_catalogs(args).await,
            None => Err(ErrorData::invalid_request(
                "Databricks integration not enabled",
                None,
            )),
        }
    }

    #[tool(description = "List all schemas in a Databricks catalog with pagination support")]
    async fn list_schemas(
        &self,
        args: DatabricksListSchemasArgs,
    ) -> Result<CallToolResult, ErrorData> {
        match &self.databricks {
            Some(db) => db.list_schemas(args).await,
            None => Err(ErrorData::invalid_request(
                "Databricks integration not enabled",
                None,
            )),
        }
    }

    #[tool(description = "List tables in a Databricks catalog and schema")]
    async fn list_tables(
        &self,
        args: DatabricksListTablesArgs,
    ) -> Result<CallToolResult, ErrorData> {
        match &self.databricks {
            Some(db) => db.list_tables(args).await,
            None => Err(ErrorData::invalid_request(
                "Databricks integration not enabled",
                None,
            )),
        }
    }

    #[tool(
        description = "Get detailed information about a Databricks table including schema and optional sample data"
    )]
    async fn describe_table(
        &self,
        args: DatabricksDescribeTableArgs,
    ) -> Result<CallToolResult, ErrorData> {
        match &self.databricks {
            Some(db) => db.describe_table(args).await,
            None => Err(ErrorData::invalid_request(
                "Databricks integration not enabled",
                None,
            )),
        }
    }

    #[tool(description = "Get metadata for a Google Sheets spreadsheet")]
    async fn get_metadata(
        &self,
        args: google_sheets::GetMetadataArgs,
    ) -> Result<CallToolResult, ErrorData> {
        match &self.google_sheets {
            Some(sheets) => sheets.get_metadata(args).await,
            None => Err(ErrorData::invalid_request(
                "Google Sheets integration not enabled",
                None,
            )),
        }
    }

    #[tool(description = "Read a specific range from a Google Sheets spreadsheet")]
    async fn read_range(
        &self,
        args: google_sheets::ReadRangeArgs,
    ) -> Result<CallToolResult, ErrorData> {
        match &self.google_sheets {
            Some(sheets) => sheets.read_range(args).await,
            None => Err(ErrorData::invalid_request(
                "Google Sheets integration not enabled",
                None,
            )),
        }
    }

    #[tool(description = "Fetch all data from a Google Sheets spreadsheet")]
    async fn fetch_full(
        &self,
        args: google_sheets::FetchFullArgs,
    ) -> Result<CallToolResult, ErrorData> {
        match &self.google_sheets {
            Some(sheets) => sheets.fetch_full(args).await,
            None => Err(ErrorData::invalid_request(
                "Google Sheets integration not enabled",
                None,
            )),
        }
    }
}

#[tool_handler]
impl ServerHandler for UnifiedProvider {
    fn get_info(&self) -> ServerInfo {
        let mut enabled_integrations = Vec::new();
        if self.databricks.is_some() {
            enabled_integrations.push("Databricks");
        }
        if self.google_sheets.is_some() {
            enabled_integrations.push("Google Sheets");
        }

        let integrations_str = enabled_integrations.join(", ");

        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "dabgent-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Dabgent MCP Server".to_string()),
                website_url: None,
                icons: None,
            },
            instructions: Some(format!(
                "MCP server providing integration tools for: {}",
                integrations_str
            )),
        }
    }
}
