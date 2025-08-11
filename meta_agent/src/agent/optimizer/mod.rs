use super::Tree;
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};

const STEP_TEMPLATE: &'static str = include_str!("./templates/formatter/step.jinja2");

#[derive(Deserialize, Serialize)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Deserialize, Serialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<String>,
}

#[derive(Deserialize, Serialize)]
pub struct Step {
    pub messages: Vec<Message>,
}

pub struct SimpleTrimmer {
    pub max_content_len: usize,
}

impl SimpleTrimmer {
    pub fn trim(&self, message: &rig::message::Message) -> Message {
        use rig::message::*;
        let mut m_content = Vec::new();
        match message {
            Message::User { content } => {
                for item in content.iter() {
                    match item {
                        UserContent::Text(text) => {
                            let s = Self::up_to_n_chars(&text.text, self.max_content_len);
                            m_content.push(s);
                        }
                        UserContent::ToolResult(tool) => {
                            let mut buffer = String::new();
                            for tool_item in tool.content.iter() {
                                match tool_item {
                                    ToolResultContent::Text(text) => {
                                        buffer.push_str(&text.text);
                                    }
                                    _ => continue,
                                }
                            }
                            let s = Self::up_to_n_chars(&buffer, self.max_content_len);
                            m_content.push(format!("tool: {} content: {}", tool.id, s));
                        }
                        _ => continue,
                    }
                }
                super::optimizer::Message {
                    role: Role::User,
                    content: m_content,
                }
            }
            Message::Assistant { content, .. } => {
                for item in content.iter() {
                    match item {
                        AssistantContent::Text(text) => {
                            let s = Self::up_to_n_chars(&text.text, self.max_content_len);
                            m_content.push(s);
                        }
                        AssistantContent::ToolCall(ToolCall { id, function, .. }) => {
                            let s = serde_json::to_string(&function.arguments).unwrap();
                            let s = Self::up_to_n_chars(&s, self.max_content_len);
                            let s = format!("tool: {} name: {} args: {}", id, function.name, s);
                            m_content.push(s);
                        }
                    }
                }
                super::optimizer::Message {
                    role: Role::Assistant,
                    content: m_content,
                }
            }
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

pub struct SimpleFormatter {
    tera: Tera,
}

impl Default for SimpleFormatter {
    fn default() -> Self {
        let mut tera = Tera::default();
        tera.add_raw_template(Self::step_template(), STEP_TEMPLATE)
            .unwrap();
        Self { tera }
    }
}

impl SimpleFormatter {
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

    let trimmer = SimpleTrimmer {
        max_content_len: 50,
    };
    let mut steps = Vec::new();
    for idx in 0..trajectory.num_nodes() {
        let mut messages = Vec::new();
        for m in trajectory.get_node(idx).history.iter() {
            messages.push(trimmer.trim(m));
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
    let formatter = SimpleFormatter::default();
    let formatted = formatter.format(&tree).unwrap();

    tracing::info!(formatted, "tree_structure");
    std::fs::write("tree_structure.txt", &formatted).unwrap();
}
