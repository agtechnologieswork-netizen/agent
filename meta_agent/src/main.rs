#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    Ok(())
}
