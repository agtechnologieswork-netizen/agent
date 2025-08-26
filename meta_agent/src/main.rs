#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    
    // Run NiceGUI demo instead of basic Python demo
    meta_agent::agent::actor::run_nicegui_demo_agent().await?;
    
    // Alternative demos:
    // meta_agent::agent::actor::run_demo_agent().await?; // Basic Python demo
    // meta_agent::agent::actor::eval_demo_agent().await?;
    //meta_agent::agent::optimizer::test_step_render();
    //meta_agent::agent::optimizer::test_traj_render();
    //meta_agent::agent::optimizer::test_simple_formatter();
    //meta_agent::agent::optimizer::test_traj_formatter();
    Ok(())
}
