pub mod planner;
pub mod llm;
pub mod events;

/// Load environment variables for the agent/tests.
pub fn load_env_for_agent() {
    let _ = dotenvy::dotenv();
}
