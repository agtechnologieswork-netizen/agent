use crate::toolbox::{Tool, ToolDyn};
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use rig::completion::ToolDefinition;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::future::Future;

/// Tool for requesting multiple choice selection from user
#[derive(Debug, Clone)]
pub struct MultiChoiceTool;

#[derive(Debug, Serialize, Deserialize)]
pub struct MultiChoiceArgs {
    pub prompt: String,
    pub options: Vec<String>,
    #[serde(default)]
    pub allow_multiple: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MultiChoiceOutput {
    pub status: String,
    pub wait_type: String,
}

impl Tool for MultiChoiceTool {
    type Args = MultiChoiceArgs;
    type Output = MultiChoiceOutput;
    type Error = String;

    fn name(&self) -> String {
        "request_multi_choice".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: Some("Request user to select from multiple options".to_owned()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "The question or prompt for the user"
                    },
                    "options": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "List of options for the user to choose from"
                    },
                    "allow_multiple": {
                        "type": "boolean",
                        "description": "Whether to allow multiple selections",
                        "default": false
                    }
                },
                "required": ["prompt", "options"]
            }),
        }
    }

    async fn call(
        &self,
        _args: Self::Args,
        _sandbox: &mut Box<dyn SandboxDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        // This returns immediately - the actual selection happens in the UI
        Ok(Ok(MultiChoiceOutput {
            status: "waiting_for_user".to_string(),
            wait_type: "multi_choice".to_string(),
        }))
    }
}

/// Tool for requesting clarification from user
#[derive(Debug, Clone)]
pub struct ClarificationTool;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClarificationArgs {
    pub question: String,
    pub context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClarificationOutput {
    pub status: String,
    pub wait_type: String,
}

impl Tool for ClarificationTool {
    type Args = ClarificationArgs;
    type Output = ClarificationOutput;
    type Error = String;

    fn name(&self) -> String {
        "request_clarification".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: Some("Request clarification from the user when something is unclear".to_owned()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "The clarification question"
                    },
                    "context": {
                        "type": "string",
                        "description": "Optional context about what needs clarification"
                    }
                },
                "required": ["question"]
            }),
        }
    }

    async fn call(
        &self,
        _args: Self::Args,
        _sandbox: &mut Box<dyn SandboxDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        Ok(Ok(ClarificationOutput {
            status: "waiting_for_user".to_string(),
            wait_type: "clarification".to_string(),
        }))
    }
}

/// Tool for requesting confirmation from user
#[derive(Debug, Clone)]
pub struct ConfirmationTool;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfirmationArgs {
    pub prompt: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfirmationOutput {
    pub status: String,
    pub wait_type: String,
}

impl Tool for ConfirmationTool {
    type Args = ConfirmationArgs;
    type Output = ConfirmationOutput;
    type Error = String;

    fn name(&self) -> String {
        "request_confirmation".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: Some("Request yes/no confirmation from the user".to_owned()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "The confirmation prompt"
                    }
                },
                "required": ["prompt"]
            }),
        }
    }

    async fn call(
        &self,
        _args: Self::Args,
        _sandbox: &mut Box<dyn SandboxDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        Ok(Ok(ConfirmationOutput {
            status: "waiting_for_user".to_string(),
            wait_type: "confirmation".to_string(),
        }))
    }
}

/// Tool for indicating need to continue generation after hitting token limit
#[derive(Debug, Clone)]
pub struct ContinueTool;

#[derive(Debug, Serialize, Deserialize)]
pub struct ContinueArgs {
    pub reason: String,
    pub progress_summary: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContinueOutput {
    pub status: String,
    pub need_continuation: bool,
}

impl Tool for ContinueTool {
    type Args = ContinueArgs;
    type Output = ContinueOutput;
    type Error = String;

    fn name(&self) -> String {
        "continue_generation".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: Some("Indicate that generation needs to continue due to length limits".to_owned()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "reason": {
                        "type": "string",
                        "description": "Why continuation is needed"
                    },
                    "progress_summary": {
                        "type": "string",
                        "description": "Summary of progress so far"
                    }
                },
                "required": ["reason"]
            }),
        }
    }

    async fn call(
        &self,
        _args: Self::Args,
        _sandbox: &mut Box<dyn SandboxDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        Ok(Ok(ContinueOutput {
            status: "need_continuation".to_string(),
            need_continuation: true,
        }))
    }
}

/// Create a toolset with user interaction tools
pub fn user_interaction_tools() -> Vec<Box<dyn ToolDyn>> {
    vec![
        Box::new(MultiChoiceTool),
        Box::new(ClarificationTool),
        Box::new(ConfirmationTool),
        Box::new(ContinueTool),
    ]
}

/// Combine user interaction tools with existing tools
pub fn with_user_interaction(mut tools: Vec<Box<dyn ToolDyn>>) -> Vec<Box<dyn ToolDyn>> {
    tools.extend(user_interaction_tools());
    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_multi_choice_tool() {
        let tool = MultiChoiceTool;
        let args = MultiChoiceArgs {
            prompt: "Select the tables you need".to_string(),
            options: vec![
                "users".to_string(),
                "orders".to_string(),
                "products".to_string(),
            ],
            allow_multiple: true,
        };
        
        // Create a dummy sandbox for testing
        let mut sandbox: Box<dyn SandboxDyn> = Box::new(dabgent_sandbox::DummySandbox);
        
        let result = tool.call(args, &mut sandbox).await.unwrap().unwrap();
        assert_eq!(result.status, "waiting_for_user");
        assert_eq!(result.wait_type, "multi_choice");
    }

    #[tokio::test]
    async fn test_clarification_tool() {
        let tool = ClarificationTool;
        let args = ClarificationArgs {
            question: "Which database should I use?".to_string(),
            context: Some("PostgreSQL or MongoDB".to_string()),
        };
        
        let mut sandbox: Box<dyn SandboxDyn> = Box::new(dabgent_sandbox::DummySandbox);
        
        let result = tool.call(args, &mut sandbox).await.unwrap().unwrap();
        assert_eq!(result.status, "waiting_for_user");
        assert_eq!(result.wait_type, "clarification");
    }
}