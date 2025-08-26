use crate::{
    agent::{
        AgentNode, AgentTool, Checker, Command, Event, NodeTool, Rollout, Search, ToolCallExt, Tree,
    },
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

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct Metrics {
    pub output_tokens: u64,
}

impl Metrics {
    fn output_tokens(mut self, output_tokens: u64) -> Self {
        self.output_tokens = output_tokens;
        self
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
    /// Files modified by this node - accumulated from tools
    pub files: std::collections::HashMap<String, String>,
}

impl AgentNode for Node {
    fn workspace_mut(&mut self) -> &mut Box<dyn WorkspaceDyn> {
        &mut self.workspace
    }

    fn files_mut(&mut self) -> &mut std::collections::HashMap<String, String> {
        &mut self.files
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
                        match tool.call(args, node).await {
                            Ok(result) => result,
                            Err(e) => {
                                tracing::warn!("Tool {} failed: {}", call.function.name, e);
                                Err(serde_json::json!(e.to_string()))
                            }
                        }
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
            files: std::collections::HashMap::new(),
        };
        let tools = self.run_tools(&response, &mut node).await?;
        let message = match tools {
            Some(tools) => {
                let tools = tools.into_iter().map(UserContent::ToolResult);
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
pub struct AgentPipeline {
    pub rollout: AgentActor,
    pub search: SearchActor,
}

impl AgentPipeline {
    async fn handle_command(
        &mut self,
        cmd: &Command<PipelineCmd>,
        state: &mut Option<Tree<Node>>,
        event_tx: &tokio::sync::mpsc::Sender<Event<PipelineEvent>>,
    ) -> Result<()> {
        match &cmd.cmd {
            PipelineCmd::Start { prompt, workspace } => {
                let node = Node {
                    kind: NodeKind::Step,
                    history: vec![prompt.into()],
                    workspace: workspace.fork().await?,
                    metrics: Default::default(),
                    files: std::collections::HashMap::new(),
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
        event_tx: &tokio::sync::mpsc::Sender<Event<PipelineEvent>>,
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
                        let event = PipelineEvent::Scheduled(p_idx);
                        let _ = event_tx.send(Event::new(root.num_nodes(), event)).await;
                    }
                }
                SearchAction::Done(solution_id) => return Ok(solution_id),
            }
            match set.join_next().await {
                Some(result) => {
                    let (node, p_idx) = result??;
                    let node_id = root.add_node(node, p_idx)?;
                    self.search.unlock(p_idx)?;
                    let event = PipelineEvent::Expanded(node_id, p_idx);
                    let _ = event_tx.send(Event::new(root.num_nodes(), event)).await;
                }
                None => eyre::bail!("No rollouts selected"),
            }
        }
    }
}

impl super::Pipeline for AgentPipeline {
    type Checkpoint = Option<Tree<Node>>;
    type Command = Command<PipelineCmd>;
    type Event = Event<PipelineEvent>;

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
        workspace: Box<dyn WorkspaceDyn + 'static>,
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

pub async fn eval_demo_agent() -> Result<()> {
    use crate::agent::optimizer::{self};
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
        preamble: preamble.clone(),
    };
    let dagger_ref = crate::workspace::dagger::DaggerRef::new();
    let workspace = dagger_ref
        .workspace("Dockerfile.appbuild".into(), "./src/stacks/python".into())
        .await?;
    let pipeline = AgentPipeline { rollout, search };
    let evaluator = optimizer::Evaluator {
        pipeline,
        workspace: Box::new(workspace),
        dataset: vec![
            "Create a simple python script that fetches my public ip using one of the common services.".to_string(),
            "Create s python script that reads a csv file using pandas and prints first 5 rows".to_string(),
        ],
        concurrency: 5,
    };
    let evaluation = evaluator
        .evaluate(&optimizer::AgentConfig {
            preamble: preamble.clone(),
        })
        .await?;

    let result = serde_json::to_string_pretty(&evaluation)?;
    std::fs::write("evaluation.json", &result)?;
    Ok(())
}

pub async fn run_nicegui_demo_agent() -> Result<()> {
    use super::Pipeline;
    use crate::{agent::toolset, stacks::nicegui, tools_vec};
    use rig::client::ProviderClient;
    use tokio::sync::mpsc;

    let client = rig::providers::anthropic::Client::from_env();
    let preamble = "
You are a NiceGUI application developer.
Workspace is already set up with a NiceGUI project template.
Use uv package manager if you need to add extra libraries.
Create modern, user-friendly web applications using NiceGUI framework.
Focus on clean code, proper data models, and comprehensive testing.
Always follow Python best practices and type safety.
"
    .to_string();
    let model = "claude-sonnet-4-20250514".to_string();
    let tools = tools_vec![
        node: toolset::WriteFileTool,
        toolset::ReadFileTool,
        toolset::LsDirTool,
        toolset::RmFileTool,
        node: toolset::EditFileTool,
        nicegui::UvAddTool,
        node: toolset::FinishTool::new(nicegui::NiceguiChecker),
    ];
    let search = SearchActor::new().with_limit(50); // Allow more exploration for complex NiceGUI apps
    let rollout = AgentActor {
        llm: Arc::new(client),
        tools: Arc::new(tools),
        model,
        preamble,
    };
    let dagger_ref = crate::workspace::dagger::DaggerRef::new();

    // Get absolute path to the nicegui template
    let current_dir = std::env::current_dir()
        .map_err(|e| eyre::eyre!("Failed to get current directory: {}", e))?;
    let template_path = current_dir
        .parent()
        .ok_or_eyre("Could not get parent directory of current working directory")?
        .join("agent/nicegui_agent/template");

    // Check if template directory exists before trying to use it
    if !template_path.exists() {
        eyre::bail!(
            "NiceGUI template directory not found at '{}'. \
            Please ensure you're running from the meta_agent directory and the agent/ directory exists.",
            template_path.display()
        );
    }

    // Use the existing Dockerfile but add dev dependencies
    let mut workspace = dagger_ref
        .workspace("Dockerfile".into(), template_path.to_string_lossy().into())
        .await
        .map_err(|e| {
            eyre::eyre!(
                "Failed to create workspace with template at '{}': {}",
                template_path.display(),
                e
            )
        })?;

    // Add dev dependencies for validation tools (like Python version does)
    tracing::info!("Installing development dependencies...");
    let dev_setup_commands = [
        "apt-get update",
        "apt-get install -y nodejs npm gcc musl-dev linux-headers-generic",
        "npm install -g pyright",
        "uv add --group dev ruff pytest pytest-asyncio pyright ast-grep-cli",
    ];

    for cmd in &dev_setup_commands {
        let result = workspace.bash(cmd).await?;
        if result.exit_code != 0 {
            return Err(eyre::eyre!(
                "Dev setup command failed: {} - {}",
                cmd,
                result.stderr
            ));
        }
    }
    let prompt = "Create a simple counter application using NiceGUI with increment/decrement buttons and persistent storage.";
    let (cmd_tx, cmd_rx) = mpsc::channel(1);
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let mut pipeline = AgentPipeline { rollout, search };
    let cmd = Command::new(
        None,
        PipelineCmd::Start {
            prompt: prompt.to_string(),
            workspace: Box::new(workspace),
        },
    );

    tokio::spawn(async move {
        tracing::info!("started nicegui event consumer");
        while let Some(event) = event_rx.recv().await {
            tracing::info!(?event, "nicegui event received");
        }
        tracing::info!("stopped nicegui event consumer");
    });

    tokio::spawn({
        let cmd_tx = cmd_tx.clone();
        async move {
            let _ = cmd_tx.send(cmd).await;
        }
    });

    let result = pipeline.execute(cmd_rx, event_tx).await?;

    if result.is_none() {
        eyre::bail!("empty state from nicegui pipeline execution");
    }

    let root = result.unwrap();
    tracing::info!("NiceGUI demo finished with {} nodes", root.num_nodes());

    // Extract generated files from the agent
    let generated_files = extract_generated_files(&root).await?;
    tracing::info!("Extracted {} generated files", generated_files.len());

    // Save files to nicegui_output directory
    save_application_files(&generated_files).await?;

    // Also save trajectory for debugging
    let trajectory_json = serde_json::to_string_pretty(&root)?;
    std::fs::write("nicegui_trajectory.json", &trajectory_json)?;

    tracing::info!("NiceGUI application saved to nicegui_output/");
    Ok(())
}

/// Extract all generated files from the final agent workspace
async fn extract_generated_files(
    root: &Tree<Node>,
) -> Result<std::collections::HashMap<String, String>> {
    let mut all_files = std::collections::HashMap::new();

    // Get all solution nodes (should be just one)
    let solution_nodes: Vec<_> = root
        .get_leafs_idx()
        .into_iter()
        .filter(|&idx| matches!(root.get_node(idx).kind, NodeKind::Done))
        .collect();

    if solution_nodes.is_empty() {
        tracing::warn!("No solution node found, extracting from all leaf nodes");
        // Fallback: collect from all nodes
        for idx in 0..root.num_nodes() {
            let node = root.get_node(idx);
            all_files.extend(node.files.clone());
        }
    } else {
        // Get the solution node and its trajectory
        let solution_idx = solution_nodes[0];
        let trajectory = root.get_trajectory(solution_idx);

        // Collect files from all nodes in the solution trajectory
        for &node_idx in &trajectory {
            let node = root.get_node(node_idx);
            all_files.extend(node.files.clone());
        }

        // Note: We collect files from the nodes which should include all files
        // modified by the agent. Template files can be copied separately if needed.
    }

    Ok(all_files)
}

/// Save application files to nicegui_output directory
async fn save_application_files(
    generated_files: &std::collections::HashMap<String, String>,
) -> Result<()> {
    let output_dir = std::path::Path::new("nicegui_output");

    // Create output directory
    if output_dir.exists() {
        std::fs::remove_dir_all(output_dir)?;
    }
    std::fs::create_dir_all(output_dir)?;

    // First, copy the template files to provide a complete base
    let template_path = std::env::current_dir()?
        .parent()
        .ok_or_eyre("Could not get parent directory")?
        .join("agent/nicegui_agent/template");

    if template_path.exists() {
        copy_directory_recursive(&template_path, output_dir)?;
        tracing::info!("Copied template files to nicegui_output/");
    }

    // Then overwrite with generated files
    for (file_path, content) in generated_files {
        let output_path = output_dir.join(file_path);

        // Create parent directories if needed
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&output_path, content)?;
        tracing::info!("Generated file: {}", file_path);
    }

    tracing::info!(
        "Saved {} generated files to nicegui_output/",
        generated_files.len()
    );
    Ok(())
}

/// Recursively copy a directory
fn copy_directory_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_directory_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
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
    let cmd = Command::new(
        None,
        PipelineCmd::Start {
            prompt: prompt.to_string(),
            workspace: Box::new(workspace),
        },
    );

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
            let _ = cmd_tx.send(cmd).await;
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
