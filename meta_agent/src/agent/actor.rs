use crate::{
    agent::{Checker, Rollout, Search, ToolDyn, ToolResult, Tree},
    llm::{Completion, CompletionResponse, LLMClientDyn},
    workspace::WorkspaceDyn,
};
use eyre::{OptionExt, Result};
use rig::{
    OneOrMany,
    message::{Message, UserContent},
};
use std::{collections::HashSet, pin::Pin, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanItem {
    pub guidance: String,
    pub files: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Step,
    Done,
    Plan { items: Vec<PlanItem> },
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
                let result = match tool {
                    Some(tool) => {
                        let args = call.function.arguments.clone();
                        let value = match tool.call(args, &mut node.workspace).await? {
                            Ok(value) => value,
                            Err(error) => serde_json::json!({"error": error}),
                        };
                        serde_json::to_string(&value)?
                    }
                    None => format!("Tool {} not found", call.function.name),
                };
                results.push(ToolResult::as_result(call, result));
            }
        }
        Ok((!results.is_empty()).then_some(results))
    }

    fn continue_message(&self) -> String {
        "continue".to_string()
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
        tracing::info!(?response, "rollout");
        let mut node = Node {
            kind: NodeKind::Step,
            history: vec![response.message()],
            workspace: trajectory.workspace,
        };
        // TODO: Catch "done" tool running and mark as completed
        let tools = self.run_tools(&response, &mut node).await?;
        let message = match tools {
            Some(tools) => {
                let tools = tools.into_iter().map(|x| UserContent::ToolResult(x));
                Message::from(OneOrMany::many(tools)?)
            }
            None => Message::from(self.continue_message()),
        };
        node.history.push(message);
        Ok(node)
    }
}

#[derive(Clone)]
pub struct SearchActor {
    locked: HashSet<usize>,
}

impl SearchActor {
    pub fn new() -> Self {
        Self {
            locked: HashSet::new(),
        }
    }
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

pub struct PythonChecker;

impl Checker for PythonChecker {
    fn run<'a>(
        &'a self,
        workspace: &'a mut Box<dyn WorkspaceDyn>,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Option<serde_json::Value>>> + Send + Sync + 'a>>
    {
        Box::pin(async {
            let _ = workspace.bash("uv run pytest").await;
            Ok(None)
        })
    }
}

pub async fn run<T: Send + 'static>(
    mut search: impl Search<T>,
    rollout: impl Rollout<T> + 'static,
    root: &mut Tree<T>,
    step_limit: usize,
) -> Result<()> {
    let mut iter = 0usize;
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
        // TODO: early out for testing
        match iter.cmp(&step_limit) {
            std::cmp::Ordering::Greater => break,
            _ => iter = iter + 1,
        }
    }
    Ok(())
}

pub async fn run_demo_agent() -> Result<()> {
    use crate::agent::toolset;
    use rig::client::ProviderClient;

    let client = rig::providers::anthropic::Client::from_env();
    let preamble = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
"
    .to_string();
    let model = "claude-sonnet-4-20250514".to_string();
    let tools: Vec<Box<dyn ToolDyn>> = vec![
        Box::new(toolset::BashTool),
        Box::new(toolset::WriteFileTool),
        Box::new(toolset::ReadFileTool),
        Box::new(toolset::LsDirTool),
        Box::new(toolset::RmFileTool),
        Box::new(toolset::EditFileTool),
    ];
    let search = SearchActor::new();
    let rollout = AgentActor {
        llm: Arc::new(client),
        tools: Arc::new(tools),
        model,
        preamble,
    };

    let dagger_ref = crate::workspace::dagger::DaggerRef::new();
    let workspace = dagger_ref
        .workspace("Dockerfile.appbuild".into(), "./src/stacks/python".into())
        .await?;
    let prompt =
        "Create a simple python script that fetches my public ip using one of the common services.";
    let mut root = Tree::new(Node {
        kind: NodeKind::Step,
        history: vec![prompt.into()],
        workspace: Box::new(workspace),
    });
    run(search, rollout, &mut root, 5).await?;
    Ok(())
}
