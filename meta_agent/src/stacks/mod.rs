use crate::agent::actor::AgentPipeline;
use eyre::Result;

pub mod python;
pub mod nicegui;

/// All supported stack types
#[derive(Debug, Clone)]
pub enum Stack {
    Python(python::PythonStack),
    Nicegui(nicegui::NiceguiStack),
}

impl Stack {
    /// The name of the stack (e.g., "python", "nicegui")
    pub fn name(&self) -> &'static str {
        match self {
            Stack::Python(s) => s.name(),
            Stack::Nicegui(s) => s.name(),
        }
    }
    
    /// Path to the stack's context directory relative to meta_agent root
    pub fn context_path(&self) -> &'static str {
        match self {
            Stack::Python(s) => s.context_path(),
            Stack::Nicegui(s) => s.context_path(),
        }
    }
    
    /// Preamble/system message for the stack
    pub fn preamble(&self) -> &'static str {
        match self {
            Stack::Python(s) => s.preamble(),
            Stack::Nicegui(s) => s.preamble(),
        }
    }
    
    /// Create the pipeline for this stack
    pub async fn create_pipeline(&self) -> Result<AgentPipeline> {
        match self {
            Stack::Python(s) => s.create_pipeline().await,
            Stack::Nicegui(s) => s.create_pipeline().await,
        }
    }
    
    /// Get the dockerfile path for this stack (defaults to Dockerfile.appbuild in context_path)
    pub fn dockerfile_path(&self) -> String {
        format!("{}/Dockerfile.appbuild", self.context_path())
    }
    
    /// Get the advanced preamble for this stack (stack-specific prompts)
    pub fn get_advanced_preamble(&self, use_databricks: bool) -> String {
        match self {
            Stack::Python(_) => self.preamble().to_string(), // Python uses simple preamble for now
            Stack::Nicegui(s) => s.get_advanced_preamble(use_databricks),
        }
    }
    
    /// Get template files that should be included in the initial workspace
    pub fn template_files(&self) -> Result<std::collections::HashMap<String, String>> {
        match self {
            Stack::Python(s) => s.template_files(),
            Stack::Nicegui(s) => s.template_files(),
        }
    }
}

/// Configuration trait for individual stack types
pub trait StackConfig {
    fn name(&self) -> &'static str;
    fn context_path(&self) -> &'static str;
    fn preamble(&self) -> &'static str;
    fn create_pipeline(&self) -> impl std::future::Future<Output = Result<AgentPipeline>> + Send;
    
    /// Get template files that should be included in the initial workspace
    fn template_files(&self) -> Result<std::collections::HashMap<String, String>> {
        Ok(std::collections::HashMap::new())
    }
}

/// Registry of all available stack configurations
pub struct StackRegistry;

impl StackRegistry {
    /// Get stack configuration by name
    pub async fn get_stack(name: &str) -> Result<Stack> {
        match name {
            "python" => Ok(Stack::Python(python::PythonStack)),
            "nicegui" => Ok(Stack::Nicegui(nicegui::NiceguiStack)),
            _ => eyre::bail!("Unsupported stack: {}. Supported stacks: python, nicegui", name),
        }
    }
    
    /// List all supported stack names
    pub fn supported_stacks() -> Vec<&'static str> {
        vec!["python", "nicegui"]
    }
}
