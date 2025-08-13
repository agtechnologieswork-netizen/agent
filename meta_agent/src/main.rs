#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    meta_agent::agent::actor::run_demo_agent().await?;
    //meta_agent::agent::actor::eval_demo_agent().await?;
    //meta_agent::agent::optimizer::test_step_render();
    //meta_agent::agent::optimizer::test_traj_render();
    //meta_agent::agent::optimizer::test_simple_formatter();
    //meta_agent::agent::optimizer::test_traj_formatter();
    Ok(())
}
