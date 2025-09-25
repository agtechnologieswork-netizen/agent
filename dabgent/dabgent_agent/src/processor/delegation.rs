use super::{Aggregate, Processor};
use crate::event::{Event, ParentAggregate};
use crate::processor::thread;
use crate::toolbox::databricks::databricks_toolset;
use dabgent_mq::{EventDb, EventStore, Query};
use uuid::Uuid;

const DATABRICKS_SYSTEM_PROMPT: &str = r#"
You are a Databricks catalog explorer. Your role is to explore Unity Catalog to understand available data structures.

## Your Task
Explore the specified Databricks catalog and provide a comprehensive summary of:
- Available schemas and their purposes
- Tables within each schema with descriptions
- Key columns and data types for relevant tables
- Sample data to understand the data structure
- Relationships between tables if apparent

## Focus Areas
When exploring data for DataApp creation:
- Look for tables that contain business-relevant data
- Identify primary keys and foreign key relationships
- Sample a few rows to understand data patterns
- Note any interesting columns that would make good API fields

## Output Format
Provide your findings in a structured markdown format with:
1. **Catalog Overview**: Brief description
2. **Schemas Found**: List with purposes
3. **Key Tables**: Detailed breakdown with columns and sample data
4. **Recommendations**: Which tables/columns would work well for a DataApp API

## Completion
When you have completed your exploration and analysis, call the `done` tool with a comprehensive summary of your findings. The summary should include:
- A brief overview of what you discovered
- Key schemas and table counts
- Most relevant tables for DataApp APIs
- Specific recommendations with table/column details

Example: `done(summary="Explored catalog 'main': Found 3 relevant schemas for bakery business. Key tables: sales.transactions (50K records, daily sales data), inventory.products (500 items, product catalog with pricing), customers.profiles (10K customers with purchase history). Recommended starting with sales.transactions for revenue analytics API.")`

Be thorough but concise. Focus on data that would be useful for creating REST APIs.
"#;

pub struct DelegationProcessor<E: EventStore> {
    event_store: E,
    default_model: String,
}

impl<E: EventStore> Processor<Event> for DelegationProcessor<E> {
    async fn run(&mut self, event: &EventDb<Event>) -> eyre::Result<()> {
        match &event.data {
            Event::DelegateWork { agent_type, prompt, parent_tool_id } if agent_type == "databricks_explorer" => {
                tracing::info!(
                    "Received delegation request for {} on aggregate {}",
                    agent_type,
                    event.aggregate_id
                );
                self.handle_delegate_work(event, agent_type, prompt, parent_tool_id).await?;
            }
            Event::TaskCompleted { success, summary } if self.is_delegated_thread(&event.aggregate_id) => {
                if *success {
                    tracing::info!(
                        "Delegated work completed successfully for aggregate {}",
                        event.aggregate_id,
                    );
                    self.handle_work_completion(event, summary).await?;
                } else {
                    tracing::warn!(
                        "Delegated work failed for aggregate {}",
                        event.aggregate_id,
                    );
                    // TODO: Handle failed delegation
                }
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

    async fn handle_delegate_work(
        &mut self,
        event: &EventDb<Event>,
        _agent_type: &str,
        prompt: &str,
        parent_tool_id: &str,
    ) -> eyre::Result<()> {
        // Create task thread
        let task_thread_id = format!("databricks_{}", Uuid::new_v4());

        // Get Databricks tools and add Done tool for completion signaling
        let mut tools = databricks_toolset().map_err(|e| eyre::eyre!("Failed to get databricks tools: {}", e))?;

        // Simple validator that always passes for delegation completion
        struct AlwaysPassValidator;
        impl crate::toolbox::Validator for AlwaysPassValidator {
            async fn run(&self, _sandbox: &mut Box<dyn dabgent_sandbox::SandboxDyn>) -> Result<Result<(), String>, eyre::Error> {
                Ok(Ok(()))
            }
        }

        tools.push(Box::new(crate::toolbox::basic::DoneTool::new(AlwaysPassValidator)));

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