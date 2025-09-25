use dabgent_agent::processor::{CompactProcessor, Pipeline, Processor, ThreadProcessor, ToolProcessor};
use dabgent_agent::toolbox::{databricks::databricks_toolset, ToolDyn};
use dabgent_mq::{EventStore, create_store, StoreConfig};
use dabgent_sandbox::Sandbox;
use eyre::Result;
use rig::client::ProviderClient;

const SYSTEM_PROMPT: &str = r#"
You are a data analyst working with Databricks Unity Catalog.

Your approach should be systematic:
1. Start by exploring available tables to understand the data landscape
2. Investigate promising tables by examining their metadata, schemas, and sample data
3. Use SQL queries when needed to better understand the data contents
4. Provide clear summaries of your findings

Be thorough in your analysis and provide concrete examples when describing data.
"#;

const MODEL: &str = "claude-sonnet-4-20250514";

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    const STREAM_ID: &str = "databricks_exploration";
    const AGGREGATE_ID: &str = "explorer";

    let llm = rig::providers::anthropic::Client::from_env();
    let store = create_store(Some(StoreConfig::from_env())).await?;

    // Load Databricks tools
    let tools = match databricks_toolset() {
        Ok(tools) => {
            println!("✅ Databricks tools loaded successfully");
            tools
        }
        Err(e) => {
            eprintln!("❌ Failed to load Databricks tools: {}", e);
            return Err(e);
        }
    };

    // Push LLM config
    push_llm_config(&store, STREAM_ID, AGGREGATE_ID, &tools).await?;

    // Push initial prompt
    push_prompt(&store, STREAM_ID, AGGREGATE_ID, USER_PROMPT).await?;

    println!("🚀 Starting Databricks bakery sales data exploration...");

    // Set up processors
    let thread_processor = ThreadProcessor::new(llm, store.clone());
    let tool_processor = ToolProcessor::new(
        DummySandbox::new().boxed(),
        store.clone(),
        tools,
        None,
    );
    let compact_processor = CompactProcessor::new(
        store.clone(),
        2048, // Compact threshold - keep context manageable
        MODEL.to_string(),
    );

    let pipeline = Pipeline::new(
        store,
        vec![
            thread_processor.boxed(),
            tool_processor.boxed(),
            compact_processor.boxed(),
        ],
    );

    pipeline.run(STREAM_ID.to_owned()).await?;
    println!("✅ Exploration completed!");

    Ok(())
}

const USER_PROMPT: &str = r#"
I need to find bakery sales data in our Databricks environment.

Please help me locate tables that contain bakery or food sales information. I'm looking for data that might include:
- Bakery product sales transactions
- Food retail data
- Sales records with bakery items
- Any tables with relevant product categories or sales metrics

Can you explore the Unity Catalog and tell me what bakery-related sales data is available?
"#;

/// Dummy sandbox for Databricks tools that don't need actual file operations
struct DummySandbox;

impl DummySandbox {
    fn new() -> Self {
        Self
    }
}

impl Sandbox for DummySandbox {
    async fn exec(&mut self, _command: &str) -> Result<dabgent_sandbox::ExecResult> {
        Ok(dabgent_sandbox::ExecResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    async fn write_file(&mut self, _path: &str, _content: &str) -> Result<()> {
        Ok(())
    }

    async fn write_files(&mut self, _files: Vec<(&str, &str)>) -> Result<()> {
        Ok(())
    }

    async fn read_file(&self, _path: &str) -> Result<String> {
        Ok(String::new())
    }

    async fn delete_file(&mut self, _path: &str) -> Result<()> {
        Ok(())
    }

    async fn list_directory(&self, _path: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn set_workdir(&mut self, _path: &str) -> Result<()> {
        Ok(())
    }

    async fn export_directory(&self, _container_path: &str, _host_path: &str) -> Result<String> {
        Ok(String::new())
    }

    async fn fork(&self) -> Result<DummySandbox> {
        Ok(DummySandbox)
    }
}

async fn push_llm_config<S: EventStore>(
    store: &S,
    stream_id: &str,
    aggregate_id: &str,
    tools: &[Box<dyn ToolDyn>],
) -> Result<()> {
    let tool_definitions: Vec<rig::completion::ToolDefinition> = tools
        .iter()
        .map(|tool| tool.definition())
        .collect();

    let event = dabgent_agent::event::Event::LLMConfig {
        model: MODEL.to_owned(),
        temperature: 0.1,
        max_tokens: 8192,
        preamble: Some(SYSTEM_PROMPT.to_owned()),
        tools: Some(tool_definitions),
        recipient: None,
        parent: None,
    };
    store
        .push_event(stream_id, aggregate_id, &event, &Default::default())
        .await
        .map_err(Into::into)
}

async fn push_prompt<S: EventStore>(
    store: &S,
    stream_id: &str,
    aggregate_id: &str,
    prompt: &str,
) -> Result<()> {
    let content = rig::message::UserContent::Text(rig::message::Text { text: prompt.to_owned() });
    let event = dabgent_agent::event::Event::UserMessage(rig::OneOrMany::one(content));
    store
        .push_event(stream_id, aggregate_id, &event, &Default::default())
        .await
        .map_err(Into::into)
}