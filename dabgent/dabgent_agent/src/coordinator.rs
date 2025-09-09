//! System Coordinator for managing all workers and event routing
//! 
//! This module provides the top-level coordination between Thread and Planner systems,
//! allowing them to work together as a unified agent.

use crate::agent::{Worker, ToolWorker, PlannerWorker};
use crate::event_router::EventRouter;
use crate::llm::LLMClient;
use dabgent_mq::EventStore;
use dabgent_sandbox::SandboxDyn;
use crate::toolbox::ToolDyn;
use eyre::Result;
use uuid::Uuid;

/// Coordinator that manages all workers and their interactions
pub struct SystemCoordinator<T: LLMClient, E: EventStore> {
    /// Worker for LLM-based thread execution
    pub llm_worker: Worker<T, E>,
    /// Worker for tool execution in sandbox
    pub tool_worker: ToolWorker<E>,
    /// Worker for task planning and orchestration
    pub planner_worker: PlannerWorker<T, E>,
    /// Router for event distribution
    pub event_router: EventRouter<E>,
}

impl<T: LLMClient, E: EventStore> SystemCoordinator<T, E> {
    /// Create a new system coordinator
    pub fn new(
        llm: T,
        event_store: E,
        sandbox: Box<dyn SandboxDyn>,
        preamble: String,
        llm_tools: Vec<Box<dyn ToolDyn>>,
        sandbox_tools: Vec<Box<dyn ToolDyn>>,
    ) -> Self {
        Self {
            llm_worker: Worker::new(
                llm.clone(),
                event_store.clone(),
                preamble.clone(),
                llm_tools,
            ),
            tool_worker: ToolWorker::new(
                sandbox,
                event_store.clone(),
                sandbox_tools,
            ),
            planner_worker: PlannerWorker::new(
                llm,
                event_store.clone(),
            ),
            event_router: EventRouter::new(event_store),
        }
    }

    /// Run all workers concurrently for a given session
    pub async fn run(&mut self, stream_id: &str, aggregate_id: &str) -> Result<()> {
        // Start all workers concurrently
        // In production, we'd use tokio::select! to handle graceful shutdown
        tokio::try_join!(
            self.llm_worker.run(stream_id, aggregate_id),
            self.tool_worker.run(stream_id, aggregate_id),
            self.planner_worker.run(stream_id, aggregate_id),
        )?;
        
        Ok(())
    }
    
    /// Initialize planning with user input and run the system
    pub async fn plan_and_execute(&mut self, user_input: String) -> Result<String> {
        // Initialize the planning process
        self.planner_worker.initialize_planning(user_input).await?;
        
        // Run the coordinated execution
        // In a real implementation, we'd want to run this with proper session management
        let session_id = Uuid::new_v4().to_string();
        
        // Start workers in background tasks
        let _stream_id = "session";
        let _aggregate_id = &session_id;
        
        // For MVP, we'll just initialize and return
        // In Phase 3, we'll implement proper async execution
        Ok(format!("Planning initialized for session: {}", session_id))
    }
    
    /// Execute a simple prompt without planning (legacy mode)
    pub async fn simple_execute(&mut self, prompt: String) -> Result<String> {
        use crate::thread::{Thread, Command, Event};
        use crate::handler::Handler;
        
        // Create a simple thread execution
        let mut thread = Thread::new();
        let events = thread.process(Command::Prompt(prompt))?;
        
        // Store events
        let session_id = Uuid::new_v4().to_string();
        for event in events {
            // Use the event router to store events
            self.event_router
                .route_event(
                    "session",
                    &session_id,
                    crate::event_router::SystemEvent::Thread(event),
                    &dabgent_mq::db::Metadata::default(),
                )
                .await?;
        }
        
        Ok(format!("Thread execution started for session: {}", session_id))
    }
}

/// Execution mode for the coordinator
#[derive(Debug, Clone)]
pub enum ExecutionMode {
    /// Use the planner for complex multi-step tasks
    Planned,
    /// Direct thread execution for simple tasks
    Direct,
    /// Automatically decide based on input complexity
    Auto,
}

impl<T: LLMClient, E: EventStore> SystemCoordinator<T, E> {
    /// Execute with a specific mode
    pub async fn execute_with_mode(
        &mut self,
        input: String,
        mode: ExecutionMode,
    ) -> Result<String> {
        match mode {
            ExecutionMode::Planned => {
                self.plan_and_execute(input).await
            }
            ExecutionMode::Direct => {
                self.simple_execute(input).await
            }
            ExecutionMode::Auto => {
                // Simple heuristic: use planner for multi-sentence or complex requests
                if input.contains(" and ") || input.contains(" then ") || input.lines().count() > 1 {
                    self.plan_and_execute(input).await
                } else {
                    self.simple_execute(input).await
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_mode() {
        // Test that execution modes can be created
        let _ = ExecutionMode::Planned;
        let _ = ExecutionMode::Direct;
        let _ = ExecutionMode::Auto;
    }
}
