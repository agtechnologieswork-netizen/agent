use crate::thread::{Event, ToolResponse};
use dabgent_mq::{db::{Query, Metadata}, EventStore};
use eyre::Result;
use serde_json::json;
use uuid::Uuid;

pub struct CompactWorker<E: EventStore> {
    event_store: E,
    max_error_length: usize,
    stream_id: String,
}

impl<E: EventStore> CompactWorker<E> {
    pub fn new(event_store: E, stream_id: String, max_error_length: usize) -> Self {
        Self {
            event_store,
            max_error_length,
            stream_id,
        }
    }

    pub async fn run(&self) -> Result<()> {
        tokio::select! {
            _ = self.handle_tool_completed_raw() => {},
            _ = self.handle_llm_completed() => {},
        }
        Ok(())
    }

    async fn handle_tool_completed_raw(&self) -> Result<()> {
        let query = Query {
            stream_id: self.stream_id.clone(),
            event_type: Some("tool_completed_raw".to_owned()),
            aggregate_id: None,
        };

        let mut receiver = self.event_store.subscribe::<Event>(&query)?;

        while let Some(event) = receiver.next_full().await {
            match event {
                Ok(event) => {
                    if let Event::ToolCompletedRaw(response) = &event.data {
                        if self.needs_compaction(response) {
                            // Create compaction thread
                            let compact_id = format!("compact_{}", Uuid::new_v4());

                            let metadata = Metadata::default()
                                .with_causation_id(event.aggregate_id.parse().unwrap_or_else(|_| Uuid::new_v4()))
                                .with_extra(json!({ "compaction": true }));

                            let prompt = self.build_compact_prompt(response);

                            self.event_store
                                .push_event(
                                    &self.stream_id,
                                    &compact_id,
                                    &Event::Prompted(prompt),
                                    &metadata,
                                )
                                .await?;
                        } else {
                            // Pass through as-is
                            self.event_store
                                .push_event(
                                    &self.stream_id,
                                    &event.aggregate_id,
                                    &Event::ToolCompleted(response.clone()),
                                    &Default::default(),
                                )
                                .await?;
                        }
                    }
                }
                Err(error) => {
                    tracing::error!(?error, "compact worker - tool completed raw");
                }
            }
        }
        Ok(())
    }

    async fn handle_llm_completed(&self) -> Result<()> {
        let query = Query {
            stream_id: self.stream_id.clone(),
            event_type: Some("llm_completed".to_owned()),
            aggregate_id: None,
        };

        let mut receiver = self.event_store.subscribe::<Event>(&query)?;

        while let Some(event) = receiver.next_full().await {
            match event {
                Ok(event) => {
                    // Only process compact_ threads
                    if !event.aggregate_id.starts_with("compact_") {
                        continue;
                    }

                    if let Event::LlmCompleted(response) = &event.data {
                        // Get parent thread from Prompted event's causation_id
                        let prompted_query = Query {
                            stream_id: self.stream_id.clone(),
                            aggregate_id: Some(event.aggregate_id.clone()),
                            event_type: Some("prompted".to_owned()),
                        };

                        if let Ok(events) = self.event_store.load_events_raw(&prompted_query, None).await {
                            if let Some(prompted) = events.first() {
                                // Parse metadata from JsonValue
                                if let Ok(metadata) = serde_json::from_value::<Metadata>(prompted.metadata.clone()) {
                                    if let Some(parent_id) = metadata.causation_id {
                                        // Extract compacted text and send back to parent
                                        let compacted_text = self.extract_text_from_completion(response);
                                        let tool_response = self.build_tool_response(&compacted_text);

                                        self.event_store
                                            .push_event(
                                                &self.stream_id,
                                                &parent_id.to_string(),
                                                &Event::ToolCompleted(tool_response),
                                                &Default::default(),
                                            )
                                            .await?;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(error) => {
                    tracing::error!(?error, "compact worker - llm completed");
                }
            }
        }
        Ok(())
    }

    fn needs_compaction(&self, response: &ToolResponse) -> bool {
        let total_length = self.estimate_response_length(response);
        total_length > self.max_error_length
    }

    fn estimate_response_length(&self, response: &ToolResponse) -> usize {
        response
            .content
            .iter()
            .map(|content| match content {
                rig::message::UserContent::Text(text) => text.text.len(),
                rig::message::UserContent::ToolResult(tool_result) => {
                    tool_result
                        .content
                        .iter()
                        .map(|content| match content {
                            rig::message::ToolResultContent::Text(text) => text.text.len(),
                            rig::message::ToolResultContent::Image(_) => 50, // Rough estimate for image refs
                        })
                        .sum()
                }
                rig::message::UserContent::Image(_) => 50, // Rough estimate for image refs
                rig::message::UserContent::Audio(_) => 100, // Rough estimate for audio refs
                rig::message::UserContent::Video(_) => 100, // Rough estimate for video refs
                rig::message::UserContent::Document(_) => 200, // Rough estimate for document refs
            })
            .sum()
    }

    fn build_compact_prompt(&self, response: &ToolResponse) -> String {
        let content = self.extract_content_for_prompt(response);
        format!(
            "Compact the following tool output to under {} characters while preserving the most critical information and error details:\n\n{}",
            self.max_error_length,
            content
        )
    }

    fn extract_content_for_prompt(&self, response: &ToolResponse) -> String {
        response
            .content
            .iter()
            .map(|content| match content {
                rig::message::UserContent::Text(text) => text.text.clone(),
                rig::message::UserContent::ToolResult(tool_result) => {
                    tool_result
                        .content
                        .iter()
                        .map(|content| match content {
                            rig::message::ToolResultContent::Text(text) => text.text.clone(),
                            rig::message::ToolResultContent::Image(_) => "[image]".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
                rig::message::UserContent::Image(_) => "[image]".to_string(),
                rig::message::UserContent::Audio(_) => "[audio]".to_string(),
                rig::message::UserContent::Video(_) => "[video]".to_string(),
                rig::message::UserContent::Document(_) => "[document]".to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn extract_text_from_completion(&self, response: &crate::llm::CompletionResponse) -> String {
        response
            .choice
            .iter()
            .filter_map(|choice| match choice {
                rig::message::AssistantContent::Text(text) => Some(text.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn build_tool_response(&self, compacted_text: &str) -> ToolResponse {
        ToolResponse {
            content: rig::OneOrMany::one(rig::message::UserContent::Text(rig::agent::Text {
                text: compacted_text.to_string(),
            })),
        }
    }
}