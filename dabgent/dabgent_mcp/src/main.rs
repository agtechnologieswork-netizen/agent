use dabgent_mcp::providers::UnifiedProvider;
use eyre::Result;
use rmcp::transport::stdio;
use rmcp::ServiceExt;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // configure tracing to write to stderr only if RUST_LOG is set
    // this prevents interference with stdio MCP transport
    if std::env::var("RUST_LOG").is_ok() {
        // write to a file to avoid interfering with stdio
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/dabgent-mcp.log")?;

        tracing_subscriber::fmt()
            .with_writer(move || log_file.try_clone().unwrap())
            .init();
    }

    // always try to initialize all integrations
    // gracefully skip if credentials are missing
    let provider = UnifiedProvider::new().await?;

    let service = provider.serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}
