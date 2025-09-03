use crate::{
    agent::{Tool, Checker, utils::compact_error_message},
    llm::LLMClientDyn,
    workspace::WorkspaceDyn,
};
use eyre::Result;
use serde::Deserialize;
use std::sync::Arc;

/// Install packages using uv package manager
#[derive(Clone)]
pub struct UvAddTool;

#[derive(Deserialize)]
pub struct UvAddArgs {
    pub packages: Vec<String>,
}

impl Tool for UvAddTool {
    type Args = UvAddArgs;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "uv_add".to_string()
    }

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Install additional packages using uv".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "packages": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "List of packages to install"
                    }
                },
                "required": ["packages"]
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        let packages = args.packages.join(" ");
        let cmd = format!("uv add {}", packages);
        let result = workspace.bash(&cmd).await?;
        
        match result.exit_code {
            0 => Ok(Ok("success".to_string())),
            _ => Ok(Err(format!("Failed to add packages: {}", result.stderr))),
        }
    }
}

/// Run pyright type checking
#[derive(Clone)]
pub struct TypeCheckTool;

impl Tool for TypeCheckTool {
    type Args = serde_json::Value;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "type_check".to_string()
    }

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Run type checking with pyright".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(
        &self,
        _args: Self::Args,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        let result = workspace.bash("uv run pyright .").await?;
        
        match result.exit_code {
            0 => Ok(Ok("Type checking passed".to_string())),
            _ => Ok(Err(format!("{}\n{}", result.stdout, result.stderr))),
        }
    }
}

/// Run ruff linting with automatic fixes
#[derive(Clone)]
pub struct LintTool;

impl Tool for LintTool {
    type Args = serde_json::Value;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "lint_check".to_string()
    }

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Run ruff linting with automatic fixes".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(
        &self,
        _args: Self::Args,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        let result = workspace.bash("uv run ruff check . --fix").await?;
        
        match result.exit_code {
            0 => Ok(Ok("Linting passed".to_string())),
            _ => Ok(Err(format!("{}\n{}", result.stdout, result.stderr))),
        }
    }
}

/// Run pytest tests
#[derive(Clone)]
pub struct TestTool;

impl Tool for TestTool {
    type Args = serde_json::Value;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "run_tests".to_string()
    }

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Run pytest tests".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(
        &self,
        _args: Self::Args,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        let result = workspace.bash_with_pg("uv run pytest").await?;
        
        match result.exit_code {
            0 => Ok(Ok("All tests passed".to_string())),
            _ => Ok(Err(format!("{}\n{}", result.stdout, result.stderr))),
        }
    }
}

/// Run SQLModel validation tests
#[derive(Clone)]
pub struct SqlModelTool;

impl Tool for SqlModelTool {
    type Args = serde_json::Value;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "sqlmodel_check".to_string()
    }

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Run SQLModel validation tests".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(
        &self,
        _args: Self::Args,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        // Check if app/database.py exists
        if workspace.read_file("app/database.py").await.is_err() {
            return Ok(Err("Database configuration missing: app/database.py file not found".to_string()));
        }

        let result = workspace.bash_with_pg("uv run pytest -m sqlmodel -v").await?;
        
        match result.exit_code {
            0 => Ok(Ok("SQLModel validation passed".to_string())),
            _ => Ok(Err(format!("SQLModel validation failed:\n{}\n{}", result.stdout, result.stderr))),
        }
    }
}

/// Run ast-grep code pattern checks
#[derive(Clone)]
pub struct AstGrepTool;

impl Tool for AstGrepTool {
    type Args = serde_json::Value;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "astgrep_check".to_string()
    }

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Run ast-grep code pattern analysis".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(
        &self,
        _args: Self::Args,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        let result = workspace.bash("uv run ast-grep scan app/ tests/").await?;
        
        match result.exit_code {
            0 => Ok(Ok("Code pattern analysis passed".to_string())),
            _ => Ok(Err(format!("{}\n{}", result.stdout, result.stderr))),
        }
    }
}

/// Composite checker that runs all NiceGUI validation checks
/// Matches the validation pipeline from the Python version
pub struct NiceguiChecker {
    pub llm: Arc<dyn LLMClientDyn>,
    pub model: String,
}

impl NiceguiChecker {
    pub fn new(llm: Arc<dyn LLMClientDyn>, model: String) -> Self {
        Self { llm, model }
    }

    async fn compact_error_if_needed(&self, error: &str) -> String {
        const MAX_ERROR_LENGTH: usize = 4096;
        
        if error.len() <= MAX_ERROR_LENGTH {
            return error.to_string();
        }

        // Use tokio::task::spawn_blocking to make the async operation sync-compatible
        let llm = self.llm.clone();
        let model = self.model.clone();
        let error_owned = error.to_string();
        
        let handle = tokio::task::spawn(async move {
            compact_error_message(llm.as_ref(), &model, &error_owned, MAX_ERROR_LENGTH).await
        });

        match handle.await {
            Ok(Ok(compacted)) => {
                tracing::info!("Successfully compacted error message from {} to {} characters", error.len(), compacted.len());
                compacted
            },
            Ok(Err(e)) => {
                tracing::warn!("Failed to compact error message using LLM: {}", e);
                // Fallback to truncation
                format!("{}...\n[Error compaction failed, truncated from {} characters]", 
                    &error[..MAX_ERROR_LENGTH.saturating_sub(100)], 
                    error.len())
            },
            Err(e) => {
                tracing::warn!("Task spawn failed for error compaction: {}", e);
                // Fallback to truncation
                format!("{}...\n[Error compaction task failed, truncated from {} characters]", 
                    &error[..MAX_ERROR_LENGTH.saturating_sub(100)], 
                    error.len())
            }
        }
    }
}

impl Checker for NiceguiChecker {
    async fn run(
        &self,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> Result<Option<serde_json::Value>> {
        let mut all_errors = String::new();

        // Run lint checks
        let lint_tool = LintTool;
        if let Ok(Err(error)) = lint_tool.call(serde_json::Value::Null, workspace).await {
            let compacted_error = self.compact_error_if_needed(&error).await;
            all_errors.push_str(&format!("Lint errors:\n{}\n", compacted_error));
        }

        // Run type checks
        let type_tool = TypeCheckTool;
        if let Ok(Err(error)) = type_tool.call(serde_json::Value::Null, workspace).await {
            let compacted_error = self.compact_error_if_needed(&error).await;
            all_errors.push_str(&format!("Type errors:\n{}\n", compacted_error));
        }

        // Run tests (with PostgreSQL)
        let test_tool = TestTool;
        if let Ok(Err(error)) = test_tool.call(serde_json::Value::Null, workspace).await {
            let compacted_error = self.compact_error_if_needed(&error).await;
            all_errors.push_str(&format!("Test errors:\n{}\n", compacted_error));
        }

        // Run SQLModel checks if database.py exists (with PostgreSQL)
        let sqlmodel_tool = SqlModelTool;
        if let Ok(Err(error)) = sqlmodel_tool.call(serde_json::Value::Null, workspace).await {
            let compacted_error = self.compact_error_if_needed(&error).await;
            all_errors.push_str(&format!("SQLModel errors:\n{}\n", compacted_error));
        }

        // Run ast-grep checks
        let astgrep_tool = AstGrepTool;
        if let Ok(Err(error)) = astgrep_tool.call(serde_json::Value::Null, workspace).await {
            let compacted_error = self.compact_error_if_needed(&error).await;
            all_errors.push_str(&format!("Code pattern violations:\n{}\n", compacted_error));
        }

        if all_errors.is_empty() {
            Ok(None) // All checks passed
        } else {
            Ok(Some(serde_json::json!({"validation_errors": all_errors.trim()})))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{CompletionResponse, FinishReason, LLMClient};
    use rig::message::AssistantContent;
    use rig::OneOrMany;

    // Mock LLM client for testing
    #[derive(Clone)]
    struct MockLLM;

    impl LLMClient for MockLLM {
        async fn completion(&self, _completion: crate::llm::Completion) -> eyre::Result<CompletionResponse> {
            // Return a mock compacted error message
            let response_text = r#"<error>
Lint errors:
    src/main.py:15: F841 Local variable unused
    src/models.py:23: E302 Expected 2 blank lines

Type errors:
    src/service.py:45: error: Missing return type annotation

Test failures:
    AssertionError: Expected 'success' got 'failure'
    7 failed, 43 passed in 2.1s
</error>"#;
            
            Ok(CompletionResponse {
                choice: OneOrMany::one(AssistantContent::Text(rig::message::Text { 
                    text: response_text.to_string() 
                })),
                finish_reason: FinishReason::Stop,
                output_tokens: 150,
                input_tokens: 0,
                cache_read_input_tokens: None,
                cache_creation_input_tokens: None,
            })
        }
    }

    #[tokio::test]
    async fn test_error_compaction() {
        let mock_llm = Arc::new(MockLLM);
        let checker = NiceguiChecker::new(mock_llm, "test-model".to_string());
        
        // Create a long error message that should trigger compaction
        let long_error = "A".repeat(5000); // 5000 characters, longer than 4096 limit
        
        let compacted = checker.compact_error_if_needed(&long_error).await;
        
        // Should be compacted and contain the mock response
        assert!(compacted.contains("Lint errors:"));
        assert!(compacted.contains("Type errors:"));
        assert!(compacted.len() < long_error.len());
    }

    #[tokio::test]
    async fn test_short_error_not_compacted() {
        let mock_llm = Arc::new(MockLLM);
        let checker = NiceguiChecker::new(mock_llm, "test-model".to_string());
        
        let short_error = "Short error message";
        let result = checker.compact_error_if_needed(short_error).await;
        
        // Should return the original message unchanged
        assert_eq!(result, short_error);
    }
}