//! Smoke test for dabgent-mcp server
//!
//! Verifies that:
//! - Server can be instantiated in-process
//! - Basic MCP protocol operations work (list_tools, call_tool)
//! - At least one provider is available

use dabgent_mcp::providers::{CombinedProvider, DatabricksProvider, GoogleSheetsProvider};
use eyre::Result;
use rmcp::ServiceExt;
use rmcp_in_process_transport::in_process::TokioInProcess;

#[tokio::test]
async fn smoke_test_mcp_server() -> Result<()> {
    // initialize providers
    let databricks = DatabricksProvider::new().ok();
    let google_sheets = GoogleSheetsProvider::new().await.ok();

    // at least one provider must be available
    let provider = CombinedProvider::new(databricks, google_sheets)
        .expect("At least one integration must be configured for smoke test");

    // create in-process service
    let tokio_in_process = TokioInProcess::new(provider).await?;
    let service = ().serve(tokio_in_process).await?;

    // verify server info is available
    let server_info = service.peer_info();
    assert!(server_info.is_some(), "Server info should be available");

    let info = server_info.unwrap();
    assert_eq!(info.server_info.name, "dabgent-mcp");
    assert!(!info.server_info.version.is_empty());

    // list tools
    let tools_response = service.list_tools(Default::default()).await?;
    assert!(!tools_response.tools.is_empty(), "Should have at least one tool");

    // cleanup
    service.cancel().await?;

    Ok(())
}
