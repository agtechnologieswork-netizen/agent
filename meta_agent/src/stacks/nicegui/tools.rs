use crate::{
    agent::{Tool, Checker},
    workspace::WorkspaceDyn,
};
use eyre::Result;
use serde::Deserialize;

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
pub struct NiceguiChecker;

impl Checker for NiceguiChecker {
    async fn run(
        &self,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> Result<Option<serde_json::Value>> {
        let mut all_errors = String::new();

        // Run lint checks
        let lint_tool = LintTool;
        if let Ok(Err(error)) = lint_tool.call(serde_json::Value::Null, workspace).await {
            all_errors.push_str(&format!("Lint errors:\n{}\n", error));
        }

        // Run type checks
        let type_tool = TypeCheckTool;
        if let Ok(Err(error)) = type_tool.call(serde_json::Value::Null, workspace).await {
            all_errors.push_str(&format!("Type errors:\n{}\n", error));
        }

        // Run tests (with PostgreSQL)
        let test_tool = TestTool;
        if let Ok(Err(error)) = test_tool.call(serde_json::Value::Null, workspace).await {
            all_errors.push_str(&format!("Test errors:\n{}\n", error));
        }

        // Run SQLModel checks if database.py exists (with PostgreSQL)
        let sqlmodel_tool = SqlModelTool;
        if let Ok(Err(error)) = sqlmodel_tool.call(serde_json::Value::Null, workspace).await {
            all_errors.push_str(&format!("SQLModel errors:\n{}\n", error));
        }

        // Run ast-grep checks
        let astgrep_tool = AstGrepTool;
        if let Ok(Err(error)) = astgrep_tool.call(serde_json::Value::Null, workspace).await {
            all_errors.push_str(&format!("Code pattern violations:\n{}\n", error));
        }

        if all_errors.is_empty() {
            Ok(None) // All checks passed
        } else {
            Ok(Some(serde_json::json!({"validation_errors": all_errors.trim()})))
        }
    }
}