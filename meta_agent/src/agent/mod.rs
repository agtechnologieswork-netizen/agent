use crate::workspace::WorkspaceDyn;
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tokio::sync::mpsc;
pub mod actor;
pub mod demo;
pub mod optimizer;
pub mod toolset;
pub mod tree;
pub use tree::Tree;

pub trait Search<T>: Clone + Send {
    type SearchAct;
    fn select(&mut self, root: &Tree<T>) -> impl Future<Output = Result<Self::SearchAct>> + Send;
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

pub struct Command<T> {
    pub node_seq_num: Option<usize>,
    pub cmd: T,
}

pub struct Event<T> {
    pub node_seq_num: usize,
    pub event: T,
}

/*pub trait Pipeline {
    type Checkpoint;
    type Command;
    type Event;

    fn execute(
        &mut self,
        cmd_rx: mpsc::Receiver<Self::Command>,
        event_tx: mpsc::Sender<Self::Event>,
        checkpoint: Option<Self::Checkpoint>,
    ) -> impl Future<Output = Result<Self::Checkpoint>> + Send + Sync;
}*/

pub trait Checker: Sized + Send + Sync {
    fn run<'a>(
        &'a self,
        workspace: &'a mut Box<dyn WorkspaceDyn>,
    ) -> impl Future<Output = Result<Option<serde_json::Value>>> + Send + Sync + 'a;
    fn boxed(self) -> Box<dyn CheckerDyn>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}

pub trait CheckerDyn: Send + Sync {
    fn run<'a>(
        &'a self,
        workspace: &'a mut Box<dyn WorkspaceDyn>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<serde_json::Value>>> + Send + Sync + 'a>>;
}

impl<T: Checker> CheckerDyn for T {
    fn run<'a>(
        &'a self,
        workspace: &'a mut Box<dyn WorkspaceDyn>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<serde_json::Value>>> + Send + Sync + 'a>> {
        Box::pin(self.run(workspace))
    }
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
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> impl Future<Output = Result<Result<Self::Output, Self::Error>>> + Send + Sync;
    fn boxed(self) -> Box<Self>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
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
        Box::pin(async {
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

pub trait NodeTool<T>: Tool {
    fn call_node(
        &self,
        args: Self::Args,
        node: &mut T,
    ) -> impl Future<Output = Result<Result<Self::Output, Self::Error>>> + Send + Sync;
}

pub trait NodeToolDyn<T>: Send + Sync {
    fn name(&self) -> String;
    fn definition(
        &self,
        prompt: String,
    ) -> Pin<Box<dyn Future<Output = rig::completion::ToolDefinition> + Send + Sync + '_>>;
    fn call<'a>(
        &'a self,
        args: serde_json::Value,
        node: &'a mut T,
    ) -> Pin<Box<dyn Future<Output = ToolDynResult> + Send + Sync + 'a>>;
}

impl<T: Send + Sync, U: Tool + NodeTool<T>> NodeToolDyn<T> for U {
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
        node: &'a mut T,
    ) -> Pin<Box<dyn Future<Output = ToolDynResult> + Send + Sync + 'a>> {
        Box::pin(async {
            match serde_json::from_value::<<Self as Tool>::Args>(args) {
                Ok(args) => {
                    let result = NodeTool::call_node(self, args, node).await?;
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

pub trait AgentNode {
    fn workspace_mut(&mut self) -> &mut Box<dyn WorkspaceDyn>;
}

pub enum AgentTool<N> {
    Regular(Box<dyn ToolDyn>),
    Node(Box<dyn NodeToolDyn<N>>),
}

impl<N: AgentNode> AgentTool<N> {
    pub fn name(&self) -> String {
        match self {
            AgentTool::Regular(tool) => tool.name(),
            AgentTool::Node(tool) => tool.name(),
        }
    }

    pub async fn definition(&self, prompt: String) -> rig::completion::ToolDefinition {
        match self {
            AgentTool::Regular(tool) => tool.definition(prompt).await,
            AgentTool::Node(tool) => tool.definition(prompt).await,
        }
    }

    pub async fn call(
        &self,
        args: serde_json::Value,
        node: &mut N,
    ) -> Result<Result<serde_json::Value, serde_json::Value>> {
        match self {
            AgentTool::Regular(tool) => tool.call(args, node.workspace_mut()).await,
            AgentTool::Node(tool) => tool.call(args, node).await,
        }
    }

    pub fn regular<T: ToolDyn + 'static>(tool: T) -> Self {
        AgentTool::Regular(Box::new(tool))
    }

    pub fn node<T: NodeToolDyn<N> + 'static>(tool: T) -> Self {
        AgentTool::Node(Box::new(tool))
    }
}

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

#[macro_export]
macro_rules! tools_vec {
    () => {
        Vec::<$crate::agent::AgentTool<_>>::new()
    };

    // Handle a single node tool
    (node: $tool:expr) => {
        vec![$crate::agent::AgentTool::node($tool)]
    };

    // Handle a single regular tool
    ($tool:expr) => {
        vec![$crate::agent::AgentTool::regular($tool)]
    };

    // Handle multiple items recursively
    (node: $tool:expr, $($rest:tt)*) => {
        {
            let mut tools = vec![$crate::agent::AgentTool::node($tool)];
            tools.extend(tools_vec!($($rest)*));
            tools
        }
    };

    ($tool:expr, $($rest:tt)*) => {
        {
            let mut tools = vec![$crate::agent::AgentTool::regular($tool)];
            tools.extend(tools_vec!($($rest)*));
            tools
        }
    };
}
