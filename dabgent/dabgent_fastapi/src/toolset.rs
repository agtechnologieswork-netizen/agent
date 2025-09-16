use dabgent_agent::toolbox::{Tool, ToolDyn, Validator, basic::{WriteFile, ReadFile, EditFile, LsDir, RmFile, DoneTool}};
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use serde::{Deserialize, Serialize};

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
            name: Tool::name(self),
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

        let mut command = format!("cd /app/backend && uv add {}", package);

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

pub fn dataapps_toolset<T: Validator + Send + Sync + 'static>(validator: T) -> Vec<Box<dyn ToolDyn>> {
    vec![
        Box::new(WriteFile),
        Box::new(ReadFile),
        Box::new(EditFile),
        Box::new(LsDir),
        Box::new(RmFile),
        Box::new(UvAdd),
        Box::new(DoneTool::new(validator)),
    ]
}