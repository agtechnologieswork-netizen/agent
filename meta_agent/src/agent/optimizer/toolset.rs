use super::Evaluation;
use rig::tool::Tool;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct TraverseArgs {
    pub evaluation_id: usize,
    pub node_id: usize,
    pub num_steps: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum TraverseError {
    #[error("missing evaluation with id {0}")]
    MissingEvaluation(usize),
    #[error("invalid node id {0}")]
    InvalidNodeId(usize),
    #[error("invalid step count {0}")]
    InvalidStepCount(usize),
}

#[derive(Clone)]
pub struct Traverse {
    pub evaluation: Arc<Evaluation>,
}

impl Tool for Traverse {
    const NAME: &'static str = "traverse";

    type Args = TraverseArgs;
    type Output = String;
    type Error = TraverseError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Print evalutation trajectory messages for a number of steps upwards"
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "evaluation_id": {"type": "integer"},
                    "node_id": {"type": "integer"},
                    "num_steps": {"type": "integer"},
                },
                "required": ["evaluation_id", "node_id", "num_steps"],
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok(format!("placeholder"))
    }
}

#[derive(Deserialize)]
pub struct CompleteArgs {
    pub inspirations: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("complete failed")]
pub struct CompleteError;

#[derive(Clone)]
pub struct Complete;

impl Tool for Complete {
    const NAME: &'static str = "complete";

    type Args = CompleteArgs;
    type Output = Vec<String>;
    type Error = CompleteError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Suggest improvements for the agent configuration and instructions"
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "inspirations": {
                        "type": "array",
                        "items": {"type": "string"}
                    }
                },
                "required": ["inspirations"],
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok(vec![format!("placeholder")])
    }
}
