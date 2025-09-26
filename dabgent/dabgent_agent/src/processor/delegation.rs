use super::{Aggregate, Processor};
use crate::event::{Event, ParentAggregate};
use crate::processor::thread;
use crate::toolbox::databricks::databricks_toolset;
use dabgent_mq::{EventDb, EventStore, Query};
use uuid::Uuid;

const DATABRICKS_SYSTEM_PROMPT: &str = r#"
You are a Databricks catalog explorer. Your role is to explore Unity Catalog to understand available data structures and provide detailed table schemas.

## Your Task
Explore the specified Databricks catalog and provide a comprehensive summary of:
- Available schemas and their purposes
- Tables within each schema with descriptions
- **DETAILED column structure for each relevant table** including:
  - Column names and data types
  - Sample values from each column
  - Any constraints or key information
- Relationships between tables if apparent

## Focus Areas
When exploring data for DataApp creation:
- Look for tables that contain business-relevant data
- Identify primary keys and foreign key relationships
- **Use `databricks_describe_table` to get full column details and sample data for each relevant table**
- Note columns that would make good API fields

## Output Format
Provide your findings in a structured markdown format with:
1. **Catalog Overview**: Brief description
2. **Schemas Found**: List with purposes
3. **Key Tables**: For each table include:
   - Table name and purpose
   - **Complete column list with data types**
   - **Sample data rows showing actual values**
   - Row counts and other metadata
4. **Recommendations**: Which tables/columns would work well for a DataApp API with specific column mappings

## Completion
When you have completed your exploration and analysis, call the `finish_delegation` tool with a comprehensive summary that includes:
- Brief overview of what you discovered
- Key schemas and table counts
- **Detailed table structures for each relevant table** including:
  - Full column specifications (name: data_type)
  - Sample data showing what the columns contain
- Specific API endpoint recommendations with exact column mappings

Example: `finish_delegation(summary="Explored catalog 'main': Found bakery schema with 3 tables. products table (id: bigint, name: string, price: decimal, category: string) contains 500 bakery items like 'Chocolate Croissant', $4.50, 'pastry'. orders table (order_id: bigint, customer_id: bigint, product_id: bigint, quantity: int, order_date: timestamp) has 10K orders. Recommend /api/products endpoint returning {id, name, price, category} and /api/orders endpoint returning {order_id, customer_id, product_id, quantity, order_date}.")`

**IMPORTANT**: Always use `databricks_describe_table` on relevant tables to get complete column details and sample data. This detailed structure information is critical for API design.
"#;

pub struct DelegationProcessor<E: EventStore> {
    event_store: E,
    default_model: String,
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
    pub fn new(event_store: E, default_model: String) -> Self {
        Self {
            event_store,
            default_model,
        }
    }

    fn is_delegated_thread(&self, aggregate_id: &str) -> bool {
        // Check if this is a delegated thread (starts with known prefixes)
        aggregate_id.starts_with("databricks_")
    }

    fn is_delegation_tool_result(&self, tool_results: &[crate::event::TypedToolResult]) -> bool {
        // Check if any of the tool results are delegation tools
        tool_results.iter().any(|result| {
            match &result.tool_name {
                crate::event::ToolKind::Other(tool_name) => tool_name == "explore_databricks_catalog",
                _ => false,
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
            match &result.tool_name {
                crate::event::ToolKind::Other(tool_name) => tool_name == "explore_databricks_catalog",
                _ => false,
            }
        });

        if let Some(delegation_result) = delegation_result {
            let parent_tool_id = delegation_result.result.id.clone();

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

                let agent_type = "databricks_explorer";
                let prompt = format!("Explore catalog '{}': {}", catalog, prompt_arg);

                self.handle_delegate_work(event, agent_type, &prompt, &parent_tool_id).await?;
            } else {
                tracing::warn!("Could not find original tool call for delegation, using defaults");
                let agent_type = "databricks_explorer";
                let prompt = "Explore bakery business data, focusing on products, sales, customers, and orders.";
                self.handle_delegate_work(event, agent_type, prompt, &parent_tool_id).await?;
            }
        }

        Ok(())
    }

    async fn handle_delegate_work(
        &mut self,
        event: &EventDb<Event>,
        _agent_type: &str,
        prompt: &str,
        parent_tool_id: &str,
    ) -> eyre::Result<()> {
        // Create task thread
        let task_thread_id = format!("databricks_{}", Uuid::new_v4());

        // Get Databricks tools (includes FinishDelegationTool)
        let tools = databricks_toolset().map_err(|e| eyre::eyre!("Failed to get databricks tools: {}", e))?;

        let tool_definitions: Vec<rig::completion::ToolDefinition> = tools
            .iter()
            .map(|tool| tool.definition())
            .collect();

        // Send LLMConfig first with parent tracking
        self.event_store
            .push_event(
                &event.stream_id,
                &task_thread_id,
                &Event::LLMConfig {
                    model: self.default_model.clone(),
                    temperature: 0.0,
                    max_tokens: 16384,
                    preamble: Some(DATABRICKS_SYSTEM_PROMPT.to_string()),
                    tools: Some(tool_definitions),
                    recipient: Some("databricks_worker".to_string()),
                    parent: Some(ParentAggregate {
                        aggregate_id: event.aggregate_id.clone(),
                        tool_id: Some(parent_tool_id.to_string()),
                    }),
                },
                &Default::default(),
            )
            .await?;

        // Send the exploration task
        self.event_store
            .push_event(
                &event.stream_id,
                &task_thread_id,
                &Event::UserMessage(rig::OneOrMany::one(
                    rig::message::UserContent::Text(rig::message::Text {
                        text: prompt.to_string(),
                    }),
                )),
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
            // Use the summary directly from the Done tool
            let result_content = format!(
                "## Databricks Exploration Results\n\n{}\n\n*This data was discovered from your Databricks catalog and can be used to build your DataApp API.*",
                summary
            );

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

        Ok(())
    }
}