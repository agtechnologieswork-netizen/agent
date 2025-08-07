use crate::{
    agent::{AgentNode, AgentTool, Checker, NodeTool, Rollout, Search, ToolCallExt, Tree},
    llm::{Completion, CompletionResponse, LLMClientDyn},
    workspace::WorkspaceDyn,
};
use eyre::{OptionExt, Result};
use rig::{
    OneOrMany,
    message::{Message, UserContent},
};
use std::{collections::HashSet, sync::Arc};

#[derive(serde::Serialize)]
pub struct Metrics {
    pub output_tokens: u64,
}

impl Metrics {
    fn output_tokens(mut self, output_tokens: u64) -> Self {
        self.output_tokens = output_tokens;
        self
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self { output_tokens: 0 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum NodeKind {
    Step,
    Done,
}

#[derive(serde::Serialize)]
pub struct Node {
    pub kind: NodeKind,
    pub history: Vec<rig::message::Message>,
    #[serde(skip)]
    pub workspace: Box<dyn WorkspaceDyn>,
    pub metrics: Metrics,
}

impl Node {
    pub fn root_prompt(prompt: impl Into<Message>, workspace: impl WorkspaceDyn + 'static) -> Self {
        Self {
            kind: NodeKind::Step,
            history: vec![prompt.into()],
            workspace: Box::new(workspace),
            metrics: Default::default(),
        }
    }
}

impl AgentNode for Node {
    fn workspace_mut(&mut self) -> &mut Box<dyn WorkspaceDyn> {
        &mut self.workspace
    }
}

impl NodeTool<Node> for crate::agent::toolset::FinishTool {
    async fn call_node(
        &self,
        args: Self::Args,
        node: &mut Node,
    ) -> Result<Result<Self::Output, Self::Error>> {
        use crate::agent::Tool;
        let result = Tool::call(self, args, &mut node.workspace).await?;
        if result.is_ok() {
            node.kind = NodeKind::Done;
        }
        Ok(result)
    }
}

#[derive(Clone)]
pub struct AgentActor {
    pub llm: Arc<dyn LLMClientDyn>,
    pub tools: Arc<Vec<AgentTool<Node>>>,
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
                        tool.call(args, node).await?
                    }
                    None => {
                        let error = format!("Tool {} not found", call.function.name);
                        Err(serde_json::json!(error))
                    }
                };
                results.push(call.to_result(result));
            }
        }
        Ok((!results.is_empty()).then_some(results))
    }

    fn continue_message(&self) -> String {
        "continue or complete the task".to_string()
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
            metrics: Metrics::default().output_tokens(response.output_tokens),
        };
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
        return self.limit.is_some_and(|limit| root.num_nodes() >= limit);
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
}

pub struct PythonChecker;

impl Checker for PythonChecker {
    async fn run(
        &self,
        workspace: &mut Box<dyn WorkspaceDyn>,
    ) -> Result<Option<serde_json::Value>> {
        let result = workspace.bash("uv run main.py").await?;
        Ok(match result.exit_code {
            0 => None,
            _ => Some(serde_json::json!({"stdout": result.stdout, "stderr": result.stderr})),
        })
    }
}

pub async fn run<T: Send + 'static>(
    mut search: impl Search<T, SearchAct = SearchAction>,
    rollout: impl Rollout<T> + 'static,
    root: &mut Tree<T>,
) -> Result<usize> {
    let mut set = tokio::task::JoinSet::new();
    loop {
        match search.select(root).await? {
            SearchAction::Rollout(node_ids) => {
                for p_idx in node_ids {
                    let trajectory = rollout.trajectory(root, p_idx).await?;
                    let rollout = rollout.clone();
                    set.spawn(async move {
                        rollout.rollout(trajectory).await.map(|node| (node, p_idx))
                    });
                }
            }
            SearchAction::Done(solution_id) => return Ok(solution_id),
        }
        match set.join_next().await {
            Some(result) => {
                let (node, p_idx) = result??;
                root.add_node(node, p_idx).and(search.unlock(p_idx))?;
            }
            None => eyre::bail!("No rollouts selected"),
        }
    }
}

pub async fn run_demo_agent() -> Result<()> {
    use crate::{agent::toolset, tools_vec};
    use rig::client::ProviderClient;

    let client = rig::providers::anthropic::Client::from_env();
    let preamble = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.
"
    .to_string();
    let model = "claude-sonnet-4-20250514".to_string();
    let tools = tools_vec![
        toolset::BashTool,
        toolset::WriteFileTool,
        toolset::ReadFileTool,
        toolset::LsDirTool,
        toolset::RmFileTool,
        toolset::EditFileTool,
        node: toolset::FinishTool::new(PythonChecker),
    ];
    let search = SearchActor::new().with_limit(10);
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
    let mut root = Tree::new(Node::root_prompt(prompt, workspace));
    run(search, rollout, &mut root).await?;
    tracing::info!("Finished with {} nodes", root.num_nodes());
    let result = serde_json::to_string_pretty(&root)?;
    std::fs::write("trajectory.json", &result)?;
    Ok(())
}
