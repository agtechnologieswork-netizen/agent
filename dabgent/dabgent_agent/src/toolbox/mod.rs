pub mod basic;
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub trait Tool: Send + Sync {
    type Args: for<'a> Deserialize<'a> + Serialize + Send + Sync;
    type Output: Serialize + Send + Sync;
    type Error: Serialize + Send + Sync;
    fn name(&self) -> String;
    fn definition(&self) -> rig::completion::ToolDefinition;
    fn call(
        &self,
        args: Self::Args,
        sandbox: &mut Box<dyn SandboxDyn>,
    ) -> impl Future<Output = Result<Result<Self::Output, Self::Error>>> + Send;
}

type ToolDynResult = Result<Result<serde_json::Value, serde_json::Value>>;

pub trait ToolDyn {
    fn name(&self) -> String;
    fn definition(&self) -> rig::completion::ToolDefinition;
    fn call<'a>(
        &'a self,
        args: serde_json::Value,
        sandbox: &'a mut Box<dyn SandboxDyn>,
    ) -> Pin<Box<dyn Future<Output = ToolDynResult> + Send + 'a>>;
}

impl<T: Tool> ToolDyn for T {
    fn name(&self) -> String {
        Tool::name(self)
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
        self.definition()
    }

    fn call<'a>(
        &'a self,
        args: serde_json::Value,
        sandbox: &'a mut Box<dyn SandboxDyn>,
    ) -> Pin<Box<dyn Future<Output = ToolDynResult> + Send + 'a>> {
        Box::pin(async move {
            match serde_json::from_value::<<Self as Tool>::Args>(args) {
                Ok(args) => {
                    let result = Tool::call(self, args, sandbox).await?;
                    let result = match result {
                        Ok(output) => Ok(serde_json::to_value(output)?),
                        Err(error) => Err(serde_json::to_value(error)?),
                    };
                    Ok(result)
                }
                Err(error) => Ok(Err(serde_json::to_value(error.to_string())?)),
            }
        })
    }
}
