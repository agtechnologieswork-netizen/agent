use super::StackConfig;
use crate::agent::{actor, actor::AgentPipeline};
use eyre::Result;

#[derive(Debug, Clone)]
pub struct PythonStack;

impl StackConfig for PythonStack {
    fn name(&self) -> &'static str {
        "python"
    }
    
    fn context_path(&self) -> &'static str {
        "./src/stacks/python"
    }
    
    fn preamble(&self) -> &'static str {
        "You are a Python development assistant. Use uv for package management."
    }
    
    async fn create_pipeline(&self) -> Result<AgentPipeline> {
        actor::claude_python_pipeline().await
    }
}

impl PythonStack {
    /// Get the advanced Python system prompt 
    pub fn get_advanced_preamble(&self) -> String {
        crate::agent::optimizer::get_python_system_prompt()
    }
}