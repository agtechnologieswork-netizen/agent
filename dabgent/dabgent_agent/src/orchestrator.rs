use crate::worker_orchestrator::WorkerOrchestrator;
use crate::thread;
use dabgent_mq::db::{EventStore, Query};
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use std::future::Future;
use std::pin::Pin;

/// Default system prompt for the planning agent
const DEFAULT_SYSTEM_PROMPT: &str = r#"
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.

You MUST manage your planning in a plan.md file:
1. Create plan.md when starting a new task
2. Update plan.md as you make progress
3. Use markdown checkboxes: [ ] pending, [~] in progress, [x] completed, [!] failed
4. Add notes and context as needed
5. Keep the plan organized and up-to-date
"#;

/// Simplified PlanningOrchestrator using the reusable WorkerOrchestrator
pub struct PlanningOrchestrator<S: EventStore> {
    store: S,
    stream_id: String,
    aggregate_id: String,
}

impl<S: EventStore> PlanningOrchestrator<S> {
    pub fn new(store: S, stream_id: String, aggregate_id: String) -> Self {
        Self {
            store,
            stream_id: format!("{}_planning", stream_id),
            aggregate_id,
        }
    }

    /// Setup workers using the reusable orchestrator
    pub async fn setup_workers<V>(
        &self,
        sandbox: Box<dyn SandboxDyn>,
        llm: rig::providers::anthropic::Client,
        validator: V,
    ) -> Result<()>
    where
        V: crate::toolbox::Validator + Clone + Send + Sync + 'static,
    {
        let orchestrator = WorkerOrchestrator::<S, V>::new(
            self.store.clone(),
            self.stream_id.clone(),
            self.aggregate_id.clone(),
        );

        let system_prompt = DEFAULT_SYSTEM_PROMPT.to_string();
        
        tracing::debug!("System prompt being used: {}", system_prompt);

        orchestrator.spawn_workers(llm, sandbox, system_prompt, validator).await?;
        Ok(())
    }

    /// Process a message
    pub async fn process_message(&self, content: String) -> Result<()> {
        let orchestrator = WorkerOrchestrator::<S, crate::validator::NoOpValidator>::new(
            self.store.clone(),
            self.stream_id.clone(),
            self.aggregate_id.clone(),
        );
        
        orchestrator.send_prompt(content).await
    }

    /// Monitor progress
    pub async fn monitor_progress<F>(&self, mut on_status: F) -> Result<()>
    where
        F: FnMut(String) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + 'static,
    {
        let mut receiver = self.store.subscribe::<thread::Event>(&Query {
            stream_id: self.stream_id.clone(),
            event_type: None,
            aggregate_id: Some(self.aggregate_id.clone()),
        })?;
        
        let timeout = std::time::Duration::from_secs(300);
        
        loop {
            match tokio::time::timeout(timeout, receiver.next()).await {
                Ok(Some(Ok(event))) => {
                    let status = self.format_event_status(&event).await;
                    on_status(status).await?;
                    
                    if matches!(event, thread::Event::ToolCompleted(ref resp) if self.is_done(resp)) {
                        on_status("✅ Task completed successfully!".to_string()).await?;
                        break;
                    }
                }
                Ok(Some(Err(e))) => {
                    on_status(format!("❌ Error: {}", e)).await?;
                    break;
                }
                Ok(None) => {
                    on_status("⚠️ Event stream closed".to_string()).await?;
                    break;
                }
                Err(_) => {
                    on_status("⏱️ Task timed out after 5 minutes".to_string()).await?;
                    break;
                }
            }
        }
        Ok(())
    }

    async fn format_event_status(&self, event: &thread::Event) -> String {
        match event {
            thread::Event::Prompted(task) => {
                format!("🎯 Starting task: {}", task.lines().next().unwrap_or(task))
            }
            thread::Event::LlmCompleted(_) => {
                if let Ok(content) = tokio::fs::read_to_string("plan.md").await {
                    format!("📋 Current plan:\n```markdown\n{}\n```", content)
                } else {
                    "🤔 Planning next steps...".to_string()
                }
            }
            thread::Event::ToolCompleted(_) => {
                "🔧 Executing tools...".to_string()
            }
            thread::Event::UserResponded(response) => {
                format!("💬 User responded: {}", response.content)
            }
        }
    }

    fn is_done(&self, _response: &thread::ToolResponse) -> bool {
        // Check if the response indicates completion
        // This is a simplified check - implement proper logic based on your needs
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_contains_plan_instructions() {
        // Use the constant system prompt
        let system_prompt = DEFAULT_SYSTEM_PROMPT;

        // Verify it contains plan.md instructions
        assert!(system_prompt.contains("plan.md"), "System prompt should mention plan.md");
        assert!(system_prompt.contains("Create plan.md"), "System prompt should instruct to create plan.md");
        assert!(system_prompt.contains("MUST manage your planning"), "System prompt should require planning");
    }
}