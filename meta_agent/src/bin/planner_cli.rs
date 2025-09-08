// Minimal CLI entrypoint for the MVP planner

use meta_agent::planner::cli::run_cli;
use rig::providers::anthropic::Client as Anthropic;
use meta_agent::llm::LLMClient as _; // bring `.boxed()` into scope

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Non-interactive input via env var PLANNER_INPUT; fallback to prompt
    let model = std::env::var("PLANNER_MODEL").unwrap_or_else(|_| "claude-3-5-sonnet-latest".to_string());
    let llm_api_key = std::env::var("ANTHROPIC_API_KEY").ok();
    let input = std::env::var("PLANNER_INPUT").ok();

    let anthropic_base = std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| "https://api.anthropic.com".to_string());
    let anthropic_version = std::env::var("ANTHROPIC_VERSION").unwrap_or_else(|_| "2023-06-01".to_string());
    let llm = llm_api_key.as_ref().map(|key| {
        Anthropic::new(key.as_str(), anthropic_base.as_str(), None, anthropic_version.as_str()).boxed()
    });

    if let Some(input) = input {
        // Run non-interactively
        // Reuse run_cli path by setting stdin to input when provided
        println!("Input: {}", input);
        // For now, call run_cli which asks stdin; a dedicated non-interactive path can be added later
        run_cli(llm, model).await
    } else {
        // Interactive
        run_cli(llm, model).await
    }
}


