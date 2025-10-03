use super::agent::{Event, Response};
use crate::llm::FinishReason;
use crate::toolbox::ToolDyn;
use dabgent_sandbox::DaggerSandbox;
use eyre::Result;
use rig::message::AssistantContent;

/// SandboxReplayer centralizes replay-only side effects for rebuilding the sandbox:
/// - Re-invoking tool calls from agent responses (Response::Completion with ToolCalls)
///
/// This keeps FinishHandler lean and avoids duplicating logic that also lives in ToolHandler.
/// It does not emit new events; it only applies side-effects to the sandbox for deterministic replay.
pub struct SandboxReplayer<'a> {
    pub sandbox: &'a mut DaggerSandbox,
    pub tools: &'a [Box<dyn ToolDyn>],
}

impl<'a> SandboxReplayer<'a> {
    pub fn new(sandbox: &'a mut DaggerSandbox, tools: &'a [Box<dyn ToolDyn>]) -> Self {
        Self { sandbox, tools }
    }

    /// Apply replay side-effects for a single event.
    pub async fn apply<T>(&mut self, event: &Event<T>) -> Result<()> {
        match event {
            Event::Response(Response::Completion { response })
                if response.finish_reason == FinishReason::ToolUse =>
            {
                self.replay_tool_calls(response).await?;
            }
            _ => {
                // No side-effects required for other events during replay
            }
        }
        Ok(())
    }

    /// Apply replay side-effects for a sequence of events.
    pub async fn apply_all<T>(&mut self, events: &[Event<T>]) -> Result<()> {
        for e in events {
            self.apply(e).await?;
        }
        Ok(())
    }

    async fn replay_tool_calls(&mut self, response: &crate::llm::CompletionResponse) -> Result<()> {
        tracing::debug!("Replaying tool calls from agent message during replay");
        for content in response.choice.iter() {
            if let AssistantContent::ToolCall(call) = content {
                let tool_name = &call.function.name;
                let args = call.function.arguments.clone();

                match self.tools.iter().find(|t| t.name() == *tool_name) {
                    Some(tool) => {
                        if !tool.needs_replay() {
                            tracing::debug!(
                                "Skipping replay for non-replayable tool: {}",
                                tool_name
                            );
                        } else {
                            match tool.call(args, self.sandbox).await {
                                Ok(_) => tracing::debug!("Replayed tool call: {}", tool_name),
                                Err(e) => tracing::warn!(
                                    "Failed tool call during replay {}: {:?}",
                                    tool_name,
                                    e
                                ),
                            }
                        }
                    }
                    None => {
                        tracing::warn!("Tool not found during replay: {}", tool_name);
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Note: Tests for replay functionality are disabled because DaggerSandbox requires actual Dagger runtime
    // For testing, we would need to use the SandboxHandle with a proper Dagger instance
    // or create a mock implementation of the Sandbox trait
    // In practice, replay functionality should be tested via integration tests with the full agent runtime
}
