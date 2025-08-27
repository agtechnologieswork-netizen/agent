use crate::agent::{Search, Tree};
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum NodeKind {
    /// Base step performed by the agent
    Step,
    /// Tool execution requested
    Tool,
    /// Done node indicating the end of the agent's actions
    Done,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Node {
    pub kind: NodeKind,
    pub history: Vec<rig::message::Message>,
    pub metrics: Metrics,
}

pub enum SearchAction {
    Rollout(Vec<usize>),
    Done(usize),
}

#[derive(Clone)]
pub struct SearchActor {
    locked: HashSet<usize>,
    limit: Option<usize>,
}

impl SearchActor {
    pub fn new() -> Self {
        Self {
            locked: HashSet::new(),
            limit: None,
        }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn is_exhausted(&self, root: &Tree<Node>) -> bool {
        self.limit.is_some_and(|limit| root.num_nodes() >= limit)
    }
}

impl Search<Node> for SearchActor {
    type SearchAct = SearchAction;

    async fn select(&mut self, root: &Tree<Node>) -> Result<Self::SearchAct> {
        if self.is_exhausted(root) {
            eyre::bail!("Search limit exhausted after {} steps.", root.num_nodes());
        }
        let mut node_ids = Vec::new();
        for idx in root.get_leafs_idx() {
            if self.locked.contains(&idx) {
                continue;
            }
            match root.get_node(idx).kind {
                NodeKind::Done => return Ok(SearchAction::Done(idx)),
                _ => {
                    node_ids.push(idx);
                    self.locked.insert(idx);
                }
            }
        }
        Ok(SearchAction::Rollout(node_ids))
    }

    fn unlock(&mut self, idx: usize) -> Result<()> {
        if !self.locked.remove(&idx) {
            eyre::bail!("Node {} is not locked", idx);
        }
        Ok(())
    }

    fn clear(&mut self) {
        self.locked.clear();
    }
}

impl Default for SearchActor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct AgentActor {}

#[derive(Clone)]
pub struct AgentPipeline {
    pub rollout: AgentActor,
    pub search: SearchActor,
}

pub enum PipelineCmd {
    Start { prompt: String },
}

#[derive(Debug)]
pub enum PipelineEvent {
    /// Scheduled rollout node parent id
    Scheduled(usize),
    /// Scheduled rollout node (node_id, parent_id)
    Expanded(usize, usize),
    /// Completed solution search
    Finished,
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

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct Metrics {
    pub output_tokens: u64,
}

impl Metrics {
    pub fn output_tokens(mut self, output_tokens: u64) -> Self {
        self.output_tokens = output_tokens;
        self
    }
}
