use super::{Tool, Validator, ValidatorDyn};
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashArgs {
    pub command: String,
}

#[derive(Clone)]
pub struct Bash;

impl Tool for Bash {
    type Args = BashArgs;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "bash".to_owned()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
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
        sandbox: &mut Box<dyn SandboxDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        let result = sandbox.exec(&args.command).await?;
        match result.exit_code {
            0 => Ok(Ok(result.stdout)),
            _ => Ok(Err(format!(
                "Error:\n{}\n{}",
                result.stderr, result.stdout
            ))),
        }
    }
}

#[derive(Clone)]
pub struct WriteFile;

#[derive(Serialize, Deserialize)]
pub struct WriteFileArgs {
    pub path: String,
    pub contents: String,
}

impl Tool for WriteFile {
    type Args = WriteFileArgs;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "write_file".to_owned()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "write_file".to_string(),
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
                "required": ["path", "contents"],
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
        sandbox: &mut Box<dyn SandboxDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        let WriteFileArgs { path, contents } = args;
        sandbox.write_file(&path, &contents).await?;
        Ok(Ok("success".to_string()))
    }
}

#[derive(Clone)]
pub struct ReadFile;

#[derive(Serialize, Deserialize)]
pub struct ReadFileArgs {
    pub path: String,
}

impl Tool for ReadFile {
    type Args = ReadFileArgs;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "read_file".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
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
        sandbox: &mut Box<dyn SandboxDyn>,
    ) -> eyre::Result<Result<Self::Output, Self::Error>> {
        let result = sandbox.read_file(&args.path).await?;
        Ok(Ok(result))
    }
}

#[derive(Clone)]
pub struct LsDir;

#[derive(Serialize, Deserialize)]
pub struct LsDirArgs {
    pub path: String,
}

impl Tool for LsDir {
    type Args = LsDirArgs;
    type Output = Vec<String>;
    type Error = String;

    fn name(&self) -> String {
        "ls_dir".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
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
        sandbox: &mut Box<dyn SandboxDyn>,
    ) -> eyre::Result<Result<Self::Output, Self::Error>> {
        let result = sandbox.list_directory(&args.path).await?;
        Ok(Ok(result))
    }
}

#[derive(Clone)]
pub struct RmFile;

#[derive(Serialize, Deserialize)]
pub struct RmFileArgs {
    pub path: String,
}

impl Tool for RmFile {
    type Args = RmFileArgs;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "rm_file".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
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
        sandbox: &mut Box<dyn SandboxDyn>,
    ) -> eyre::Result<Result<Self::Output, Self::Error>> {
        sandbox.delete_file(&args.path).await?;
        Ok(Ok("success".to_string()))
    }
}

#[derive(Clone)]
pub struct EditFile;

#[derive(Serialize, Deserialize)]
pub struct EditFileArgs {
    pub path: String,
    pub find: String,
    pub replace: String,
}

impl Tool for EditFile {
    type Args = EditFileArgs;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "edit_file".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
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
        sandbox: &mut Box<dyn SandboxDyn>,
    ) -> eyre::Result<Result<Self::Output, Self::Error>> {
        let EditFileArgs {
            path,
            find,
            replace,
        } = args;
        let contents = sandbox.read_file(&path).await?;
        match contents.matches(&find).count() {
            1 => {
                let contents = contents.replace(&find, &replace);
                sandbox.write_file(&path, &contents).await?;
                Ok(Ok("success".to_string()))
            }
            num => Ok(Err(format!("Error: found {num} matches, expected 1"))),
        }
    }
}

pub struct DoneTool {
    validator: Box<dyn ValidatorDyn>,
}

impl DoneTool {
    pub fn new<T: Validator + Send + Sync + 'static>(validator: T) -> Self {
        Self {
            validator: validator.boxed(),
        }
    }
}

impl Tool for DoneTool {
    type Args = serde_json::Value;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "done".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
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
        sandbox: &mut Box<dyn SandboxDyn>,
    ) -> eyre::Result<eyre::Result<Self::Output, Self::Error>> {
        self.validator
            .run(sandbox)
            .await
            .map(|result| match result {
                Ok(_) => Ok("success".to_string()),
                Err(err) => Err(format!("error: {}", err)),
            })
    }
}

#[derive(Clone)]
pub struct UvAdd;

#[derive(Serialize, Deserialize)]
pub struct UvAddArgs {
    pub package: String,
    #[serde(default)]
    pub dev: bool,
}

impl Tool for UvAdd {
    type Args = UvAddArgs;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "uv_add".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Add a Python dependency using uv".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "package": {
                        "type": "string",
                        "description": "Package name to add (e.g., 'fastapi', 'requests==2.28.0')",
                    },
                    "dev": {
                        "type": "boolean",
                        "description": "Add as development dependency",
                        "default": false
                    }
                },
                "required": ["package"],
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
        sandbox: &mut Box<dyn SandboxDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        let UvAddArgs { package, dev } = args;
        
        let mut command = format!("uv add {}", package);
        
        if dev {
            command.push_str(" --dev");
        }
        
        let result = sandbox.exec(&command).await?;
        match result.exit_code {
            0 => Ok(Ok(format!("Added {}: {}", package, result.stdout))),
            _ => Ok(Err(format!("Failed to add {}: {}\n{}", package, result.stderr, result.stdout))),
        }
    }
}

pub fn toolset<T: Validator + Send + Sync + 'static>(validator: T) -> Vec<Box<dyn super::ToolDyn>> {
    vec![
        Box::new(Bash),
        Box::new(WriteFile),
        Box::new(ReadFile),
        Box::new(LsDir),
        Box::new(RmFile),
        Box::new(EditFile),
        Box::new(UvAdd),
        Box::new(DoneTool::new(validator)),
    ]
}
