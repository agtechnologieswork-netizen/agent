use super::DelegationHandler;
use crate::event::{Event, ParentAggregate};
use crate::toolbox::databricks::databricks_toolset;
use eyre::Result;
use uuid::Uuid;

pub const DATABRICKS_SYSTEM_PROMPT: &str = r#"
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

pub const TRIGGER_TOOL: &str = "explore_databricks_catalog";
pub const THREAD_PREFIX: &str = "databricks_";
pub const WORKER_NAME: &str = "databricks_worker";

pub fn create_databricks_delegation(
    catalog: &str,
    prompt_arg: &str,
    model: &str,
    parent_aggregate_id: &str,
    parent_tool_id: &str,
) -> Result<(String, Event, Event)> {
    let task_thread_id = format!("databricks_{}", Uuid::new_v4());
    let prompt = format!("Explore catalog '{}': {}", catalog, prompt_arg);

    // Get Databricks tools
    let tools = databricks_toolset().map_err(|e| eyre::eyre!("Failed to get databricks tools: {}", e))?;

    let tool_definitions: Vec<rig::completion::ToolDefinition> = tools
        .iter()
        .map(|tool| tool.definition())
        .collect();

    let config_event = Event::LLMConfig {
        model: model.to_string(),
        temperature: 0.0,
        max_tokens: 16384,
        preamble: Some(DATABRICKS_SYSTEM_PROMPT.to_string()),
        tools: Some(tool_definitions),
        recipient: Some(WORKER_NAME.to_string()),
        parent: Some(ParentAggregate {
            aggregate_id: parent_aggregate_id.to_string(),
            tool_id: Some(parent_tool_id.to_string()),
        }),
    };

    let user_event = Event::UserMessage(rig::OneOrMany::one(
        rig::message::UserContent::Text(rig::message::Text {
            text: prompt,
        }),
    ));

    Ok((task_thread_id, config_event, user_event))
}

pub struct DatabricksHandler;

impl DatabricksHandler {
    pub fn new() -> Self {
        Self
    }
}

impl DelegationHandler for DatabricksHandler {
    fn trigger_tool(&self) -> &str {
        TRIGGER_TOOL
    }

    fn thread_prefix(&self) -> &str {
        THREAD_PREFIX
    }

    fn handle(
        &self,
        catalog: &str,
        prompt_arg: &str,
        model: &str,
        parent_aggregate_id: &str,
        parent_tool_id: &str
    ) -> Result<(String, Event, Event)> {
        create_databricks_delegation(catalog, prompt_arg, model, parent_aggregate_id, parent_tool_id)
    }

    fn format_result(&self, summary: &str) -> String {
        format!(
            "## Databricks Exploration Results\n\n{}\n\n*This data was discovered from your Databricks catalog and can be used to build your DataApp API.*",
            summary
        )
    }
}

