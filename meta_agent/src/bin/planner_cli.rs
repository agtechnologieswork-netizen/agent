// Minimal CLI entrypoint for the MVP planner

use meta_agent::planner::cli::run_cli;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Load .env from pinned agent path or current
    meta_agent::load_env_for_agent();

    // Read config
    let model = std::env::var("PLANNER_MODEL").unwrap_or_else(|_| "claude-3-5-sonnet-latest".to_string());
    let input = std::env::var("PLANNER_INPUT").ok();

    // LLM will be constructed from env by the app in a later step
    let llm = None;

    if let Some(input) = input {
        println!("Input: {}", input);
        run_cli(llm, model).await
    } else {
        run_cli(llm, model).await
    }
}


