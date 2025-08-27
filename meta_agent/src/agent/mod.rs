use crate::workspace::WorkspaceDyn;
use eyre::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
pub mod actor;
pub mod optimizer;
//pub mod toolset;
pub mod tree;
pub use tree::Tree;

pub trait Search<T>: Clone + Send {
    type SearchAct;
    fn select(&mut self, root: &Tree<T>) -> impl Future<Output = Result<Self::SearchAct>> + Send;
    fn unlock(&mut self, idx: usize) -> Result<()>;
    fn clear(&mut self);
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

pub struct ToolRequest<T> {
    pub parent_id: usize,
    pub data: T,
}

pub trait ToolHandler<T> {
    fn handle(
        &self,
        cmd_rx: mpsc::Receiver<ToolRequest<T>>,
    ) -> impl Future<Output = Result<()>> + Send;
}

pub struct Command<T> {
    pub node_seq_num: Option<usize>,
    pub cmd: T,
}

impl<T> Command<T> {
    pub fn new(node_seq_num: Option<usize>, cmd: T) -> Self {
        Self { node_seq_num, cmd }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Event<T> {
    pub node_seq_num: usize,
    pub event: T,
}

impl<T> Event<T> {
    pub fn new(node_seq_num: usize, event: T) -> Self {
        Self {
            node_seq_num,
            event,
        }
    }
}

pub trait Pipeline {
    type Checkpoint: Serialize + for<'a> Deserialize<'a>;
    type Command: Send + Sync;
    type Event: Send + Sync;

    fn execute(
        &mut self,
        cmd_rx: mpsc::Receiver<Self::Command>,
        event_tx: mpsc::Sender<Self::Event>,
    ) -> impl Future<Output = Result<Self::Checkpoint>> + Send + Sync;
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
