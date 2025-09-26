//! Pipeline configuration shared between CLI and examples

use crate::toolbox::{self, basic::toolset};
use dabgent_sandbox::SandboxDyn;
use eyre::Result;

/// Default model for all pipelines
pub const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

/// Default system prompt for Python development
pub const PYTHON_SYSTEM_PROMPT: &str = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.
";

/// Default temperature for code generation
pub const DEFAULT_TEMPERATURE: f64 = 0.0;

/// Default max tokens
pub const DEFAULT_MAX_TOKENS: u64 = 4_096;

/// Default recipient for sandbox operations
pub const DEFAULT_RECIPIENT: &str = "sandbox";

// Note: create_dagger_sandbox is moved to examples and CLI since it requires dagger_sdk
// which is not a direct dependency of dabgent_agent

/// Standard Python validator for running main.py with uv
pub struct PythonValidator;

impl toolbox::Validator for PythonValidator {
    async fn run(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {
        sandbox.exec("uv run main.py").await.map(|result| {
            if result.exit_code == 0 {
                Ok(())
            } else {
                Err(format!(
                    "code: {}\nstdout: {}\nstderr: {}",
                    result.exit_code, result.stdout, result.stderr
                ))
            }
        })
    }
}

/// Creates the standard toolset with Python validator
pub fn create_python_toolset() -> Vec<Box<dyn crate::toolbox::ToolDyn>> {
    toolset(PythonValidator)
}

/// Configuration for pipeline setup
pub struct PipelineConfig {
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u64,
    pub preamble: String,
    pub recipient: Option<String>,
    pub examples_path: String,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            temperature: DEFAULT_TEMPERATURE,
            max_tokens: DEFAULT_MAX_TOKENS,
            preamble: PYTHON_SYSTEM_PROMPT.to_string(),
            recipient: Some(DEFAULT_RECIPIENT.to_string()),
            examples_path: "./examples".to_string(),
        }
    }
}

impl PipelineConfig {
    /// Creates a config for CLI usage (with adjusted paths)
    pub fn for_cli() -> Self {
        Self {
            examples_path: "./examples".to_string(),
            ..Default::default()
        }
    }

    /// Creates a config for examples (with default paths)
    pub fn for_examples() -> Self {
        Self::default()
    }
}