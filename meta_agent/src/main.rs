#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    meta_agent::agent::actor::run_demo_agent().await?;
    Ok(())
}
