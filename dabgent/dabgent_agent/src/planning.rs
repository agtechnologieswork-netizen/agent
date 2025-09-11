use serde::{Deserialize, Serialize};

/// Simple planning structures that the LLM can use as a reference
/// The actual plan.md file will be managed by the LLM using its read/write tools

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub description: String,
    pub status: StepStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub task: String,
    pub steps: Vec<PlanStep>,
    pub notes: Option<String>,
}

/// Helper to generate a markdown template for a new plan
/// The LLM will use this as a starting point and then manage the file directly
pub fn generate_plan_template(task: &str) -> String {
    format!(
        r#"# Planning

## Task
{}

## Steps
- [ ] Break down the task into steps
- [ ] Update this list as you progress

## Notes
_Add any important notes or context here_

## Progress
- Started: {{timestamp}}
- Status: In Progress
"#,
        task
    )
}

/// Helper to suggest markdown format for plan updates
/// This is just a reference - the LLM will manage the actual file
pub fn suggest_step_format(step: &PlanStep) -> String {
    let checkbox = match &step.status {
        StepStatus::Completed => "[x]",
        StepStatus::InProgress => "[~]",
        StepStatus::Failed(_) => "[!]",
        StepStatus::Pending => "[ ]",
    };
    
    let mut result = format!("- {} {}", checkbox, step.description);
    
    if let StepStatus::Failed(error) = &step.status {
        result.push_str(&format!("\n  - Error: {}", error));
    }
    
    result
}

/// Constants for the LLM to reference when managing plan.md
pub const PLAN_FILE_NAME: &str = "plan.md";
pub const PLAN_INSTRUCTIONS: &str = r#"
When managing the plan.md file:
1. Use the read tool to check current state
2. Use the write/edit tool to update progress
3. Keep the markdown well-formatted
4. Update step statuses as you complete them
5. Add notes for important decisions or blockers
"#;