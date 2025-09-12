pub mod basic;
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub trait Tool: Send + Sync + Clone {
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

pub trait ToolDyn: Send + Sync {
    fn clone_box(&self) -> Box<dyn ToolDyn>;
    fn name(&self) -> String;
    fn definition(&self) -> rig::completion::ToolDefinition;
    fn call<'a>(
        &'a self,
        args: serde_json::Value,
        sandbox: &'a mut Box<dyn SandboxDyn>,
    ) -> Pin<Box<dyn Future<Output = ToolDynResult> + Send + 'a>>;
}

impl<T: Tool + 'static> ToolDyn for T {
    fn clone_box(&self) -> Box<dyn ToolDyn> {
        Box::new(self.clone())
    }

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

pub trait Validator: Clone {
    fn run(
        &self,
        sandbox: &mut Box<dyn SandboxDyn>,
    ) -> impl Future<Output = Result<Result<(), String>>> + Send;

    fn boxed(self) -> Box<dyn ValidatorDyn>
    where
        Self: Sized + Send + Sync + 'static,
    {
        Box::new(self)
    }
}

pub trait ValidatorDyn: Send + Sync {
    fn clone_box(&self) -> Box<dyn ValidatorDyn>;
    fn run<'a>(
        &'a self,
        sandbox: &'a mut Box<dyn SandboxDyn>,
    ) -> Pin<Box<dyn Future<Output = Result<Result<(), String>>> + Send + 'a>>;
}

impl<T: Validator + Send + Sync + 'static> ValidatorDyn for T {
    fn clone_box(&self) -> Box<dyn ValidatorDyn> {
        Box::new(self.clone())
    }

    fn run<'a>(
        &'a self,
        sandbox: &'a mut Box<dyn SandboxDyn>,
    ) -> Pin<Box<dyn Future<Output = Result<Result<(), String>>> + Send + 'a>> {
        Box::pin(self.run(sandbox))
    }
}

pub trait ToolCallExt {
    fn to_result(
        &self,
        result: Result<serde_json::Value, serde_json::Value>,
    ) -> rig::message::ToolResult;
}

impl ToolCallExt for rig::message::ToolCall {
    fn to_result(
        &self,
        result: Result<serde_json::Value, serde_json::Value>,
    ) -> rig::message::ToolResult {
        use rig::message::ToolResultContent;
        let inner = match result {
            Ok(value) => value,
            Err(error) => serde_json::json!({"error": error}),
        };
        let inner = serde_json::to_string(&inner).unwrap();
        rig::message::ToolResult {
            id: self.id.clone(),
            call_id: self.call_id.clone(),
            content: rig::OneOrMany::one(ToolResultContent::Text(inner.into())),
        }
    }
}

impl Clone for Box<dyn ToolDyn> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl Clone for Box<dyn ValidatorDyn> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
