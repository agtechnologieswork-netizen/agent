//! Event Router for unified event handling between Thread and Planner systems
//! 
//! This module provides the bridge between the two event systems, allowing
//! them to work together seamlessly.

use dabgent_mq::db::{EventStore, Metadata};
use eyre::Result;
use serde::{Deserialize, Serialize};

/// Unified event type that can represent either Thread or Planner events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    /// Events from the Thread system (conversation/tool execution)
    Thread(crate::thread::Event),
    /// Events from the Planner system (task planning/orchestration)
    Planner(crate::planner::Event),
}

/// Router that handles event distribution to the appropriate system
pub struct EventRouter<E: EventStore> {
    event_store: E,
}

impl<E: EventStore> EventRouter<E> {
    /// Create a new event router with the given event store
    pub fn new(event_store: E) -> Self {
        Self { event_store }
    }

    /// Route a system event to the appropriate storage stream
    pub async fn route_event(
        &self,
        stream_id: &str,
        aggregate_id: &str,
        event: SystemEvent,
        metadata: &Metadata,
    ) -> Result<()> {
        match event {
            SystemEvent::Thread(thread_event) => {
                // Thread events go to the thread stream
                self.event_store
                    .push_event(
                        &format!("{}-thread", stream_id),
                        aggregate_id,
                        &thread_event,
                        metadata,
                    )
                    .await?;
            }
            SystemEvent::Planner(planner_event) => {
                // Planner events go to the planner stream
                self.event_store
                    .push_event(
                        &format!("{}-planner", stream_id),
                        aggregate_id,
                        &planner_event,
                        metadata,
                    )
                    .await?;
            }
        }
        Ok(())
    }

    /// Subscribe to all system events from both Thread and Planner
    pub async fn subscribe_all(
        &self,
        stream_id: &str,
        aggregate_id: Option<String>,
    ) -> Result<SystemEventStream<E>> {
        Ok(SystemEventStream {
            event_store: self.event_store.clone(),
            stream_id: stream_id.to_string(),
            aggregate_id,
        })
    }
}

/// Stream of system events from both Thread and Planner
pub struct SystemEventStream<E: EventStore> {
    event_store: E,
    stream_id: String,
    aggregate_id: Option<String>,
}

impl<E: EventStore> SystemEventStream<E> {
    /// Get the next event from either system
    pub async fn next(&mut self) -> Option<Result<SystemEvent>> {
        // This is a simplified implementation
        // In production, we'd want to merge streams from both sources
        // For now, we'll focus on planner events
        None
    }
}

/// Bridge to convert Planner commands to Thread events
pub struct PlannerThreadBridge<E: EventStore> {
    event_store: E,
}

impl<E: EventStore> PlannerThreadBridge<E> {
    /// Create a new bridge
    pub fn new(event_store: E) -> Self {
        Self { event_store }
    }

    /// Handle a task dispatch from the planner by converting it to thread events
    pub async fn handle_task_dispatch(
        &self,
        task_id: u64,
        command: crate::planner::PlannerCmd,
    ) -> Result<()> {
        use crate::planner::{NodeKind, PlannerCmd};
        use crate::thread;

        match command {
            PlannerCmd::ExecuteTask {
                node_id,
                kind,
                parameters,
            } => {
                match kind {
                    NodeKind::ToolCall | NodeKind::Processing => {
                        // Convert to Thread system prompt
                        let thread_event = thread::Event::Prompted(parameters.clone());
                        
                        // Store the event with task context
                        self.event_store
                            .push_event(
                                "thread",
                                &format!("task-{}", task_id),
                                &thread_event,
                                &Metadata::default(),
                            )
                            .await?;
                        
                        tracing::info!(
                            "Bridged task {} (node {}) to thread system",
                            task_id,
                            node_id
                        );
                    }
                    NodeKind::Clarification => {
                        // Handle clarification requests differently
                        self.handle_clarification_request(task_id, parameters).await?;
                    }
                }
            }
            PlannerCmd::RequestClarification { node_id, question } => {
                self.handle_clarification_request(task_id, question).await?;
            }
            PlannerCmd::Complete { summary } => {
                tracing::info!("Planning completed: {}", summary);
                // Could emit a completion event to the thread system
            }
        }
        
        Ok(())
    }

    /// Handle clarification requests from the planner
    async fn handle_clarification_request(
        &self,
        task_id: u64,
        question: String,
    ) -> Result<()> {
        // In a real implementation, this would:
        // 1. Pause the current task execution
        // 2. Send the question to the UI/user
        // 3. Wait for response
        // 4. Resume with the answer
        
        tracing::info!("Clarification needed for task {}: {}", task_id, question);
        
        // For now, we'll just log it
        // In Phase 3, we'll implement proper UI interaction
        
        Ok(())
    }
}

/// Convert Thread execution results back to Planner events
pub struct ThreadPlannerBridge<E: EventStore> {
    event_store: E,
}

impl<E: EventStore> ThreadPlannerBridge<E> {
    /// Create a new bridge
    pub fn new(event_store: E) -> Self {
        Self { event_store }
    }

    /// Handle thread completion and notify the planner
    pub async fn handle_thread_completion(
        &self,
        task_id: u64,
        result: String,
    ) -> Result<()> {
        use crate::planner::ExecutorEvent;

        // Create a task completion event for the planner
        let executor_event = ExecutorEvent::TaskCompleted {
            node_id: task_id,
            result: result.clone(),
        };

        // Send it to the planner via command
        // In a real implementation, this would trigger the planner's process method
        tracing::info!("Thread completed task {}: {}", task_id, result);
        
        Ok(())
    }

    /// Handle thread failure and notify the planner
    pub async fn handle_thread_failure(
        &self,
        task_id: u64,
        error: String,
    ) -> Result<()> {
        use crate::planner::ExecutorEvent;

        let executor_event = ExecutorEvent::TaskFailed {
            node_id: task_id,
            error: error.clone(),
        };

        tracing::info!("Thread failed task {}: {}", task_id, error);
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_event_creation() {
        use crate::planner;
        use crate::thread;

        // Test that we can create both types of system events
        let thread_event = thread::Event::Prompted("test".to_string());
        let system_thread = SystemEvent::Thread(thread_event);

        let planner_event = planner::Event::PlanningCompleted {
            summary: "done".to_string(),
        };
        let system_planner = SystemEvent::Planner(planner_event);

        // Just verify they can be created and serialized
        let _ = serde_json::to_string(&system_thread).unwrap();
        let _ = serde_json::to_string(&system_planner).unwrap();
    }
}
