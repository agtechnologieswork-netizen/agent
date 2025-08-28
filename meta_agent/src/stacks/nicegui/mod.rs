use super::StackConfig;
use crate::agent::{actor, actor::AgentPipeline};
use eyre::Result;

pub mod tools;
pub use tools::*;

#[derive(Debug, Clone)]
pub struct NiceguiStack;

impl StackConfig for NiceguiStack {
    fn name(&self) -> &'static str {
        "nicegui"
    }
    
    fn context_path(&self) -> &'static str {
        "./src/stacks/nicegui"
    }
    
    fn preamble(&self) -> &'static str {
        "You are a NiceGUI Python web application assistant. Use uv for package management, PostgreSQL for database, and follow modern Python patterns with type safety."
    }
    
    async fn create_pipeline(&self) -> Result<AgentPipeline> {
        actor::claude_nicegui_pipeline().await
    }
}

impl NiceguiStack {
    /// Get the advanced application system prompt for NiceGUI
    pub fn get_advanced_preamble(&self, use_databricks: bool) -> String {
        crate::agent::optimizer::get_application_system_prompt(use_databricks)
    }
}