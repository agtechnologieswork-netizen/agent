use dabgent_integrations::GoogleSheetsClient;
use eyre::Result;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct GoogleSheetsProvider {
    client: Arc<GoogleSheetsClient>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl GoogleSheetsProvider {
    pub async fn new() -> Result<Self> {
        let client = GoogleSheetsClient::new().await
            .map_err(|e| eyre::eyre!("Failed to create Google Sheets client: {}", e))?;
        Ok(Self {
            client: Arc::new(client),
            tool_router: Self::tool_router(),
        })
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

// Tool argument and result types

#[derive(Deserialize, Serialize)]
pub struct GetMetadataArgs {
    pub url_or_id: String,
}

#[derive(Deserialize, Serialize)]
pub struct ReadRangeArgs {
    pub url_or_id: String,
    pub range: String,
}

#[derive(Serialize)]
pub struct ReadRangeResult {
    pub values: Vec<Vec<String>>,
}

#[derive(Deserialize, Serialize)]
pub struct FetchFullArgs {
    pub url_or_id: String,
}


// Tool implementation methods
impl GoogleSheetsProvider {
    #[tool(description = "Get metadata for a Google Sheets spreadsheet")]
    pub async fn get_metadata(&self, args: GetMetadataArgs) -> Result<CallToolResult, ErrorData> {
        let metadata = self
            .client
            .get_spreadsheet_metadata(&args.url_or_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&metadata)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        )]))
    }

    #[tool(description = "Read a specific range from a Google Sheets spreadsheet")]
    pub async fn read_range(&self, args: ReadRangeArgs) -> Result<CallToolResult, ErrorData> {
        let values = self
            .client
            .read_range(&args.url_or_id, &args.range)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&ReadRangeResult { values })
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        )]))
    }

    #[tool(description = "Fetch all data from a Google Sheets spreadsheet")]
    pub async fn fetch_full(&self, args: FetchFullArgs) -> Result<CallToolResult, ErrorData> {
        let data = self
            .client
            .fetch_spreadsheet_data(&args.url_or_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&data)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        )]))
    }
}
