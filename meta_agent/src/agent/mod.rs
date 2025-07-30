use crate::workspace::WorkspaceDyn;
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
pub mod actor;
pub mod toolset;
pub mod tree;
pub use tree::Tree;

pub trait Search<T>: Clone + Send {
    fn select(&mut self, root: &Tree<T>) -> impl Future<Output = Result<Vec<usize>>> + Send;
    fn unlock(&mut self, idx: usize) -> Result<()>;
}

pub trait Rollout<T>: Clone + Send {
    type Trajectory: Send;
    fn trajectory(
        &self,
        root: &Tree<T>,
        idx: usize,
    ) -> impl Future<Output = Result<Self::Trajectory>> + Send;
    fn rollout(&self, trajectory: Self::Trajectory) -> impl Future<Output = Result<T>> + Send;
}

pub trait Notify<T>: Clone + Send {
    fn notify_scheduled(
        &self,
        root: &Tree<T>,
        idx: usize,
    ) -> impl Future<Output = Result<()>> + Send;
    fn notify_completed(
        &self,
        root: &Tree<T>,
        result: &Result<(T, usize)>,
    ) -> impl Future<Output = Result<()>> + Send;
}

pub trait Checker: Send + Sync {
    fn run(
        &self,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Option<serde_json::Value>>> + Send + Sync + '_>>;
}

pub trait Tool: Sized + Send + Sync {
    type Args: for<'a> Deserialize<'a> + Send + Sync;
    type Output: Serialize + Send + Sync;
    type Error: Serialize;
    fn name(&self) -> String;
    fn definition(
        &self,
        _prompt: String,
    ) -> impl Future<Output = rig::completion::ToolDefinition> + Send + Sync;
    fn call(
        &self,
        args: Self::Args,
        workspace: &mut Box<dyn crate::workspace::WorkspaceDyn>,
    ) -> impl Future<Output = Result<Result<Self::Output, Self::Error>>> + Send + Sync;
}

type ToolDynResult = Result<Result<serde_json::Value, serde_json::Value>>;

pub trait ToolDyn: Send + Sync {
    fn name(&self) -> String;
    fn definition(
        &self,
        prompt: String,
    ) -> Pin<Box<dyn Future<Output = rig::completion::ToolDefinition> + Send + Sync + '_>>;
    fn call<'a>(
        &'a self,
        args: serde_json::Value,
        workspace: &'a mut Box<dyn WorkspaceDyn>,
    ) -> Pin<Box<dyn Future<Output = ToolDynResult> + Send + Sync + 'a>>;
}

impl<T: Tool> ToolDyn for T {
    fn name(&self) -> String {
        Tool::name(self)
    }

    fn definition(
        &self,
        prompt: String,
    ) -> Pin<Box<dyn Future<Output = rig::completion::ToolDefinition> + Send + Sync + '_>> {
        Box::pin(Tool::definition(self, prompt))
    }

    fn call<'a>(
        &'a self,
        args: serde_json::Value,
        workspace: &'a mut Box<dyn WorkspaceDyn>,
    ) -> Pin<Box<dyn Future<Output = ToolDynResult> + Send + Sync + 'a>> {
        Box::pin(async move {
            match serde_json::from_value::<<Self as Tool>::Args>(args) {
                Ok(args) => {
                    let result = Tool::call(self, args, workspace).await?;
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

// Automatically implement the Tool trait for any type that implements rig::tool::Tool

impl<T: rig::tool::Tool> Tool for T
where
    T::Output: Send + Sync,
    T::Error: Serialize + Send,
{
    type Args = T::Args;
    type Output = T::Output;
    type Error = T::Error;

    fn name(&self) -> String {
        T::name(self)
    }

    async fn definition(&self, prompt: String) -> rig::completion::ToolDefinition {
        T::definition(self, prompt).await
    }

    async fn call(
        &self,
        args: Self::Args,
        _workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        Ok(T::call(self, args).await)
    }
}
