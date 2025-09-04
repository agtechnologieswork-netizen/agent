//! LLM-enhanced planner implementation
//! This module extends the basic planner with LLM capabilities for intelligent task planning

use crate::planner::{
    handler::{Command, Event, Handler, TaskPlan, PlannerError},
    types::{PlannerState, PlannerConfig},
    llm::{LLMPlanner, ParsedTask, DependencyAnalysis},
    Planner,
};
use crate::llm::LLMClientDyn;
use std::sync::Arc;

/// LLM-enhanced planner that uses AI for intelligent task planning
pub struct LLMEnhancedPlanner {
    /// Core planner for state management and event processing
    base_planner: Planner,
    /// LLM planner for intelligent parsing and analysis
    llm_planner: Option<Arc<LLMPlanner>>,
    /// Configuration (unused in MVP but kept for future)
    #[allow(dead_code)]
    config: PlannerConfig,
}

impl LLMEnhancedPlanner {
    /// Create a new LLM-enhanced planner
    pub fn new(llm: Box<dyn LLMClientDyn>, model: String, config: PlannerConfig) -> Self {
        let llm_planner = LLMPlanner::new(llm, model);
        
        Self {
            base_planner: Planner::new(),
            llm_planner: Some(Arc::new(llm_planner)),
            config,
        }
    }
    
    /// Create a planner without LLM (fallback to basic parsing)
    pub fn new_basic(config: PlannerConfig) -> Self {
        Self {
            base_planner: Planner::new(),
            llm_planner: None,
            config,
        }
    }
    
    /// Get the current state
    pub fn state(&self) -> &PlannerState {
        self.base_planner.state()
    }
    
    /// Get event history
    pub fn events(&self) -> &[Event] {
        self.base_planner.events()
    }
    
    /// Parse input using LLM if available, fallback to basic parsing
    async fn parse_input_intelligent(&self, user_input: &str) -> Result<Vec<TaskPlan>, PlannerError> {
        if let Some(llm) = &self.llm_planner {
            // Use LLM for intelligent parsing
            match llm.parse_tasks(user_input).await {
                Ok(parsed_tasks) => {
                    // Convert ParsedTask to TaskPlan
                    let task_plans: Vec<TaskPlan> = parsed_tasks
                        .into_iter()
                        .map(|t| t.into())
                        .collect();
                    
                    Ok(task_plans)
                }
                Err(e) => {
                    // Log error and fallback to basic parsing
                    eprintln!("LLM parsing failed: {}, falling back to basic parsing", e);
                    self.parse_input_basic(user_input)
                }
            }
        } else {
            // No LLM, use basic parsing
            self.parse_input_basic(user_input)
        }
    }
    
    /// Basic parsing fallback (MVP: single task)
    fn parse_input_basic(&self, user_input: &str) -> Result<Vec<TaskPlan>, PlannerError> {
        // MVP: Just create a single processing task
        Ok(vec![TaskPlan {
            id: self.base_planner.state().next_id,
            description: user_input.to_string(),
            kind: crate::planner::types::NodeKind::Processing,
            attachments: Vec::new(),
        }])
    }
    
    /// Process command with optional LLM enhancement
    pub async fn process_async(&mut self, command: Command) -> Result<Vec<Event>, PlannerError> {
        match command {
            Command::Initialize { user_input, attachments } => {
                // Use LLM for intelligent parsing
                let tasks = self.parse_input_intelligent(&user_input).await?;
                
                // Add attachments to first task if any
                let mut tasks_with_attachments = tasks;
                if !attachments.is_empty() && !tasks_with_attachments.is_empty() {
                    tasks_with_attachments[0].attachments = attachments;
                }
                
                // Use base planner for event processing
                self.base_planner.process(Command::Initialize {
                    user_input,
                    attachments: vec![],
                })
            }
            
            Command::CompactContext { max_tokens } => {
                // Use LLM for intelligent context compaction if available
                if let Some(llm) = &self.llm_planner {
                    let events = self.base_planner.events();
                    match llm.compact_context(events, max_tokens).await {
                        Ok(compacted) => {
                            // Create a custom event with the compacted context
                            let event = Event::ContextCompacted {
                                summary: compacted,
                                removed_task_ids: vec![], // TODO: Track which tasks were summarized
                            };
                            Ok(vec![event])
                        }
                        Err(_) => {
                            // Fallback to basic compaction
                            self.base_planner.process(Command::CompactContext { max_tokens })
                        }
                    }
                } else {
                    // No LLM, use basic compaction
                    self.base_planner.process(Command::CompactContext { max_tokens })
                }
            }
            
            _ => {
                // For other commands, use base planner as-is
                self.base_planner.process(command)
            }
        }
    }
    
    /// Analyze task dependencies using LLM
    pub async fn analyze_dependencies(&self) -> Result<DependencyAnalysis, PlannerError> {
        if let Some(llm) = &self.llm_planner {
            let tasks: Vec<ParsedTask> = self.base_planner.state().tasks
                .iter()
                .map(|t| ParsedTask {
                    id: t.id,
                    description: t.description.clone(),
                    kind: t.kind,
                    dependencies: Vec::new(),
                    attachments: t.attachments.clone(),
                })
                .collect();
            
            llm.analyze_dependencies(&tasks)
                .await
                .map_err(|e| PlannerError::ExternalError(e.to_string()))
        } else {
            Err(PlannerError::ExternalError("No LLM configured".to_string()))
        }
    }
}

// For synchronous Handler trait compatibility
impl Handler for LLMEnhancedPlanner {
    type Command = Command;
    type Event = Event;
    type Error = PlannerError;
    
    fn process(&mut self, command: Command) -> Result<Vec<Event>, PlannerError> {
        // For sync interface, use base planner (no LLM enhancement)
        self.base_planner.process(command)
    }
    
    fn fold(events: &[Event]) -> Self {
        Self {
            base_planner: Planner::fold(events),
            llm_planner: None,
            config: PlannerConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_planner_creation() {
        let planner = LLMEnhancedPlanner::new_basic(PlannerConfig::default());
        assert!(planner.llm_planner.is_none());
    }
    
    #[test]
    fn test_parse_input_basic() {
        let planner = LLMEnhancedPlanner::new_basic(PlannerConfig::default());
        let tasks = planner.parse_input_basic("Task 1\nTask 2\nWhat should I do?").unwrap();
        
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].description, "Task 1");
        assert_eq!(tasks[1].description, "Task 2");
        assert_eq!(tasks[2].description, "What should I do?");
        assert!(matches!(tasks[2].kind, crate::planner::types::NodeKind::Clarification));
    }
}
