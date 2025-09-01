use crate::{
    agent::{AgentNode, Checker, CheckerDyn, NodeTool, Tool},
    workspace::WorkspaceDyn,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone)]
pub struct BashTool;

#[derive(Deserialize)]
pub struct BashArgs {
    pub command: String,
}

impl Tool for BashTool {
    type Args = BashArgs;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "bash".to_string()
    }

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Run a bash command".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command to run in the shell",
                    }
                },
                "required": ["command"],
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> eyre::Result<Result<Self::Output, Self::Error>> {
        tracing::debug!("Executing bash command: {}", args.command);
        let result = workspace.bash(&args.command).await?;
        match result.exit_code {
            0 => {
                tracing::debug!("Bash command succeeded: {}", args.command);
                Ok(Ok(result.stdout))
            },
            _ => {
                tracing::info!("Bash command returned non-zero exit code {}: {}\nstderr: {}", 
                    result.exit_code, args.command, result.stderr);
                Ok(Err(format!("Error:\n{}\n{}", result.stderr, result.stdout)))
            },
        }
    }
}

#[derive(Clone)]
pub struct WriteFileTool;

#[derive(Deserialize)]
pub struct WriteFileArgs {
    pub path: String,
    pub contents: String,
}

impl Tool for WriteFileTool {
    type Args = WriteFileArgs;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "write_file".to_string()
    }

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Write content to a file".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file",
                    },
                    "contents": {
                        "type": "string",
                        "description": "Content to write to the file",
                    }
                },
                "required": ["path", "content"],
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> eyre::Result<Result<Self::Output, Self::Error>> {
        let WriteFileArgs { path, contents } = args;
        workspace.write_file(&path, &contents).await?;
        Ok(Ok("success".to_string()))
    }
}

// NodeTool implementation for WriteFileTool to track files
impl<N: AgentNode + Send + Sync> NodeTool<N> for WriteFileTool {
    async fn call_node(
        &self,
        args: Self::Args,
        node: &mut N,
    ) -> eyre::Result<Result<Self::Output, Self::Error>> {
        let WriteFileArgs { path, contents } = args;
        node.workspace_mut().write_file(&path, &contents).await?;

        // Track the file in the node
        node.files_mut().insert(path, contents);

        Ok(Ok("success".to_string()))
    }
}

#[derive(Clone)]
pub struct ReadFileTool;

#[derive(Deserialize)]
pub struct ReadFileArgs {
    pub path: String,
}

impl Tool for ReadFileTool {
    type Args = ReadFileArgs;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "read_file".to_string()
    }

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Read content from a file".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file",
                    }
                },
                "required": ["path"],
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> eyre::Result<Result<Self::Output, Self::Error>> {
        let result = workspace.read_file(&args.path).await?;
        Ok(Ok(result))
    }
}

#[derive(Clone)]
pub struct LsDirTool;

#[derive(Deserialize)]
pub struct LsDirArgs {
    pub path: String,
}

impl Tool for LsDirTool {
    type Args = LsDirArgs;
    type Output = Vec<String>;
    type Error = String;

    fn name(&self) -> String {
        "ls_dir".to_string()
    }

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "List files in a directory".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the directory",
                    }
                },
                "required": ["path"],
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> eyre::Result<Result<Self::Output, Self::Error>> {
        let result = workspace.ls(&args.path).await?;
        Ok(Ok(result))
    }
}

#[derive(Clone)]
pub struct RmFileTool;

#[derive(Deserialize)]
pub struct RmFileArgs {
    pub path: String,
}

impl Tool for RmFileTool {
    type Args = RmFileArgs;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "rm_file".to_string()
    }

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Remove a file".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to remove",
                    }
                },
                "required": ["path"],
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> eyre::Result<Result<Self::Output, Self::Error>> {
        workspace.rm(&args.path).await?;
        Ok(Ok("success".to_string()))
    }
}

#[derive(Clone)]
pub struct EditFileTool;

#[derive(Deserialize)]
pub struct EditFileArgs {
    pub path: String,
    pub find: String,
    pub replace: String,
}

impl Tool for EditFileTool {
    type Args = EditFileArgs;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "edit_file".to_string()
    }

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Edit a file by replacing text".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file",
                    },
                    "find": {
                        "type": "string",
                        "description": "Text to find in the file",
                    },
                    "replace": {
                        "type": "string",
                        "description": "Text to replace with",
                    }
                },
                "required": ["path", "find", "replace"],
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> eyre::Result<Result<Self::Output, Self::Error>> {
        let EditFileArgs {
            path,
            find,
            replace,
        } = args;
        let contents = workspace.read_file(&path).await?;
        match contents.matches(&find).count() {
            1 => {
                let contents = contents.replace(&find, &replace);
                workspace.write_file(&path, &contents).await?;
                Ok(Ok("success".to_string()))
            }
            num => Ok(Err(format!("Error: found {num} matches, expected 1"))),
        }
    }
}

// NodeTool implementation for EditFileTool to track files
impl<N: AgentNode + Send + Sync> NodeTool<N> for EditFileTool {
    async fn call_node(
        &self,
        args: Self::Args,
        node: &mut N,
    ) -> eyre::Result<Result<Self::Output, Self::Error>> {
        let EditFileArgs {
            path,
            find,
            replace,
        } = args;
        let contents = node.workspace_mut().read_file(&path).await?;
        match contents.matches(&find).count() {
            1 => {
                let new_contents = contents.replace(&find, &replace);
                node.workspace_mut()
                    .write_file(&path, &new_contents)
                    .await?;

                // Track the modified file in the node
                node.files_mut().insert(path, new_contents);

                Ok(Ok("success".to_string()))
            }
            num => Ok(Err(format!("Error: found {num} matches, expected 1"))),
        }
    }
}

#[derive(Clone)]
pub struct FinishTool {
    pub checker: Arc<dyn CheckerDyn>,
}

impl FinishTool {
    pub fn new(checker: impl Checker + 'static) -> Self {
        Self {
            checker: Arc::new(checker),
        }
    }
}

impl Tool for FinishTool {
    type Args = serde_json::Value;
    type Output = String;
    type Error = serde_json::Value;

    fn name(&self) -> String {
        "finish".to_string()
    }

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Run checks, if successful mark task as finished".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        }
    }

    async fn call(
        &self,
        _args: Self::Args,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> eyre::Result<eyre::Result<Self::Output, Self::Error>> {
        tracing::debug!("Running finish tool checker");
        self.checker.run(workspace).await.map(|value| match value {
            Some(error) => {
                tracing::info!("Finish tool validation failed: {:?}", error);
                Err(error)
            },
            None => {
                tracing::info!("Finish tool validation passed successfully");
                Ok("success".to_string())
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::WorkspaceDyn;
    use crate::workspace::dagger::*;
    use tempdir::TempDir;

    const TEST_DOCKERFILE: &str = "Dockerfile.appbuild";

    async fn setup_workspace(dagger_ref: &DaggerRef) -> Box<dyn WorkspaceDyn> {
        let temp_dir = TempDir::new("dagger").unwrap();
        let docker_path = temp_dir.path().join(TEST_DOCKERFILE);
        let dir_path = temp_dir.path().to_str().unwrap().to_string();
        std::fs::write(docker_path, "FROM alpine:latest\n").unwrap();
        let workspace = dagger_ref.workspace(TEST_DOCKERFILE.to_string(), dir_path);
        Box::new(workspace.await.unwrap()) as Box<dyn WorkspaceDyn>
    }

    #[tokio::test]
    async fn test_bash_tool() {
        let dagger_ref = DaggerRef::new();
        let mut workspace = setup_workspace(&dagger_ref).await;
        let tool = BashTool;
        let args = BashArgs {
            command: "echo Hello, World!".to_string(),
        };
        let result = tool.call(args, &mut workspace).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "Hello, World!\n");
    }

    #[tokio::test]
    async fn test_write_file_tool() {
        let dagger_ref = DaggerRef::new();
        let mut workspace = setup_workspace(&dagger_ref).await;
        let tool = WriteFileTool;
        let args = WriteFileArgs {
            path: "test.txt".to_string(),
            contents: "Hello, World!".to_string(),
        };
        let result = tool.call(args, &mut workspace).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "success");

        let read_result = workspace.read_file("test.txt").await;
        assert!(read_result.is_ok());
        assert_eq!(read_result.unwrap(), "Hello, World!");
    }

    #[tokio::test]
    async fn test_read_file_tool() {
        let dagger_ref = DaggerRef::new();
        let mut workspace = setup_workspace(&dagger_ref).await;
        workspace
            .write_file("test.txt", "Hello, World!")
            .await
            .unwrap();

        let tool = ReadFileTool;
        let args = ReadFileArgs {
            path: "test.txt".to_string(),
        };
        let result = tool.call(args, &mut workspace).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "Hello, World!");
    }

    #[tokio::test]
    async fn test_ls_dir_tool() {
        let dagger_ref = DaggerRef::new();
        let mut workspace = setup_workspace(&dagger_ref).await;
        workspace
            .write_file("file1.txt", "Content 1")
            .await
            .unwrap();
        workspace
            .write_file("file2.txt", "Content 2")
            .await
            .unwrap();
        let tool = LsDirTool;
        let args = LsDirArgs {
            path: ".".to_string(),
        }; // Current directory
        let result = tool.call(args, &mut workspace).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.is_ok());
        let files = output.unwrap();
        assert!(files.contains(&"file1.txt".to_string()));
        assert!(files.contains(&"file2.txt".to_string()));
    }

    #[tokio::test]
    async fn test_rm_file_tool() {
        let dagger_ref = DaggerRef::new();
        let mut workspace = setup_workspace(&dagger_ref).await;
        workspace
            .write_file("test.txt", "Hello, World!")
            .await
            .unwrap();
        let tool = RmFileTool;
        let args = RmFileArgs {
            path: "test.txt".to_string(),
        };
        let result = tool.call(args, &mut workspace).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "success");
        let read_result = workspace.read_file("test.txt").await;
        assert!(read_result.is_err());
    }

    #[tokio::test]
    async fn test_edit_file_tool() {
        let dagger_ref = DaggerRef::new();
        let mut workspace = setup_workspace(&dagger_ref).await;
        workspace
            .write_file("test.txt", "Hello, World!")
            .await
            .unwrap();
        let tool = EditFileTool;
        let args = EditFileArgs {
            path: "test.txt".to_string(),
            find: "World".to_string(),
            replace: "Appbuild".to_string(),
        };
        let result = tool.call(args, &mut workspace).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "success");
        let read_result = workspace.read_file("test.txt").await;
        assert!(read_result.is_ok());
        assert_eq!(read_result.unwrap(), "Hello, Appbuild!");

        let args = EditFileArgs {
            path: "test.txt".to_string(),
            find: "NonExistent".to_string(),
            replace: "Replacement".to_string(),
        };
        let result = tool.call(args, &mut workspace).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.is_err());
        assert_eq!(output.unwrap_err(), "Error: found 0 matches, expected 1");
    }
}
