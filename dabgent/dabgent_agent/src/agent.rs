use crate::handler::Handler;
use crate::llm::{Completion, CompletionResponse, LLMClient};
use crate::thread::{Command, Event, Thread, ToolResponse};
use crate::toolbox::{ToolCallExt, ToolDyn};
use crate::utils::extract_tag;
use dabgent_mq::EventStore;
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use std::collections::HashMap;
use uuid;

pub struct Worker<T: LLMClient, E: EventStore> {
    llm: T,
    event_store: E,
    preamble: String,
    tools: Vec<Box<dyn ToolDyn>>,
}

impl<T: LLMClient, E: EventStore> Worker<T, E> {
    pub fn new(llm: T, event_store: E, preamble: String, tools: Vec<Box<dyn ToolDyn>>) -> Self {
        Worker {
            llm,
            event_store,
            preamble,
            tools,
        }
    }

    pub async fn run(&self, stream_id: &str, aggregate_id: &str) -> Result<()> {
        let query = dabgent_mq::db::Query {
            stream_id: stream_id.to_owned(),
            event_type: None,
            aggregate_id: Some(aggregate_id.to_owned()),
        };
        let mut receiver = self.event_store.subscribe::<Event>(&query)?;
        while let Some(event) = receiver.next().await {
            if let Err(error) = event {
                tracing::error!(?error, "llm worker");
                continue;
            }
            match event.unwrap() {
                Event::Prompted(..) | Event::ToolCompleted(..) => {
                    let events = self.event_store.load_events::<Event>(&query, None).await?;
                    let mut thread = Thread::fold(&events);
                    let completion = self.completion(&thread).await?;
                    let new_events = thread.process(Command::Completion(completion))?;
                    for event in new_events.iter() {
                        self.event_store
                            .push_event(stream_id, aggregate_id, event, &Default::default())
                            .await?;
                    }
                }
                _ => continue,
            }
        }
        Ok(())
    }

    pub async fn completion(&self, thread: &Thread) -> Result<CompletionResponse> {
        const MODEL: &str = "claude-sonnet-4-20250514";
        let mut history = thread.messages.clone();
        let message = history.pop().expect("No messages");
        let completion = Completion::new(MODEL.to_owned(), message)
            .history(history)
            .preamble(self.preamble.clone())
            .tools(self.tools.iter().map(|tool| tool.definition()).collect())
            .temperature(1.0)
            .max_tokens(8192);
        self.llm.completion(completion).await
    }
}

pub struct ToolWorker<E: EventStore> {
    sandbox: Box<dyn SandboxDyn>,
    event_store: E,
    tools: Vec<Box<dyn ToolDyn>>,
}

impl<E: EventStore> ToolWorker<E> {
    pub fn new(sandbox: Box<dyn SandboxDyn>, event_store: E, tools: Vec<Box<dyn ToolDyn>>) -> Self {
        Self {
            sandbox,
            event_store,
            tools,
        }
    }

    pub async fn run(&mut self, stream_id: &str, aggregate_id: &str) -> Result<()> {
        let query = dabgent_mq::db::Query {
            stream_id: stream_id.to_owned(),
            event_type: Some("llm_completed".to_owned()),
            aggregate_id: Some(aggregate_id.to_owned()),
        };
        let mut receiver = self.event_store.subscribe::<Event>(&query)?;
        while let Some(event) = receiver.next().await {
            match event {
                Ok(Event::LlmCompleted(response)) if Thread::has_tool_calls(&response) => {
                    let _events = self.event_store.load_events::<Event>(&query, None).await?;
                    match self.run_tools(&response).await {
                        Ok(tools) => {
                            if tools.is_empty() {
                                tracing::error!("CRITICAL: Tool execution returned empty results despite has_tool_calls=true. This indicates a serious bug.");
                                tracing::error!("Response choices: {:?}", response.choice);
                                return Err(eyre::eyre!("Tool execution returned empty results"));
                            }
                            
                            let command = {
                                let tools = tools.into_iter().map(rig::message::UserContent::ToolResult);
                                ToolResponse {
                                    content: rig::OneOrMany::many(tools).map_err(|e| eyre::eyre!("Failed to create ToolResponse: {:?}", e))?,
                                }
                            };
                            // Emit ToolCompletedRaw directly instead of going through Handler
                            let raw_event = Event::ToolCompletedRaw(command);
                            self.event_store
                                .push_event(stream_id, aggregate_id, &raw_event, &Default::default())
                                .await?;
                        }
                        Err(e) => {
                            tracing::error!("Tool execution failed: {:?}", e);
                            return Err(e);
                        }
                    }
                }
                Err(error) => {
                    tracing::error!(?error, "sandbox worker");
                }
                _ => continue,
            }
        }
        Ok(())
    }

    async fn run_tools(
        &mut self,
        response: &CompletionResponse,
    ) -> Result<Vec<rig::message::ToolResult>> {
        let mut results = Vec::new();
        for content in response.choice.iter() {
            if let rig::message::AssistantContent::ToolCall(call) = content {
                tracing::info!("Executing tool: {} with args: {:?}", call.function.name, call.function.arguments);
                
                // Handle internal export_artifacts tool
                let result = if call.function.name == "export_artifacts" {
                    // This is an internal tool not exposed to LLM
                    let path = call.function.arguments
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("/tmp/export");
                    
                    match crate::toolbox::basic::export_artifacts(&mut self.sandbox, path).await {
                        Ok(msg) => {
                            tracing::info!("Export artifacts completed: {}", msg);
                            Ok(serde_json::json!({"success": msg}))
                        }
                        Err(e) => {
                            tracing::error!("Export artifacts failed: {:?}", e);
                            Err(serde_json::json!({"error": format!("Export failed: {:?}", e)}))
                        }
                    }
                } else {
                    // Regular tool handling
                    let tool = self.tools.iter().find(|t| t.name() == call.function.name);
                    match tool {
                        Some(tool) => {
                            let args = call.function.arguments.clone();
                            match tool.call(args, &mut self.sandbox).await {
                                Ok(result) => {
                                    tracing::info!("Tool {} executed successfully", call.function.name);
                                    result
                                }
                                Err(e) => {
                                    tracing::error!("Tool {} failed: {:?}", call.function.name, e);
                                    // Convert the error to a JSON value for the tool result
                                    Err(serde_json::json!({"error": format!("Tool execution failed: {:?}", e)}))
                                }
                            }
                        }
                        None => {
                            let error = format!("{} not found", call.function.name);
                            tracing::error!("Tool not found: {}", call.function.name);
                            Err(serde_json::json!({"error": error}))
                        }
                    }
                };
                results.push(call.to_result(result));
            }
        }
        tracing::info!("Tool execution completed, {} results", results.len());
        Ok(results)
    }
}

pub struct CompactWorker<E: EventStore> {
    event_store: E,
    // Maps compaction aggregate_id to original aggregate_id
    compaction_contexts: HashMap<String, String>,
}

impl<E: EventStore> CompactWorker<E> {
    pub fn new(event_store: E) -> Self {
        Self {
            event_store,
            compaction_contexts: HashMap::new(),
        }
    }

    pub async fn run(&mut self, stream_id: &str, aggregate_id: &str) -> Result<()> {
        // Subscribe to all events to handle both ToolCompletedRaw and LlmCompleted
        let query = dabgent_mq::db::Query {
            stream_id: stream_id.to_owned(),
            event_type: None, // Subscribe to all event types
            aggregate_id: Some(aggregate_id.to_owned()), // Subscribe to specific aggregate
        };

        let mut receiver = self.event_store.subscribe::<Event>(&query)?;

        while let Some(event_result) = receiver.next().await {
            if let Ok(event) = event_result {
                match event {
                    Event::ToolCompletedRaw(response) => {
                        // Handle raw tool results from the main thread
                        self.handle_raw_tool_result(stream_id, aggregate_id, &response).await?;
                    }
                    Event::LlmCompleted(response) => {
                        // Check if this is a compaction response by looking at aggregate_ids in our mapping
                        if let Some(original_aggregate_id) = self.find_compaction_context(&response) {
                            self.handle_compaction_result(stream_id, &original_aggregate_id, &response).await?;
                        }
                    }
                    _ => continue,
                }
            }
        }
        Ok(())
    }

    async fn handle_raw_tool_result(&mut self, stream_id: &str, aggregate_id: &str, response: &ToolResponse) -> Result<()> {
        const MAX_ERROR_LENGTH: usize = 4096;
        
        // Check if the tool response contains errors that need compaction
        let needs_compaction = self.check_if_needs_compaction(response, MAX_ERROR_LENGTH);
        
        if needs_compaction {
            // Generate new aggregate_id for compaction context
            let compaction_aggregate_id = uuid::Uuid::new_v4().to_string();
            
            // Store the mapping
            self.compaction_contexts.insert(compaction_aggregate_id.clone(), aggregate_id.to_string());
            
            // Create compaction prompt
            let error_content = self.extract_error_content(response);
            let compaction_prompt = self.create_compaction_prompt(&error_content, MAX_ERROR_LENGTH);
            
            // Emit Prompted event with new aggregate_id
            let compaction_event = Event::Prompted(compaction_prompt);
            self.event_store
                .push_event(stream_id, &compaction_aggregate_id, &compaction_event, &Default::default())
                .await?;
        } else {
            // Pass through as ToolCompleted
            let completed_event = Event::ToolCompleted(response.clone());
            self.event_store
                .push_event(stream_id, aggregate_id, &completed_event, &Default::default())
                .await?;
        }
        
        Ok(())
    }

    async fn handle_compaction_result(&mut self, stream_id: &str, compaction_aggregate_id: &str, response: &CompletionResponse) -> Result<()> {
        // Extract the original aggregate_id
        if let Some(original_aggregate_id) = self.compaction_contexts.get(compaction_aggregate_id) {
            // Extract compacted error from LLM response
            let compacted_content = self.extract_compacted_result(response);
            
            // Create ToolResponse with compacted content
            let compacted_response = ToolResponse {
                content: rig::OneOrMany::one(rig::message::UserContent::text(compacted_content)),
            };
            
            // Emit ToolCompleted event with original aggregate_id
            let completed_event = Event::ToolCompleted(compacted_response);
            self.event_store
                .push_event(stream_id, original_aggregate_id, &completed_event, &Default::default())
                .await?;
            
            // Clean up the mapping
            self.compaction_contexts.remove(compaction_aggregate_id);
        }
        
        Ok(())
    }

    fn check_if_needs_compaction(&self, response: &ToolResponse, max_length: usize) -> bool {
        // Check if any content in the response exceeds the max length
        response.content.iter().any(|c| self.content_needs_compaction(c, max_length))
    }

    fn content_needs_compaction(&self, content: &rig::message::UserContent, max_length: usize) -> bool {
        match content {
            rig::message::UserContent::Text(text) => text.text.len() > max_length,
            rig::message::UserContent::ToolResult(tool_result) => {
                tool_result.content.iter().any(|tc| match tc {
                    rig::message::ToolResultContent::Text(text) => text.text.len() > max_length,
                    _ => false,
                })
            }
            _ => false,
        }
    }

    fn extract_error_content(&self, response: &ToolResponse) -> String {
        // Extract the error content that needs to be compacted
        // This is a simplified implementation
        response.content.iter().map(|c| self.content_to_string(c)).collect::<Vec<_>>().join("\n")
    }

    fn content_to_string(&self, content: &rig::message::UserContent) -> String {
        match content {
            rig::message::UserContent::Text(text) => text.text.clone(),
            rig::message::UserContent::ToolResult(tool_result) => {
                tool_result.content.iter().map(|tc| match tc {
                    rig::message::ToolResultContent::Text(text) => text.text.clone(),
                    _ => String::new(),
                }).collect::<Vec<_>>().join("\n")
            }
            _ => String::new(),
        }
    }

    fn create_compaction_prompt(&self, error_content: &str, max_length: usize) -> String {
        format!(
            r#"You need to compact an error message to be concise while keeping the most important information.
The error message is expected be reduced to be less than {} characters approximately.
Keep the key error type, file paths, line numbers, and the core issue.
Remove verbose stack traces, repeated information, and non-essential details not helping to understand the root cause.

Output the compacted error message wrapped in <error> tags.

The error message to compact is:
<message>
{}
</message>"#,
            max_length, error_content
        )
    }

    fn find_compaction_context(&self, _response: &CompletionResponse) -> Option<String> {
        // For now, we'll check if we have any compaction contexts
        // In a real implementation, we would match the response to a specific compaction context
        // based on some identifier in the response
        self.compaction_contexts.values().next().cloned()
    }

    fn extract_compacted_result(&self, response: &CompletionResponse) -> String {
        // Extract the compacted error from the LLM response
        for choice in response.choice.iter() {
            if let rig::message::AssistantContent::Text(text) = choice {
                if let Some(compacted) = extract_tag(&text.text, "error") {
                    return compacted;
                }
            }
        }
        
        // Fallback: return first text content
        for choice in response.choice.iter() {
            if let rig::message::AssistantContent::Text(text) = choice {
                return text.text.clone();
            }
        }
        
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dabgent_mq::db::{EventStore as EventStoreTrait, Metadata};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tokio::sync::mpsc;

    // Mock event store for testing
    #[derive(Clone)]
    struct MockEventStore {
        events: Arc<Mutex<Vec<(String, String, Event)>>>, // (stream_id, aggregate_id, event)
        watchers: Arc<Mutex<HashMap<dabgent_mq::db::Query, Vec<mpsc::UnboundedSender<dabgent_mq::db::Event>>>>>,
    }

    impl MockEventStore {
        fn new() -> Self {
            Self {
                events: Arc::new(Mutex::new(Vec::new())),
                watchers: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn get_events(&self) -> Vec<(String, String, Event)> {
            self.events.lock().unwrap().clone()
        }
    }

    impl EventStoreTrait for MockEventStore {
        async fn push_event<T: dabgent_mq::Event>(
            &self,
            stream_id: &str,
            aggregate_id: &str,
            event: &T,
            _metadata: &Metadata,
        ) -> Result<(), dabgent_mq::db::Error> {
            // For testing, we'll only handle Event types
            if let Ok(event_json) = serde_json::to_value(event) {
                if let Ok(typed_event) = serde_json::from_value::<Event>(event_json) {
                    self.events.lock().unwrap().push((
                        stream_id.to_string(),
                        aggregate_id.to_string(),
                        typed_event,
                    ));
                }
            }
            Ok(())
        }

        async fn load_events_raw(
            &self,
            _query: &dabgent_mq::db::Query,
            _sequence: Option<i64>,
        ) -> Result<Vec<dabgent_mq::db::Event>, dabgent_mq::db::Error> {
            Ok(Vec::new())
        }

        fn get_watchers(&self) -> &Arc<Mutex<HashMap<dabgent_mq::db::Query, Vec<mpsc::UnboundedSender<dabgent_mq::db::Event>>>>> {
            &self.watchers
        }
    }

    #[tokio::test]
    async fn test_compact_worker_handles_large_error() {
        let event_store = MockEventStore::new();
        let mut compact_worker = CompactWorker::new(event_store.clone());

        // Create a ToolResponse with a large error message (> 4096 chars)
        let large_error = "Error: ".to_string() + &"x".repeat(5000);
        let tool_response = ToolResponse {
            content: rig::OneOrMany::one(rig::message::UserContent::text(large_error)),
        };

        // Simulate handling the raw tool result
        let result = compact_worker.handle_raw_tool_result("test_stream", "test_aggregate", &tool_response).await;
        assert!(result.is_ok());

        // Check that a compaction event was emitted
        let events = event_store.get_events();
        assert!(!events.is_empty());
        
        // Should emit a Prompted event with compaction request
        let has_prompted_event = events.iter().any(|(_, _, event)| {
            matches!(event, Event::Prompted(prompt) if prompt.contains("compact an error message"))
        });
        assert!(has_prompted_event, "Should emit Prompted event for compaction");
    }

    #[tokio::test]
    async fn test_compact_worker_passes_through_small_error() {
        let event_store = MockEventStore::new();
        let mut compact_worker = CompactWorker::new(event_store.clone());

        // Create a ToolResponse with a small error message (< 4096 chars)
        let small_error = "Small error message".to_string();
        let tool_response = ToolResponse {
            content: rig::OneOrMany::one(rig::message::UserContent::text(small_error)),
        };

        // Simulate handling the raw tool result
        let result = compact_worker.handle_raw_tool_result("test_stream", "test_aggregate", &tool_response).await;
        assert!(result.is_ok());

        // Check that a ToolCompleted event was emitted directly
        let events = event_store.get_events();
        assert!(!events.is_empty());
        
        // Should emit a ToolCompleted event without compaction
        let has_completed_event = events.iter().any(|(_, _, event)| {
            matches!(event, Event::ToolCompleted(_))
        });
        assert!(has_completed_event, "Should emit ToolCompleted event directly for small errors");
    }

    #[test]
    fn test_compact_worker_check_if_needs_compaction() {
        let event_store = MockEventStore::new();
        let compact_worker = CompactWorker::new(event_store);

        // Test with large content
        let large_response = ToolResponse {
            content: rig::OneOrMany::one(rig::message::UserContent::text("x".repeat(5000))),
        };
        assert!(compact_worker.check_if_needs_compaction(&large_response, 4096));

        // Test with small content
        let small_response = ToolResponse {
            content: rig::OneOrMany::one(rig::message::UserContent::text("small".to_string())),
        };
        assert!(!compact_worker.check_if_needs_compaction(&small_response, 4096));
    }
}
