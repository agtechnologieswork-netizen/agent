use crate::{
    agent::{Rollout, Search, ToolDyn, Tree},
    llm::{Completion, CompletionResponse, LLMClientDyn},
    workspace::WorkspaceDyn,
};
use eyre::{OptionExt, Result};
use std::{collections::HashSet, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Step,
    Done,
}

pub struct Node {
    pub kind: NodeKind,
    pub history: Vec<rig::message::Message>,
    pub workspace: Box<dyn WorkspaceDyn>,
}

#[derive(Clone)]
pub struct AgentActor {
    pub llm: Arc<dyn LLMClientDyn>,
    pub tools: Arc<Vec<Box<dyn ToolDyn>>>,
    pub model: String,
    pub preamble: String,
}

impl AgentActor {
    // TODO: parametrize by trajectory type
    pub async fn tools_definitions(&self) -> Result<Vec<rig::completion::ToolDefinition>> {
        let mut definitions = Vec::new();
        for tool in self.tools.iter() {
            // TODO: Properly pass the prompt to rig tools
            let definition = tool.definition("".to_string()).await;
            definitions.push(definition);
        }
        Ok(definitions)
    }

    pub async fn run_tools(
        &self,
        response: &CompletionResponse,
        node: &mut Node,
    ) -> Result<Option<Vec<rig::message::ToolResult>>> {
        let mut results = Vec::new();
        for item in response.choice.iter() {
            if let rig::completion::AssistantContent::ToolCall(call) = item {
                let tool = self.tools.iter().find(|t| t.name() == call.function.name);
                let tool = match tool {
                    Some(tool) => tool,
                    None => {
                        let result = rig::message::ToolResult {
                            id: call.id.clone(),
                            call_id: call.call_id.clone(),
                            content: rig::OneOrMany::one(rig::message::ToolResultContent::text(
                                format!("Tool {} not found", call.function.name),
                            )),
                        };
                        results.push(result);
                        continue;
                    }
                };
                let args = call.function.arguments.clone();
                let result = tool.call(args, &mut node.workspace).await?;
                // TODO: Prepend {"error": ...} to the result if it is an error
                let result = match result {
                    Ok(value) => serde_json::to_string(&value)?,
                    Err(error) => serde_json::to_string(&error)?,
                };
                let result = rig::message::ToolResult {
                    id: call.id.clone(),
                    call_id: call.call_id.clone(),
                    content: rig::OneOrMany::one(rig::message::ToolResultContent::text(result)),
                };
                results.push(result);
            }
        }
        Ok((!results.is_empty()).then_some(results))
    }
}

pub struct Trajectory {
    pub message: rig::message::Message,
    pub history: Vec<rig::message::Message>,
    pub workspace: Box<dyn WorkspaceDyn>,
}

impl Rollout<Node> for AgentActor {
    type Trajectory = Trajectory;

    async fn trajectory(&self, root: &Tree<Node>, idx: usize) -> Result<Trajectory> {
        let mut trajectory = root.get_trajectory(idx);
        let mut history = Vec::new();
        for idx in trajectory.iter() {
            history.extend_from_slice(&root.get_node(*idx).history);
        }
        let message = history.pop().ok_or_eyre("Empty history")?;
        let last_idx = trajectory.pop().unwrap();
        let workspace = root.get_node(last_idx).workspace.fork().await?;
        Ok(Trajectory {
            message,
            history,
            workspace,
        })
    }

    async fn rollout(&self, trajectory: Trajectory) -> Result<Node> {
        let completion = Completion::new(self.model.clone(), trajectory.message)
            .history(trajectory.history)
            .preamble(self.preamble.clone())
            .tools(self.tools_definitions().await?)
            .temperature(1.0)
            .max_tokens(8192);
        let response = self.llm.completion(completion).await?;
        let mut node = Node {
            kind: NodeKind::Step,
            history: vec![response.message()],
            workspace: trajectory.workspace,
        };
        // TODO: Catch "done" tool running and mark as completed
        let _tool_results = self.run_tools(&response, &mut node).await?;
        // Simulate a rollout process
        Ok(node)
    }
}

#[derive(Clone)]
pub struct SearchActor {
    locked: HashSet<usize>,
}

impl Search<Node> for SearchActor {
    async fn select(&mut self, root: &Tree<Node>) -> Result<Vec<usize>> {
        let mut node_ids = Vec::new();
        for idx in root.get_leafs_idx() {
            if self.locked.contains(&idx) {
                continue;
            }
            match root.get_node(idx).kind {
                NodeKind::Done => continue,
                _ => {
                    node_ids.push(idx);
                    self.locked.insert(idx);
                }
            }
        }
        Ok(node_ids)
    }

    fn unlock(&mut self, idx: usize) -> Result<()> {
        if !self.locked.remove(&idx) {
            eyre::bail!("Node {} is not locked", idx);
        }
        Ok(())
    }
}

pub async fn run<T: Send + 'static>(
    mut search: impl Search<T>,
    rollout: impl Rollout<T> + 'static,
    root: &mut Tree<T>,
) -> Result<()> {
    let mut set = tokio::task::JoinSet::new();
    while let Ok(node_ids) = search.select(root).await {
        for p_idx in node_ids {
            let trajectory = rollout.trajectory(root, p_idx).await?;
            let rollout = rollout.clone();
            set.spawn(async move { rollout.rollout(trajectory).await.map(|node| (node, p_idx)) });
        }
        match set.join_next().await {
            Some(result) => {
                let (node, p_idx) = result??;
                root.add_node(node, p_idx).and(search.unlock(p_idx))?;
            }
            None => break,
        }
    }
    Ok(())
}
