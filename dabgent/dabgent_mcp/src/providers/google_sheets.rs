use dabgent_integrations::{
    FetchSpreadsheetDataRequest, GetSpreadsheetMetadataRequest, GoogleSheetsClient,
    ReadRangeRequest, ToolResultDisplay,
};
use eyre::Result;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler};
use std::sync::Arc;

#[derive(Clone)]
pub struct GoogleSheetsProvider {
    client: Arc<GoogleSheetsClient>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl GoogleSheetsProvider {
    pub async fn new() -> Result<Self> {
        let client = GoogleSheetsClient::new()
            .await
            .map_err(|e| eyre::eyre!("Failed to create Google Sheets client: {}", e))?;
        Ok(Self {
            client: Arc::new(client),
            tool_router: Self::tool_router(),
        })
    }

    #[tool(description = "Get metadata for a Google Sheets spreadsheet")]
    pub async fn get_metadata(
        &self,
        Parameters(args): Parameters<GetSpreadsheetMetadataRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.client.get_spreadsheet_metadata(&args).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result.display())])),
            Err(e) => Err(ErrorData::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Read a specific range from a Google Sheets spreadsheet")]
    pub async fn read_range(
        &self,
        Parameters(args): Parameters<ReadRangeRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.client.read_range(&args).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result.display())])),
            Err(e) => Err(ErrorData::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Fetch all data from a Google Sheets spreadsheet")]
    pub async fn fetch_full(
        &self,
        Parameters(args): Parameters<FetchSpreadsheetDataRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.client.fetch_spreadsheet_data(&args).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result.display())])),
            Err(e) => Err(ErrorData::internal_error(e.to_string(), None)),
        }
    }
}

#[tool_handler]
impl ServerHandler for GoogleSheetsProvider {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "dabgent-mcp-google-sheets".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Dabgent MCP - Google Sheets".to_string()),
                website_url: None,
                icons: None,
            },
            instructions: Some(
                "MCP server providing Google Sheets integration tools for reading spreadsheet data and metadata.".to_string(),
            ),
        }
    }
}
