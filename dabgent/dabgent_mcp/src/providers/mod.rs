pub mod databricks;
pub mod filesystem;
pub mod google_sheets;

pub use databricks::DatabricksProvider;
pub use filesystem::FilesystemProvider;
pub use google_sheets::GoogleSheetsProvider;

use eyre::Result;
use rmcp::model::{
    CallToolRequestParam, CallToolResult, Implementation, PaginatedRequestParam, ProtocolVersion,
    ServerCapabilities, ServerInfo,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler};
use std::sync::Arc;

#[derive(Clone)]
pub struct CombinedProvider {
    databricks: Option<Arc<DatabricksProvider>>,
    google_sheets: Option<Arc<GoogleSheetsProvider>>,
    filesystem: Option<Arc<FilesystemProvider>>,
}

impl CombinedProvider {
    pub fn new(
        databricks: Option<DatabricksProvider>,
        google_sheets: Option<GoogleSheetsProvider>,
        filesystem: Option<FilesystemProvider>,
    ) -> Result<Self> {
        if databricks.is_none() && google_sheets.is_none() && filesystem.is_none() {
            return Err(eyre::eyre!("at least one provider must be available"));
        }
        Ok(Self {
            databricks: databricks.map(Arc::new),
            google_sheets: google_sheets.map(Arc::new),
            filesystem: filesystem.map(Arc::new),
        })
    }
}

impl ServerHandler for CombinedProvider {
    fn get_info(&self) -> ServerInfo {
        let mut providers = Vec::new();
        if self.databricks.is_some() {
            providers.push("Databricks");
        }
        if self.google_sheets.is_some() {
            providers.push("Google Sheets");
        }
        if self.filesystem.is_some() {
            providers.push("Filesystem");
        }

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
                "MCP server providing integrations for: {}",
                providers.join(", ")
            )),
        }
    }

    async fn call_tool(
        &self,
        params: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        // route to appropriate provider based on tool name prefix or availability
        if let Some(ref databricks) = self.databricks {
            if let Ok(result) = databricks
                .call_tool(params.clone(), context.clone())
                .await
            {
                return Ok(result);
            }
        }

        if let Some(ref google_sheets) = self.google_sheets {
            if let Ok(result) = google_sheets
                .call_tool(params.clone(), context.clone())
                .await
            {
                return Ok(result);
            }
        }

        if let Some(ref filesystem) = self.filesystem {
            if let Ok(result) = filesystem
                .call_tool(params.clone(), context.clone())
                .await
            {
                return Ok(result);
            }
        }

        Err(ErrorData::invalid_params(
            format!("unknown tool: {}", params.name),
            None,
        ))
    }

    async fn list_tools(
        &self,
        params: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<rmcp::model::ListToolsResult, ErrorData> {
        let mut tools = Vec::new();

        if let Some(ref databricks) = self.databricks {
            if let Ok(result) = databricks.list_tools(params.clone(), context.clone()).await {
                tools.extend(result.tools);
            }
        }

        if let Some(ref google_sheets) = self.google_sheets {
            if let Ok(result) = google_sheets.list_tools(params.clone(), context.clone()).await {
                tools.extend(result.tools);
            }
        }

        if let Some(ref filesystem) = self.filesystem {
            if let Ok(result) = filesystem.list_tools(params.clone(), context.clone()).await {
                tools.extend(result.tools);
            }
        }

        Ok(rmcp::model::ListToolsResult {
            tools,
            next_cursor: None,
        })
    }
}
