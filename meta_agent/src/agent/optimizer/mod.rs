use super::{Pipeline, Tree};
use crate::{agent::Command, llm::LLMClientDyn, workspace::WorkspaceDyn};
use eyre::OptionExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tera::{Context, Tera};
use tokio::sync::mpsc;

const STEP_TEMPLATE: &'static str = include_str!("./templates/formatter/step.jinja2");

#[derive(Deserialize, Serialize, Clone, Copy)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Deserialize, Serialize)]
pub enum Content {
    Text(String),
    ToolCall {
        id: String,
        name: String,
        args: serde_json::Value,
    },
    ToolResult {
        id: String,
        text: String,
    },
}

impl std::fmt::Display for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Content::Text(text) => write!(f, "{text}"),
            Content::ToolCall { id, name, args } => {
                write!(f, "tool: {id} name: {name} args: {args}")
            }
            Content::ToolResult { id, text } => write!(f, "tool: {id} result: {text}"),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Message<T> {
    pub role: Role,
    pub content: Vec<T>,
}

impl From<&rig::message::Message> for Message<Content> {
    fn from(value: &rig::message::Message) -> Self {
        use rig::message::*;
        let mut m_content = Vec::new();
        match value {
            Message::User { content } => {
                for item in content.iter() {
                    match item {
                        UserContent::Text(text) => {
                            m_content.push(Content::Text(text.text.clone()));
                        }
                        UserContent::ToolResult(tool) => {
                            let mut buffer = Vec::new();
                            for tool_item in tool.content.iter() {
                                match tool_item {
                                    ToolResultContent::Text(text) => {
                                        buffer.push(text.text.clone());
                                    }
                                    _ => continue,
                                }
                            }
                            m_content.push(Content::ToolResult {
                                id: tool.id.clone(),
                                text: buffer.join(" "),
                            });
                        }
                        _ => continue,
                    }
                }
                Self {
                    role: Role::User,
                    content: m_content,
                }
            }
            Message::Assistant { content, .. } => {
                for item in content.iter() {
                    match item {
                        AssistantContent::Text(text) => {
                            m_content.push(Content::Text(text.text.clone()));
                        }
                        AssistantContent::ToolCall(ToolCall { id, function, .. }) => {
                            m_content.push(Content::ToolCall {
                                id: id.clone(),
                                name: function.name.clone(),
                                args: function.arguments.clone(),
                            });
                        }
                    }
                }
                Self {
                    role: Role::Assistant,
                    content: m_content,
                }
            }
        }
    }
}

impl From<&Message<Content>> for Message<String> {
    fn from(value: &Message<Content>) -> Self {
        Self {
            role: value.role,
            content: value.content.iter().map(|x| x.to_string()).collect(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Step {
    pub messages: Vec<Message<String>>,
}

pub struct Trimmer {
    pub max_len: usize,
}

impl Default for Trimmer {
    fn default() -> Self {
        Self { max_len: 50 }
    }
}

impl Trimmer {
    pub fn trim(&self, message: &Message<Content>) -> Message<String> {
        let content = message
            .content
            .iter()
            .map(|item| match item {
                Content::Text(text) => Self::up_to_n_chars(text, self.max_len),
                Content::ToolCall { id, name, args } => {
                    let args =
                        Self::up_to_n_chars(&serde_json::to_string(args).unwrap(), self.max_len);
                    format!("tool: {id} name: {name} args: {args}")
                }
                Content::ToolResult { id, text } => {
                    let result = Self::up_to_n_chars(text, self.max_len);
                    format!("tool: {id} result: {result}")
                }
            })
            .collect();
        Message {
            role: message.role,
            content: content,
        }
    }

    pub fn up_to_n_chars(s: &str, n: usize) -> String {
        if s.chars().count() <= n {
            return s.to_string();
        }
        let m = (0..=n).rfind(|m| s.is_char_boundary(*m)).unwrap();
        let substring = match s[..m].rfind(char::is_whitespace) {
            Some(k) => &s[..k],
            None => &s[..m],
        };
        format!("{substring}...")
    }
}

pub struct Formatter {
    tera: Tera,
}

impl Default for Formatter {
    fn default() -> Self {
        let mut tera = Tera::default();
        tera.add_raw_template(Self::step_template(), STEP_TEMPLATE)
            .unwrap();
        Self { tera }
    }
}

impl Formatter {
    pub fn format(&self, root: &Tree<Step>) -> eyre::Result<String> {
        let mut result = String::new();
        let mut stack = vec![(0, 0)]; // (node_idx, depth)

        while let Some((node_idx, depth)) = stack.pop() {
            let context = Context::from_serialize(root.get_node(node_idx))?;
            let messages = self.tera.render(Self::step_template(), &context)?;
            result.extend((0..depth).map(|_| "\t"));
            result.push_str(&format!("ID: {node_idx} Messages: {messages}"));

            let child_indices = root.get_children(node_idx);
            for &child_idx in child_indices.iter().rev() {
                stack.push((child_idx, depth + 1));
            }
        }

        Ok(result)
    }

    fn step_template() -> &'static str {
        "step"
    }
}

#[derive(Deserialize, Serialize)]
pub struct AgentConfig {
    pub preamble: String,
}

#[derive(Deserialize, Serialize)]
pub struct Evaluation {
    pub trajectories: Vec<Tree<super::actor::Node>>,
    pub score: f32,
    //pub metrics: HashMap<String, f32>,
}

pub struct Evaluator {
    pub pipeline: super::actor::AgentPipeline,
    pub workspace: Box<dyn WorkspaceDyn + 'static>,
    pub dataset: Vec<String>,
}

impl Evaluator {
    pub async fn evaluate(&self, config: &AgentConfig) -> eyre::Result<Evaluation> {
        let mut set = tokio::task::JoinSet::new();
        for prompt in self.dataset.iter().cloned() {
            let workspace = self.workspace.fork().await?;
            let mut pipeline = self.pipeline.clone();
            pipeline.rollout.preamble = config.preamble.clone();
            set.spawn(async move {
                let (cmd_tx, cmd_rx) = mpsc::channel(1);
                let (event_tx, mut event_rx) = mpsc::channel(1);
                let cmd = Command::new(
                    None,
                    super::actor::PipelineCmd::Start {
                        prompt: prompt.to_string(),
                        workspace: workspace,
                    },
                );

                tokio::spawn(async move { while let Some(_) = event_rx.recv().await {} });
                tokio::spawn({
                    let cmd_tx = cmd_tx.clone();
                    async move {
                        let _ = cmd_tx.send(cmd).await;
                    }
                });

                let result = pipeline.execute(cmd_rx, event_tx).await?;
                result.ok_or_eyre("no solutions")
            });
        }
        let mut trajectories = Vec::new();
        while let Some(result) = set.join_next().await {
            trajectories.push(result??);
        }
        let mut score = 0f32;
        for t in trajectories.iter() {
            for idx in 0..t.num_nodes() {
                score += t.get_node(idx).metrics.output_tokens as f32;
            }
        }
        let score = trajectories.len() as f32 / score;
        Ok(Evaluation {
            trajectories,
            score,
        })
    }
}

pub struct PromptSampler {
    pub llm: Arc<dyn LLMClientDyn>,
    pub tools: Arc<Vec<Box<dyn rig::tool::ToolDyn>>>,
    pub model: String,
}

pub fn test_step_render() {
    let mut tera = Tera::default();
    tera.add_raw_template("step", STEP_TEMPLATE).unwrap();
    let step = Step {
        messages: vec![
            Message {
                role: Role::User,
                content: vec!["hello".to_string()],
            },
            Message {
                role: Role::Assistant,
                content: vec!["world".to_string()],
            },
        ],
    };
    let context = Context::from_serialize(&step).unwrap();
    let rendered = tera.render("step", &context).unwrap();
    tracing::info!(rendered, "template");
    std::fs::write("step.txt", &rendered).unwrap();
}

pub fn test_traj_render() {
    let save = std::fs::read("trajectory.json").unwrap();
    let trajectory: Tree<super::actor::Node> = serde_json::from_slice(&save).unwrap();

    let trimmer = Trimmer::default();
    let mut steps = Vec::new();
    for idx in 0..trajectory.num_nodes() {
        let mut messages = Vec::new();
        for m in trajectory.get_node(idx).history.iter() {
            messages.push(trimmer.trim(&m.into()));
        }
        steps.push(Step { messages });
    }

    let mut tera = Tera::default();
    let template = include_str!("./templates/formatter/step.jinja2");
    tera.add_raw_template("step", template).unwrap();

    let mut lines = Vec::new();
    for step in steps.iter() {
        let context = Context::from_serialize(step).unwrap();
        let rendered = tera.render("step", &context).unwrap();
        lines.push(rendered);
    }
    std::fs::write("step_vec.txt", lines.join("")).unwrap();
}

pub fn test_simple_formatter() {
    // Create a tree with some test steps
    let root_step = Step {
        messages: vec![
            Message {
                role: Role::User,
                content: vec!["Initial user message".to_string()],
            },
            Message {
                role: Role::Assistant,
                content: vec!["Initial assistant response".to_string()],
            },
        ],
    };

    let mut tree = Tree::new(root_step);

    // Add some child nodes
    let child1_step = Step {
        messages: vec![Message {
            role: Role::User,
            content: vec!["First branch user message".to_string()],
        }],
    };

    let child2_step = Step {
        messages: vec![Message {
            role: Role::User,
            content: vec!["Second branch user message".to_string()],
        }],
    };

    let child1_idx = tree.add_node(child1_step, 0).unwrap();
    let _child2_idx = tree.add_node(child2_step, 0).unwrap();

    // Add a grandchild to first branch
    let grandchild_step = Step {
        messages: vec![Message {
            role: Role::Assistant,
            content: vec!["Grandchild response".to_string()],
        }],
    };

    tree.add_node(grandchild_step, child1_idx).unwrap();

    // Format and print the tree
    let formatter = Formatter::default();
    let formatted = formatter.format(&tree).unwrap();

    tracing::info!(formatted, "tree_structure");
    std::fs::write("tree_structure.txt", &formatted).unwrap();
}

pub fn test_traj_formatter() {
    let save = std::fs::read("trajectory.json").unwrap();
    let trajectory: Tree<super::actor::Node> = serde_json::from_slice(&save).unwrap();

    let trimmer = Trimmer::default();
    let tree = trajectory.map_nodes(|n| Step {
        messages: n.history.iter().map(|m| trimmer.trim(&m.into())).collect(),
    });

    // Format and print the tree
    let formatter = Formatter::default();
    let formatted = formatter.format(&tree).unwrap();

    tracing::info!(formatted, "tree_structure");
    std::fs::write("tree_trajectory.txt", &formatted).unwrap();
}
