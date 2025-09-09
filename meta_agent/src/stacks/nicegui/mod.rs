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

    fn template_files(&self) -> Result<std::collections::HashMap<String, String>> {
        Self::read_template_files()
    }
}

impl NiceguiStack {
    /// Get the advanced application system prompt for NiceGUI
    pub fn get_advanced_preamble(&self, use_databricks: bool) -> String {
        crate::agent::optimizer::get_application_system_prompt(use_databricks)
    }

    /// Get template files from the template directory
    fn read_template_files() -> eyre::Result<std::collections::HashMap<String, String>> {
        use std::path::Path;
        use walkdir::WalkDir;

        let template_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stacks/nicegui/template");
        let mut files = std::collections::HashMap::new();

        for entry in WalkDir::new(&template_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                let relative_path = path.strip_prefix(&template_dir)
                    .map_err(|e| eyre::eyre!("Failed to get relative path: {}", e))?;
                let relative_path_str = relative_path.to_string_lossy().to_string();

                // Filter out cache files and other temporary files
                let path_lower = relative_path_str.to_lowercase();
                let should_filter = path_lower.contains("__pycache__") ||
                  path_lower.contains(".ruff_cache") ||
                  path_lower.contains(".venv");

                if should_filter {
                    tracing::debug!("Skipping template file: {}", relative_path_str);
                    continue;
                }

                let content = std::fs::read_to_string(path)
                    .map_err(|e| eyre::eyre!("Failed to read file {}: {}", path.display(), e))?;

                files.insert(relative_path_str, content);
            }
        }

        Ok(files)
    }
}
