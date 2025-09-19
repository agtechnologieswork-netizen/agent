use crate::llm::CompletionResponse;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Prompted(String),
    LlmCompleted(CompletionResponse),
    ToolCompleted(rig::OneOrMany<rig::message::UserContent>),
    ArtifactsCollected(HashMap<String, String>),
}

impl dabgent_mq::Event for Event {
    const EVENT_VERSION: &'static str = "1.0";

    fn event_type(&self) -> &'static str {
        match self {
            Event::Prompted(..) => "prompted",
            Event::LlmCompleted(..) => "llm_completed",
            Event::ToolCompleted(..) => "tool_completed",
            Event::ArtifactsCollected(..) => "artifacts_collected",
        }
    }
}
