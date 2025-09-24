use super::{Aggregate, Processor};
use crate::event::{Event, ParentAggregate};
use crate::processor::thread;
use dabgent_mq::{EventDb, EventStore, Query};
use uuid::Uuid;

pub struct CompactProcessor<E: EventStore> {
    event_store: E,
    compaction_threshold: usize,
    compaction_model: String,
}

impl<E: EventStore> Processor<Event> for CompactProcessor<E> {
    async fn run(&mut self, event: &EventDb<Event>) -> eyre::Result<()> {
        match &event.data {
            // Step 1: Intercept large ToolResult from DoneTool, create compaction thread
            Event::ToolResult(content) if self.is_done_tool_result(content) && self.should_compact(content) => {
                // Create compaction thread
                let compact_thread_id = format!("compact_{}", Uuid::new_v4());

                // Extract original tool_id for restoration later
                let original_tool_id = content.first().map(|result| result.id.clone());

                // Send LLMConfig first with parent tracking
                self.event_store
                    .push_event(
                        &event.stream_id,
                        &compact_thread_id,
                        &Event::LLMConfig {
                            model: self.compaction_model.clone(),
                            temperature: 0.0,
                            max_tokens: 1000,
                            preamble: Some(
                                "Extract and summarize key error information concisely."
                                    .to_string(),
                            ),
                            tools: None,
                            recipient: Some("compact_worker".to_string()),
                            parent: Some(ParentAggregate {
                                aggregate_id: event.aggregate_id.clone(),
                                tool_id: original_tool_id,
                            }),
                        },
                        &Default::default(),
                    )
                    .await?;

                // Build compaction prompt and send UserMessage
                let prompt = self.build_compaction_prompt(content);
                self.event_store
                    .push_event(
                        &event.stream_id,
                        &compact_thread_id,
                        &Event::UserMessage(rig::OneOrMany::one(
                            rig::message::UserContent::Text(prompt),
                        )),
                        &Default::default(),
                    )
                    .await?;
            }

            // Step 2: Handle compaction response - get parent info from LLMConfig
            Event::AgentMessage {
                response,
                recipient,
            } if recipient.as_deref() == Some("compact_worker") =>
            {
                // Load compaction thread to get parent info from LLMConfig
                let compact_query = Query::stream(&event.stream_id).aggregate(&event.aggregate_id);
                let compact_events = self.event_store.load_events::<Event>(&compact_query, None).await?;

                // Find the LLMConfig event to get parent info
                let parent_info = compact_events.iter()
                    .find_map(|e| match e {
                        Event::LLMConfig { parent, .. } => parent.as_ref(),
                        _ => None,
                    });

                if let Some(parent) = parent_info {
                    if let Some(tool_id) = &parent.tool_id {
                        // Extract compacted content from LLM response
                        let compacted_text = self.extract_compacted_content(response);

                        // Create compacted ToolResult with original tool_id
                        let compacted_result = vec![rig::message::ToolResult {
                            id: tool_id.clone(),
                            call_id: None,
                            content: rig::OneOrMany::one(rig::message::ToolResultContent::Text(
                                compacted_text.into()
                            )),
                        }];

                        // Convert compacted ToolResult directly to UserMessage for original thread
                        let tools = compacted_result.iter().map(|r| rig::message::UserContent::ToolResult(r.clone()));
                        let user_content = rig::OneOrMany::many(tools)?;

                        // Load original thread state and process
                        let original_query = Query::stream(&event.stream_id).aggregate(&parent.aggregate_id);
                        let events = self.event_store.load_events::<Event>(&original_query, None).await?;
                        let mut thread = thread::Thread::fold(&events);
                        let new_events = thread.process(thread::Command::User(user_content))?;

                        for new_event in new_events.iter() {
                            self.event_store
                                .push_event(
                                    &event.stream_id,
                                    &parent.aggregate_id,
                                    new_event,
                                    &Default::default(),
                                )
                                .await?;
                        }
                    }
                }
            }

            // Step 3: Convert non-compacted ToolResult to UserMessage for original thread
            Event::ToolResult(content) if !self.is_done_tool_result(content) || !self.should_compact(content) => {
                // Convert to UserMessage for original thread (same aggregate)
                let tools = content.iter().map(|r| rig::message::UserContent::ToolResult(r.clone()));
                let user_content = rig::OneOrMany::many(tools)?;

                // Load original thread state and process
                let original_query = Query::stream(&event.stream_id).aggregate(&event.aggregate_id);
                let events = self.event_store.load_events::<Event>(&original_query, None).await?;
                let mut thread = thread::Thread::fold(&events);
                let new_events = thread.process(thread::Command::User(user_content))?;

                for new_event in new_events.iter() {
                    self.event_store
                        .push_event(
                            &event.stream_id,
                            &event.aggregate_id,
                            new_event,
                            &Default::default(),
                        )
                        .await?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl<E: EventStore> CompactProcessor<E> {
    pub fn new(
        event_store: E,
        compaction_threshold: usize,
        compaction_model: String,
    ) -> Self {
        Self {
            event_store,
            compaction_threshold,
            compaction_model,
        }
    }

    fn is_done_tool_result(&self, results: &[rig::message::ToolResult]) -> bool {
        // Check if any of the tool results look like they came from DoneTool
        // DoneTool returns "success" on success or "validation error: ..." on failure
        results.iter().any(|result| {
            result.content.iter().any(|content| {
                if let rig::message::ToolResultContent::Text(text) = content {
                    let text_content = &text.text;
                    // Check for patterns typical of DoneTool output
                    text_content == "success"
                        || text_content.starts_with("validation error:")
                        || text_content.contains("validation error")
                } else {
                    false
                }
            })
        })
    }

    fn should_compact(&self, results: &[rig::message::ToolResult]) -> bool {
        let size = self.calculate_text_size(results);
        size > self.compaction_threshold
    }

    fn calculate_text_size(&self, results: &[rig::message::ToolResult]) -> usize {
        results
            .iter()
            .map(|result| {
                result
                    .content
                    .iter()
                    .map(|content| match content {
                        rig::message::ToolResultContent::Text(text) => text.text.len(),
                        _ => 0, // Skip non-text content for size calculation
                    })
                    .sum::<usize>()
            })
            .sum()
    }


    fn extract_text_content(&self, results: &[rig::message::ToolResult]) -> String {
        results
            .iter()
            .flat_map(|result| {
                result.content.iter().filter_map(|content| match content {
                    rig::message::ToolResultContent::Text(text) => Some(text.text.clone()),
                    _ => None, // Skip non-text content
                })
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn build_compaction_prompt(&self, content: &[rig::message::ToolResult]) -> rig::message::Text {
        let text_content = self.extract_text_content(content);
        rig::message::Text {
            text: format!(
                "Compact this error message to under {} characters. \
                 Keep error types, file paths, line numbers, and core issues. \
                 Remove verbose stack traces and repetition.\n\n{}",
                self.compaction_threshold, text_content
            ),
        }
    }

    fn extract_compacted_content(&self, response: &crate::llm::CompletionResponse) -> String {
        response
            .choice
            .iter()
            .filter_map(|c| match c {
                rig::message::AssistantContent::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}