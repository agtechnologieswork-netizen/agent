//! Utility functions for agent operations

use eyre::Result;

/// Create an LLM client from environment variables
pub fn create_llm_client() -> Result<rig::providers::anthropic::Client> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .map_err(|_| eyre::eyre!("Please set ANTHROPIC_API_KEY or OPENAI_API_KEY"))?;

    Ok(rig::providers::anthropic::Client::new(&api_key))
}