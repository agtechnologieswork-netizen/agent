//! Example MCP client that connects to the dabgent-mcp server
//!
//! This demonstrates how to:
//! - Start the dabgent-mcp server as a child process
//! - Connect to it using the rmcp client
//! - Call tools exposed by the server
//!
//! Run with: cargo run --example client

use eyre::Result;
use rmcp::model::CallToolRequestParam;
use rmcp::service::ServiceExt;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    // optionally initialize logging if RUST_LOG is set
    if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::fmt::init();
    }

    println!("Starting dabgent-mcp server as child process...");

    // start the server via cargo run (development mode)
    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.arg("run")
                    .arg("--package")
                    .arg("dabgent_mcp")
                    .arg("--bin")
                    .arg("dabgent-mcp");

                // pass through environment variables for credentials
                if let Ok(host) = std::env::var("DATABRICKS_HOST") {
                    cmd.env("DATABRICKS_HOST", host);
                }
                if let Ok(token) = std::env::var("DATABRICKS_TOKEN") {
                    cmd.env("DATABRICKS_TOKEN", token);
                }
                if let Ok(key) = std::env::var("GOOGLE_SERVICE_ACCOUNT_KEY") {
                    cmd.env("GOOGLE_SERVICE_ACCOUNT_KEY", key);
                }
            },
        ))?)
        .await?;

    println!("Connected to server!\n");

    // get server info
    let server_info = service.peer_info();
    println!("Server info: {:?}\n", server_info);

    // list available tools
    println!("=== Listing available tools ===");
    let tools_response = service.list_tools(Default::default()).await?;
    for tool in &tools_response.tools {
        let desc = tool.description.as_ref().map(|d| d.as_ref()).unwrap_or("No description");
        println!("- {}: {}", tool.name, desc);
    }
    println!();

    // example 1: call a Databricks tool (if available)
    if tools_response
        .tools
        .iter()
        .any(|t| t.name == "list_catalogs")
    {
        println!("=== Example: Listing Databricks catalogs ===");
        let result = service
            .call_tool(CallToolRequestParam {
                name: "list_catalogs".into(),
                arguments: None,
            })
            .await?;

        println!("Result: {}", serde_json::to_string_pretty(&result.content)?);
        println!();
    }

    println!("Example complete!");

    // cleanup
    service.cancel().await?;

    Ok(())
}
