use super::{Aggregate, Processor};
use crate::event::{Event, ParentAggregate};
use crate::processor::thread;
use dabgent_mq::{EventDb, EventStore, Query};
use regex::Regex;
use uuid::Uuid;

const COMPACTION_SYSTEM_PROMPT: &str = r#"
You are an error message compactor. Your role is to reduce verbose error messages while preserving critical debugging information.

## Objectives
- Reduce error messages to the specified character limit while maintaining clarity
- Preserve essential information: error types, file paths, line numbers, root causes
- Remove unnecessary elements: repetitive stack traces, verbose details, redundant information

## Output Format
Always wrap your compacted error message in <error> tags.

## Examples

### Example 1: Python Traceback
Input: A 800-character Python traceback with multiple stack frames
```
Traceback (most recent call last):
  File "/app/main.py", line 15, in <module>
    result = process_data()
  File "/app/main.py", line 10, in process_data
    return data.split(',')
AttributeError: 'NoneType' object has no attribute 'split'
[... additional verbose stack frames ...]
```
Output: <error>AttributeError in main.py:10 - 'NoneType' object has no attribute 'split'</error>

### Example 2: Validation Errors
Input: Verbose validation error with nested field details
```
ValidationError: Multiple validation errors occurred:
- Field 'name': This field is required and cannot be empty
- Field 'age': Value must be greater than or equal to 0
- Field 'email': Invalid email format, must contain @ symbol
[... additional validation context ...]
```
Output: <error>ValidationError: 3 fields failed - name (required), age (min:0), email (invalid format)</error>

### Example 3: Build/Compilation Errors
Input: Long compilation error with multiple issues
```
Error: Failed to compile TypeScript
src/components/UserForm.tsx(42,15): error TS2322: Type 'string' is not assignable to type 'number'
src/components/UserForm.tsx(45,8): error TS2304: Cannot find name 'useState'
[... additional compilation context ...]
```
Output: <error>TypeScript errors: UserForm.tsx:42 (string→number), :45 (useState undefined)</error>

## Instructions
Focus on the core issue and location. Remove implementation details that don't help identify the root cause.
"#;

pub struct CompactProcessor<E: EventStore> {
    event_store: E,
    compaction_threshold: usize, // Character threshold to trigger compaction
    compaction_model: String,
}

impl<E: EventStore> Processor<Event> for CompactProcessor<E> {
    async fn run(&mut self, event: &EventDb<Event>) -> eyre::Result<()> {
        match &event.data {
            Event::ToolResult(content) if self.is_done_tool_result(content) && self.should_compact(content) => {
                tracing::info!(
                    "Compaction triggered for aggregate {}",
                    event.aggregate_id,
                );
                self.handle_compaction_request(event, content).await?;
            }
            Event::AgentMessage { response, recipient } if recipient.as_deref() == Some("compact_worker") => {
                tracing::info!(
                    "Compaction received for aggregate {}",
                    event.aggregate_id,
                );
                self.handle_compaction_response(event, response).await?;
            }
            Event::ToolResult(content) if !self.is_done_tool_result(content) || !self.should_compact(content) => {
                self.handle_passthrough_tool_result(event, content).await?;
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

    async fn handle_compaction_request(
        &mut self,
        event: &EventDb<Event>,
        content: &[rig::message::ToolResult],
    ) -> eyre::Result<()> {
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
                    max_tokens: 8192,
                    preamble: Some(COMPACTION_SYSTEM_PROMPT.to_string()),
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

        Ok(())
    }

    async fn handle_compaction_response(
        &mut self,
        event: &EventDb<Event>,
        response: &crate::llm::CompletionResponse,
    ) -> eyre::Result<()> {
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

        Ok(())
    }

    async fn handle_passthrough_tool_result(
        &mut self,
        event: &EventDb<Event>,
        content: &[rig::message::ToolResult],
    ) -> eyre::Result<()> {
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

        Ok(())
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
                "Compact this error message to under {} characters:\n\n{}",
                self.compaction_threshold, text_content
            ),
        }
    }

    fn extract_tag(source: &str, tag: &str) -> Option<String> {
        // Match Python implementation: rf"<{tag}>(.*?)</{tag}>" with DOTALL
        let pattern = format!(r"(?s)<{}>(.*?)</{}>", regex::escape(tag), regex::escape(tag));
        if let Ok(regex) = Regex::new(&pattern) {
            if let Some(captures) = regex.captures(source) {
                if let Some(content) = captures.get(1) {
                    return Some(content.as_str().trim().to_string());
                }
            }
        }
        None
    }

    fn extract_compacted_content(&self, response: &crate::llm::CompletionResponse) -> String {
        let raw_response = response
            .choice
            .iter()
            .filter_map(|c| match c {
                rig::message::AssistantContent::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Try to extract content from <error> tags first
        if let Some(extracted) = Self::extract_tag(&raw_response, "error") {
            extracted
        } else {
            // If no <error> tags found, return the raw response
            tracing::warn!("LLM response did not contain <error> tags, using raw response");
            raw_response
        }
    }
}
