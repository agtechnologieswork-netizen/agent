//! LLM integration for intelligent task planning
//! This module provides LLM-powered capabilities for:
//! - Parsing natural language into structured tasks
//! - Semantic NodeKind classification

use crate::llm::{Completion, LLMClientDyn, CompletionResponse};
use crate::planner::types::NodeKind;
use crate::planner::handler::TaskPlan;
use eyre::Result;
use rig::message::{Message, AssistantContent};

/// Extract content between XML tags
fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);
    
    let start_idx = xml.find(&start_tag)?;
    let content_start = start_idx + start_tag.len();
    let end_idx = xml[content_start..].find(&end_tag)?;
    
    Some(xml[content_start..content_start + end_idx].trim().to_string())
}

/// LLM-powered planner that parses natural language into structured tasks
pub struct LLMPlanner {
    llm: Box<dyn LLMClientDyn>,
    model: String,
    system_prompt: String,
}

impl LLMPlanner {
    pub fn new(llm: Box<dyn LLMClientDyn>, model: String) -> Self {
        let system_prompt = r#"You are an expert task planner that breaks down complex requests into executable tasks.

Your responsibilities:
1. Parse natural language into a sequence of clear, actionable tasks
2. Classify each task type (Processing, ToolCall, or Clarification)
3. Preserve the user's intent while making tasks concrete

Task Types:
- Processing: Analysis, planning, implementation, or general computation tasks
- ToolCall: Tasks requiring external tools (running commands, tests, API calls)
- Clarification: Tasks that need user input or have ambiguity

Output Format:
Provide your response in structured XML format with tasks inside <tasks> tags.
Each task should have: id, description, kind.

Example:
<tasks>
<task>
  <id>1</id>
  <description>Analyze the existing codebase structure</description>
  <kind>Processing</kind>
</task>
<task>
  <id>2</id>
  <description>Run existing unit tests to understand current coverage</description>
  <kind>ToolCall</kind>
</task>
<task>
  <id>3</id>
  <description>What testing framework should we use for new tests?</description>
  <kind>Clarification</kind>
</task>
</tasks>"#.to_string();

        Self {
            llm,
            model,
            system_prompt,
        }
    }

    /// Extract text content from LLM response
    fn extract_text_from_response(&self, response: &CompletionResponse) -> Result<String> {
        for content in response.choice.iter() {
            if let AssistantContent::Text(text) = content {
                return Ok(text.text.clone());
            }
        }

        Err(eyre::eyre!("No text content in response"))
    }

    /// Helper to make LLM completion calls with consistent settings
    async fn complete(&self, prompt: String, temperature: f64, max_tokens: u64) -> Result<String> {
        let completion = Completion::new(self.model.clone(), Message::user(prompt))
            .preamble(self.system_prompt.clone())
            .temperature(temperature)
            .max_tokens(max_tokens);

        let response = self.llm.completion(completion).await?;
        self.extract_text_from_response(&response)
    }

    /// Parse natural language input into structured tasks using LLM
    pub async fn parse_tasks(&self, user_input: &str) -> Result<Vec<ParsedTask>> {
        let prompt = format!(
            r#"Parse the following user request into a sequence of executable tasks:

<request>
{}
</request>

Remember to:
- Break down complex requests into smaller, manageable tasks
- Mark ambiguous requirements as Clarification tasks
- Ensure tasks are in logical execution order"#,
            user_input
        );

        let content = self.complete(prompt, 0.3, 2000).await?;
        self.parse_llm_response(&content)
    }

    /// Parse LLM response into structured tasks
    fn parse_llm_response(&self, response: &str) -> Result<Vec<ParsedTask>> {
        let tasks_xml = extract_tag(response, "tasks")
            .ok_or_else(|| eyre::eyre!("No tasks found in LLM response"))?;

        let mut tasks = Vec::new();
        let mut current_pos = 0;

        while let Some(task_start) = tasks_xml[current_pos..].find("<task>") {
            let task_start = current_pos + task_start;
            if let Some(task_end) = tasks_xml[task_start..].find("</task>") {
                let task_end = task_start + task_end + "</task>".len();
                let task_xml = &tasks_xml[task_start..task_end];

                if let Some(task) = self.parse_single_task(task_xml) {
                    tasks.push(task);
                }

                current_pos = task_end;
            } else {
                break;
            }
        }

        Ok(tasks)
    }

    /// Parse a single task from XML
    fn parse_single_task(&self, task_xml: &str) -> Option<ParsedTask> {
        let id = extract_tag(task_xml, "id")?.parse::<u64>().ok()?;
        let description = extract_tag(task_xml, "description")?;
        let kind_str = extract_tag(task_xml, "kind")?;

        let kind = match kind_str.to_lowercase().as_str() {
            "processing" => NodeKind::Processing,
            "toolcall" | "tool" => NodeKind::ToolCall,
            "clarification" | "clarify" => NodeKind::Clarification,
            _ => NodeKind::Processing,
        };

        Some(ParsedTask {
            id,
            description,
            kind,
        })
    }

    /// Classify NodeKind using semantic understanding
    pub async fn classify_node_kind(&self, task_description: &str) -> Result<NodeKind> {
        let prompt = format!(
            r#"Classify the following task into one of these categories:

1. Processing - Analysis, planning, implementation, or general computation tasks
2. ToolCall - Tasks requiring external tools (running commands, tests, API calls)
3. Clarification - Tasks that need user input or have ambiguity

Task: "{}"

Respond with just the category name: Processing, ToolCall, or Clarification"#,
            task_description
        );

        let content = self.complete(prompt, 0.1, 10).await?;
        let kind_str = content.trim().to_lowercase();

        Ok(match kind_str.as_str() {
            "toolcall" | "tool" => NodeKind::ToolCall,
            "clarification" | "clarify" => NodeKind::Clarification,
            _ => NodeKind::Processing,
        })
    }

}

/// Parsed task from LLM
#[derive(Debug, Clone)]
pub struct ParsedTask {
    pub id: u64,
    pub description: String,
    pub kind: NodeKind,
}

impl From<ParsedTask> for TaskPlan {
    fn from(parsed: ParsedTask) -> Self {
        TaskPlan {
            id: parsed.id,
            description: parsed.description,
            kind: parsed.kind,
        }
    }
}

