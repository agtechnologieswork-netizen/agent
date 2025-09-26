use super::{Aggregate, Processor};
use crate::event::Event;
use crate::processor::thread;
use dabgent_mq::{EventDb, EventStore, Query};
use eyre::Result;

pub mod databricks;

pub trait DelegationHandler: Send + Sync {
    fn trigger_tool(&self) -> &str;
    fn thread_prefix(&self) -> &str;
    fn handle(
        &self,
        catalog: &str,
        prompt: &str,
        model: &str,
        parent_aggregate_id: &str,
        parent_tool_id: &str
    ) -> Result<(String, Event, Event)>;
    fn format_result(&self, summary: &str) -> String;
}

pub struct DelegationProcessor<E: EventStore> {
    event_store: E,
    default_model: String,
    handlers: Vec<Box<dyn DelegationHandler>>,
}

impl<E: EventStore> Processor<Event> for DelegationProcessor<E> {
    async fn run(&mut self, event: &EventDb<Event>) -> eyre::Result<()> {
        match &event.data {
            Event::ToolResult(tool_results) if self.is_delegation_tool_result(tool_results) => {
                tracing::info!(
                    "Delegation tool result detected for aggregate {}",
                    event.aggregate_id
                );
                self.handle_delegation_request(event, tool_results).await?;
            }
            Event::ToolResult(tool_results) if !self.is_delegation_tool_result(tool_results) => {
                // Skip non-delegation tool results - they're handled by their respective ToolProcessors
            }
            Event::WorkComplete { result, .. } if self.is_delegated_thread(&event.aggregate_id) => {
                tracing::info!(
                    "Delegated work completed successfully for aggregate {}",
                    event.aggregate_id,
                );
                self.handle_work_completion(event, result).await?;
            }
            _ => {}
        }
        Ok(())
    }
}

impl<E: EventStore> DelegationProcessor<E> {
    pub fn new(event_store: E, default_model: String, handlers: Vec<Box<dyn DelegationHandler>>) -> Self {
        Self {
            event_store,
            default_model,
            handlers,
        }
    }

    fn is_delegated_thread(&self, aggregate_id: &str) -> bool {
        self.handlers.iter().any(|h| aggregate_id.starts_with(h.thread_prefix()))
    }

    fn is_delegation_tool_result(&self, tool_results: &[crate::event::TypedToolResult]) -> bool {
        tool_results.iter().any(|result| {
            if let crate::event::ToolKind::Other(tool_name) = &result.tool_name {
                self.handlers.iter().any(|h| tool_name == h.trigger_tool())
            } else {
                false
            }
        })
    }

    async fn handle_delegation_request(
        &mut self,
        event: &EventDb<Event>,
        tool_results: &[crate::event::TypedToolResult],
    ) -> eyre::Result<()> {
        // Find the delegation tool result
        let delegation_result = tool_results.iter().find(|result| {
            if let crate::event::ToolKind::Other(tool_name) = &result.tool_name {
                self.handlers.iter().any(|h| tool_name == h.trigger_tool())
            } else {
                false
            }
        });

        if let Some(delegation_result) = delegation_result {
            let parent_tool_id = delegation_result.result.id.clone();

            // Find matching handler index
            let handler_idx = if let crate::event::ToolKind::Other(tool_name) = &delegation_result.tool_name {
                self.handlers.iter().position(|h| tool_name == h.trigger_tool())
            } else {
                None
            };

            if let Some(handler_idx) = handler_idx {
                // Load events to find the original tool call with arguments
                let query = Query::stream(&event.stream_id).aggregate(&event.aggregate_id);
                let events = self.event_store.load_events::<Event>(&query, None).await?;

                // Find the most recent AgentMessage with the matching tool call
                let tool_call = events.iter().rev()
                    .find_map(|e| match e {
                        Event::AgentMessage { response, .. } => {
                            response.choice.iter().find_map(|content| {
                                if let rig::message::AssistantContent::ToolCall(call) = content {
                                    if call.id == parent_tool_id {
                                        Some(call)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                        }
                        _ => None,
                    });

                if let Some(tool_call) = tool_call {
                    // Extract arguments from the tool call
                    let catalog = tool_call.function.arguments.get("catalog")
                        .and_then(|v| v.as_str())
                        .unwrap_or("main");
                    let prompt_arg = tool_call.function.arguments.get("prompt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Explore the catalog for relevant data");

                    self.handle_delegation_by_index(event, handler_idx, catalog, prompt_arg, &parent_tool_id).await?;
                } else {
                    tracing::warn!("Could not find original tool call for delegation, using defaults");
                    let catalog = "main";
                    let prompt_arg = "Explore bakery business data, focusing on products, sales, customers, and orders.";
                    self.handle_delegation_by_index(event, handler_idx, catalog, prompt_arg, &parent_tool_id).await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_delegation_by_index(
        &mut self,
        event: &EventDb<Event>,
        handler_idx: usize,
        catalog: &str,
        prompt_arg: &str,
        parent_tool_id: &str,
    ) -> eyre::Result<()> {
        let (task_thread_id, config_event, user_event) = self.handlers[handler_idx].handle(
            catalog,
            prompt_arg,
            &self.default_model,
            &event.aggregate_id,
            parent_tool_id,
        )?;

        // Send LLMConfig first with parent tracking
        self.event_store
            .push_event(
                &event.stream_id,
                &task_thread_id,
                &config_event,
                &Default::default(),
            )
            .await?;

        // Send the exploration task
        self.event_store
            .push_event(
                &event.stream_id,
                &task_thread_id,
                &user_event,
                &Default::default(),
            )
            .await?;

        Ok(())
    }

    async fn handle_work_completion(
        &mut self,
        event: &EventDb<Event>,
        summary: &str,
    ) -> eyre::Result<()> {
        // Load task thread to get parent info from LLMConfig
        let task_query = Query::stream(&event.stream_id).aggregate(&event.aggregate_id);
        let task_events = self.event_store.load_events::<Event>(&task_query, None).await?;

        // Find the LLMConfig event to get parent info
        let parent_info = task_events.iter()
            .find_map(|e| match e {
                Event::LLMConfig { parent, .. } => parent.as_ref(),
                _ => None,
            });

        if let Some(parent) = parent_info {
            // Find matching handler based on thread prefix
            let handler = self.handlers.iter()
                .find(|h| event.aggregate_id.starts_with(h.thread_prefix()));

            if let Some(handler) = handler {
                let result_content = handler.format_result(summary);

                let user_content = rig::OneOrMany::one(rig::message::UserContent::Text(
                    rig::message::Text {
                        text: result_content,
                    }
                ));

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
}