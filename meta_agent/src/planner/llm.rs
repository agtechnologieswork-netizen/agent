//! LLM integration for intelligent task planning
//! This module provides LLM-powered capabilities for:
//! - Parsing natural language into structured tasks
//! - Identifying task dependencies
//! - Semantic NodeKind classification
//! - Context compaction and summarization

use crate::llm::{Completion, LLMClientDyn, CompletionResponse};
use crate::planner::types::{NodeKind, AttachmentKind, Attachment};
use crate::planner::handler::{TaskPlan, Event};
fn extract_tag(input: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = input.find(&open)? + open.len();
    let end = input[start..].find(&close)? + start;
    Some(input[start..end].to_string())
}
use eyre::Result;
use rig::message::{Message, AssistantContent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
2. Identify dependencies between tasks
3. Classify each task type (Processing, ToolCall, or Clarification)
4. Extract URLs, file references, and other attachments
5. Preserve the user's intent while making tasks concrete

Task Types:
- Processing: Analysis, planning, implementation, or general computation tasks
- ToolCall: Tasks requiring external tools (running commands, tests, API calls)
- Clarification: Tasks that need user input or have ambiguity

Output Format:
Provide your response in structured XML format with tasks inside <tasks> tags.
Each task should have: id, description, kind, dependencies (comma-separated ids), and attachments (if any).

Example:
<tasks>
<task>
  <id>1</id>
  <description>Analyze the existing codebase structure</description>
  <kind>Processing</kind>
  <dependencies></dependencies>
</task>
<task>
  <id>2</id>
  <description>Run existing unit tests to understand current coverage</description>
  <kind>ToolCall</kind>
  <dependencies>1</dependencies>
</task>
<task>
  <id>3</id>
  <description>What testing framework should we use for new tests?</description>
  <kind>Clarification</kind>
  <dependencies>2</dependencies>
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
    
    /// Parse natural language input into structured tasks using LLM
    pub async fn parse_tasks(&self, user_input: &str) -> Result<Vec<ParsedTask>> {
        let prompt = format!(
            r#"Parse the following user request into a sequence of executable tasks:

<request>
{}
</request>

Remember to:
- Break down complex requests into smaller, manageable tasks
- Identify any URLs or file references as attachments
- Mark ambiguous requirements as Clarification tasks
- Ensure tasks are in logical execution order
- Set proper dependencies between tasks"#,
            user_input
        );
        
        let completion = Completion::new(self.model.clone(), Message::user(prompt))
            .preamble(self.system_prompt.clone())
            .temperature(0.3) // Lower temperature for more deterministic planning
            .max_tokens(2000);
        
        let response = self.llm.completion(completion).await?;
        let content = self.extract_text_from_response(&response)?;
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
        let dependencies_str = extract_tag(task_xml, "dependencies").unwrap_or_default();
        
        let kind = match kind_str.to_lowercase().as_str() {
            "processing" => NodeKind::Processing,
            "toolcall" | "tool" => NodeKind::ToolCall,
            "clarification" | "clarify" => NodeKind::Clarification,
            _ => NodeKind::Processing,
        };
        
        let dependencies = if dependencies_str.is_empty() {
            Vec::new()
        } else {
            dependencies_str
                .split(',')
                .filter_map(|s| s.trim().parse::<u64>().ok())
                .collect()
        };
        
        // Extract attachments (URLs, files) from description
        let attachments = self.extract_attachments(&description);
        
        Some(ParsedTask {
            id,
            description,
            kind,
            dependencies,
            attachments,
        })
    }
    
    /// Extract attachments from task description
    fn extract_attachments(&self, description: &str) -> Vec<Attachment> {
        let mut attachments = Vec::new();
        
        // Extract URLs
        let url_regex = regex::Regex::new(r"https?://[^\s]+").unwrap();
        for url_match in url_regex.find_iter(description) {
            let url = url_match.as_str().to_string();
            attachments.push(Attachment {
                kind: AttachmentKind::Link(url.clone()),
                label: Some(format!("URL: {}", url)),
            });
        }
        
        // Extract file paths (more precise pattern - must have path separators or start with src/etc)
        let file_regex = regex::Regex::new(r"\b(?:src/|tests/|\./)[\w/]+\.\w+\b").unwrap();
        for file_match in file_regex.find_iter(description) {
            let file = file_match.as_str().to_string();
            if !file.starts_with("http") { // Avoid URLs
                attachments.push(Attachment {
                    kind: AttachmentKind::FileRef(file.clone()),
                    label: Some(format!("File: {}", file)),
                });
            }
        }
        
        attachments
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
        
        let completion = Completion::new(self.model.clone(), Message::user(prompt))
            .temperature(0.1)
            .max_tokens(10);
        
        let response = self.llm.completion(completion).await?;
        let content = self.extract_text_from_response(&response)?;
        let kind_str = content.trim().to_lowercase();
        
        Ok(match kind_str.as_str() {
            "toolcall" | "tool" => NodeKind::ToolCall,
            "clarification" | "clarify" => NodeKind::Clarification,
            _ => NodeKind::Processing,
        })
    }
    
    /// Compact context using LLM to manage token budget
    pub async fn compact_context(
        &self,
        events: &[Event],
        token_budget: usize,
    ) -> Result<String> {
        // Build context from events
        let mut context_parts = Vec::new();
        for event in events {
            match event {
                Event::TaskStatusUpdated { task_id, status, result } => {
                    if let Some(result) = result {
                        context_parts.push(format!("Task {}: {} (Status: {:?})", task_id, result, status));
                    }
                }
                Event::ClarificationReceived { task_id, answer } => {
                    context_parts.push(format!("Clarification for task {}: {}", task_id, answer));
                }
                _ => {}
            }
        }
        
        let full_context = context_parts.join("\n");
        
        // Estimate tokens (rough approximation: 1 token ≈ 4 chars)
        let estimated_tokens = full_context.len() / 4;
        if estimated_tokens <= token_budget {
            return Ok(full_context);
        }
        
        // Use LLM to summarize
        let prompt = format!(
            r#"Summarize the following task execution context to fit within {} tokens (approximately {} characters).
Keep the most important information about completed tasks, decisions made, and key results.

Context:
{}

Provide a concise summary that preserves essential information for continuing the task sequence."#,
            token_budget,
            token_budget * 4,
            full_context
        );
        
        let completion = Completion::new(self.model.clone(), Message::user(prompt))
            .temperature(0.3)
            .max_tokens(token_budget as u64);
        
        let response = self.llm.completion(completion).await?;
        let content = self.extract_text_from_response(&response)?;
        Ok(content)
    }
    
    /// Analyze task dependencies and suggest optimal ordering
    pub async fn analyze_dependencies(&self, tasks: &[ParsedTask]) -> Result<DependencyAnalysis> {
        let task_descriptions: Vec<String> = tasks
            .iter()
            .map(|t| format!("{}: {}", t.id, t.description))
            .collect();
        
        let prompt = format!(
            r#"Analyze the dependencies between these tasks and suggest the optimal execution order.

Tasks:
{}

For each task, identify:
1. Which tasks it depends on (must complete before)
2. Which tasks can run in parallel with it
3. Any potential bottlenecks or critical path issues

Respond in JSON format:
{{
  "dependencies": {{"task_id": [dependency_ids], ...}},
  "parallel_groups": [[task_ids_that_can_run_together], ...],
  "critical_path": [task_ids_in_order],
  "bottlenecks": ["description of bottleneck", ...]
}}"#,
            task_descriptions.join("\n")
        );
        
        let completion = Completion::new(self.model.clone(), Message::user(prompt))
            .temperature(0.2)
            .max_tokens(1000);
        
        let response = self.llm.completion(completion).await?;
        let content = self.extract_text_from_response(&response)?;
        
        // Parse JSON response
        let analysis: DependencyAnalysis = serde_json::from_str(&content)
            .unwrap_or_else(|_| DependencyAnalysis::default());
        
        Ok(analysis)
    }
}

/// Parsed task from LLM
#[derive(Debug, Clone)]
pub struct ParsedTask {
    pub id: u64,
    pub description: String,
    pub kind: NodeKind,
    pub dependencies: Vec<u64>,
    pub attachments: Vec<Attachment>,
}

impl From<ParsedTask> for TaskPlan {
    fn from(parsed: ParsedTask) -> Self {
        TaskPlan {
            id: parsed.id,
            description: parsed.description,
            kind: parsed.kind,
            attachments: parsed.attachments,
        }
    }
}

/// Dependency analysis result
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependencyAnalysis {
    /// Map of task ID to its dependencies
    pub dependencies: HashMap<u64, Vec<u64>>,
    /// Groups of tasks that can run in parallel
    pub parallel_groups: Vec<Vec<u64>>,
    /// Critical path through the task graph
    pub critical_path: Vec<u64>,
    /// Identified bottlenecks
    pub bottlenecks: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Simple mock LLM client for testing
    struct TestLLMClient;
    
    impl LLMClientDyn for TestLLMClient {
        fn completion(&self, _completion: Completion) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<CompletionResponse>> + Send + '_>> {
            Box::pin(async move {
                Ok(CompletionResponse {
                    choice: rig::OneOrMany::one(AssistantContent::Text(rig::message::Text {
                        text: "test response".to_string(),
                    })),
                    finish_reason: crate::llm::FinishReason::Stop,
                    output_tokens: 0,
                    input_tokens: 0,
                    cache_read_input_tokens: None,
                    cache_creation_input_tokens: None,
                })
            })
        }
    }
    
    #[test]
    fn test_extract_attachments() {
        let planner = LLMPlanner::new(
            Box::new(TestLLMClient),
            "test".to_string(),
        );
        
        let description = "Read the API spec at https://example.com/api.pdf and analyze src/main.rs";
        let attachments = planner.extract_attachments(description);
        
        assert_eq!(attachments.len(), 2);
        assert!(matches!(&attachments[0].kind, AttachmentKind::Link(url) if url == "https://example.com/api.pdf"));
        assert!(matches!(&attachments[1].kind, AttachmentKind::FileRef(file) if file == "src/main.rs"));
    }
    
    #[test]
    fn test_parse_single_task() {
        let planner = LLMPlanner::new(
            Box::new(TestLLMClient),
            "test".to_string(),
        );
        
        let task_xml = r#"<task>
            <id>1</id>
            <description>Analyze the codebase</description>
            <kind>Processing</kind>
            <dependencies></dependencies>
        </task>"#;
        
        let task = planner.parse_single_task(task_xml).unwrap();
        assert_eq!(task.id, 1);
        assert_eq!(task.description, "Analyze the codebase");
        assert!(matches!(task.kind, NodeKind::Processing));
        assert!(task.dependencies.is_empty());
    }
}
