use crate::helpers::wrap_result;
use dabgent_integrations::{FetchFullArgs, GetMetadataArgs, GoogleSheetsClient, ReadRangeArgs};
use eyre::Result;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
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
        Parameters(args): Parameters<GetMetadataArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        wrap_result(self.client.get_spreadsheet_metadata(&args.url_or_id).await)
    }

    #[tool(description = "Read a specific range from a Google Sheets spreadsheet")]
    pub async fn read_range(
        &self,
        Parameters(args): Parameters<ReadRangeArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        wrap_result(self.client.read_range(&args.url_or_id, &args.range).await)
    }

    #[tool(description = "Fetch all data from a Google Sheets spreadsheet")]
    pub async fn fetch_full(
        &self,
        Parameters(args): Parameters<FetchFullArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        wrap_result(
            self.client
                .fetch_spreadsheet_data(&args.url_or_id)
                .await,
        )
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
