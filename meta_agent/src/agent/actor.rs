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
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::Arc};

#[derive(Deserialize, Serialize, Clone)]
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

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum NodeKind {
    Step,
    Done,
}

#[derive(Serialize, Deserialize)]
pub struct Node {
    pub kind: NodeKind,
    pub history: Vec<rig::message::Message>,
    #[serde(skip, default = "crate::workspace::mock::default_mock")]
    pub workspace: Box<dyn WorkspaceDyn>,
    pub metrics: Metrics,
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
    pub async fn tools_definitions(&self) -> Result<Vec<rig::completion::ToolDefinition>> {
        let mut definitions = Vec::new();
        for tool in self.tools.iter() {
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

    fn clear(&mut self) {
        self.locked.clear();
    }
}

pub struct AgentPipeline {
    pub rollout: AgentActor,
    pub search: SearchActor,
}

impl AgentPipeline {
    async fn handle_command(
        &mut self,
        cmd: &PipelineCmd,
        state: &mut Option<Tree<Node>>,
        event_tx: &tokio::sync::mpsc::Sender<PipelineEvent>,
    ) -> Result<()> {
        match cmd {
            PipelineCmd::Start {
                prompt,
                root_workspace,
            } => {
                let node = Node {
                    kind: NodeKind::Step,
                    history: vec![prompt.into()],
                    workspace: root_workspace.fork().await?,
                    metrics: Default::default(),
                };
                *state = Some(Tree::new(node));
                self.search_solution(state.as_mut().unwrap(), event_tx)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn search_solution(
        &mut self,
        root: &mut Tree<Node>,
        event_tx: &tokio::sync::mpsc::Sender<PipelineEvent>,
    ) -> Result<usize> {
        let mut set = tokio::task::JoinSet::new();
        loop {
            match self.search.select(root).await? {
                SearchAction::Rollout(node_ids) => {
                    for p_idx in node_ids {
                        let trajectory = self.rollout.trajectory(root, p_idx).await?;
                        let rollout = self.rollout.clone();
                        set.spawn(async move {
                            rollout.rollout(trajectory).await.map(|node| (node, p_idx))
                        });
                        let _ = event_tx.send(PipelineEvent::Scheduled(p_idx)).await;
                    }
                }
                SearchAction::Done(solution_id) => return Ok(solution_id),
            }
            match set.join_next().await {
                Some(result) => {
                    let (node, p_idx) = result??;
                    let node_id = root.add_node(node, p_idx)?;
                    self.search.unlock(p_idx)?;
                    let _ = event_tx.send(PipelineEvent::Expanded(node_id, p_idx)).await;
                }
                None => eyre::bail!("No rollouts selected"),
            }
        }
    }
}

impl super::Pipeline for AgentPipeline {
    type Checkpoint = Option<Tree<Node>>;
    type Command = PipelineCmd;
    type Event = PipelineEvent;

    async fn execute(
        &mut self,
        mut cmd_rx: tokio::sync::mpsc::Receiver<Self::Command>,
        event_tx: tokio::sync::mpsc::Sender<Self::Event>,
    ) -> Result<Self::Checkpoint> {
        let mut state: Option<Tree<Node>> = None;
        let mut command: Option<Self::Command> = None;
        loop {
            match command {
                None => command = cmd_rx.recv().await,
                Some(ref cmd) => tokio::select! {
                    res = self.handle_command(cmd, &mut state, &event_tx) => {
                        if let Err(error) = res {
                            tracing::error!(?error, "command handler");
                        }
                        return Ok(state);
                    },
                    new_cmd = cmd_rx.recv() => {command = new_cmd;},
                },
            }
        }
    }
}

pub enum PipelineCmd {
    Start {
        prompt: String,
        root_workspace: Box<dyn WorkspaceDyn + 'static>,
    },
}

#[derive(Debug)]
pub enum PipelineEvent {
    /// Scheduled rollout node parent id
    Scheduled(usize),
    /// Scheduled rollout node (node_id, parent_id)
    Expanded(usize, usize),
    Finished,
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

pub async fn run_demo_agent() -> Result<()> {
    use super::Pipeline;
    use crate::{agent::toolset, tools_vec};
    use rig::client::ProviderClient;
    use tokio::sync::mpsc;

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
    let (cmd_tx, cmd_rx) = mpsc::channel(1);
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let mut pipeline = AgentPipeline { rollout, search };
    let command = PipelineCmd::Start {
        prompt: prompt.to_string(),
        root_workspace: Box::new(workspace),
    };

    tokio::spawn(async move {
        tracing::info!("started event consumer");
        while let Some(event) = event_rx.recv().await {
            tracing::info!(?event, "event received");
        }
        tracing::info!("stopped event consumer");
    });

    tokio::spawn({
        let cmd_tx = cmd_tx.clone();
        async move {
            let _ = cmd_tx.send(command).await;
        }
    });

    let result = pipeline.execute(cmd_rx, event_tx).await?;

    if result.is_none() {
        eyre::bail!("empty state from pipeline execution");
    }

    let root = result.unwrap();
    tracing::info!("Finished with {} nodes", root.num_nodes());
    let result = serde_json::to_string_pretty(&root)?;
    std::fs::write("trajectory.json", &result)?;
    Ok(())
}
