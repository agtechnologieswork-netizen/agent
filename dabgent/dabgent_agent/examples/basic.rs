use dabgent_agent::processor::{Pipeline, Processor, ThreadProcessor, ToolProcessor};
use dabgent_agent::toolbox::{self, ToolDyn, basic::toolset};
use dabgent_mq::{EventStore, db::sqlite::SqliteStore};
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::{Sandbox, SandboxDyn};
use eyre::Result;
use rig::client::ProviderClient;
use rig::completion::ToolDefinition;
use rig::message::{Text, UserContent};

const MODEL: &str = "claude-sonnet-4-20250514";

const SYSTEM_PROMPT: &str = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.
";

const TOOL_RECIPIENT: &str = "sandbox-tools";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    const STREAM_ID: &str = "pipeline";
    let prompt = "minimal script that fetches my ip using some api like ipify.org";

    let store = store().await;
    let tools = toolset(Validator);
    let tool_definitions: Vec<ToolDefinition> =
        tools.iter().map(|tool| tool.definition()).collect();
    push_prompt(
        &store,
        STREAM_ID,
        "",
        prompt,
        tool_definitions,
        Some(TOOL_RECIPIENT.to_string()),
    )
    .await?;
    pipeline_fn(STREAM_ID, store, tools, Some(TOOL_RECIPIENT.to_string())).await?;
    Ok(())
}

pub async fn pipeline_fn(
    stream_id: &str,
    store: impl EventStore + Clone,
    tools: Vec<Box<dyn ToolDyn>>,
    recipient: Option<String>,
) -> Result<()> {
    let stream_id = stream_id.to_owned();
    let opts = ConnectOpts::default();
    opts.connect(move |client| {
        let store_clone = store.clone();
        let recipient_clone = recipient.clone();
        let stream_id_clone = stream_id.clone();
        async move {
            let llm = rig::providers::anthropic::Client::from_env();
            let sandbox = sandbox(&client).await?;

            let thread_processor = ThreadProcessor::new(llm.clone(), store_clone.clone());
            let tool_processor =
                ToolProcessor::new(sandbox.boxed(), store_clone.clone(), tools, recipient_clone);
            let pipeline = Pipeline::new(
                store_clone.clone(),
                vec![thread_processor.boxed(), tool_processor.boxed()],
            );
            pipeline.run(stream_id_clone).await?;
            Ok(())
        }
    })
    .await
    .map_err(Into::into)
}

async fn sandbox(client: &dagger_sdk::DaggerConn) -> Result<DaggerSandbox> {
    let opts = dagger_sdk::ContainerBuildOptsBuilder::default()
        .dockerfile("Dockerfile")
        .build()?;
    let ctr = client
        .container()
        .build_opts(client.host().directory("./examples"), opts);
    ctr.sync().await?;
    let sandbox = DaggerSandbox::from_container(ctr, client.clone());
    Ok(sandbox)
}

async fn store() -> SqliteStore {
    let pool = sqlx::SqlitePool::connect(":memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}

async fn push_prompt<S: EventStore>(
    store: &S,
    stream_id: &str,
    aggregate_id: &str,
    prompt: &str,
    tools: Vec<ToolDefinition>,
    recipient: Option<String>,
) -> Result<()> {
    let config = dabgent_agent::event::Event::LLMConfig {
        model: MODEL.to_owned(),
        temperature: 0.7,
        max_tokens: 4096,
        preamble: Some(SYSTEM_PROMPT.to_owned()),
        tools: Some(tools),
        recipient: recipient.clone(),
    };
    store
        .push_event(stream_id, aggregate_id, &config, &Default::default())
        .await?;

    let user_content = UserContent::Text(Text {
        text: prompt.to_owned(),
    });
    let event = dabgent_agent::event::Event::UserMessage(rig::OneOrMany::one(user_content));
    store
        .push_event(stream_id, aggregate_id, &event, &Default::default())
        .await
        .map_err(Into::into)
}

pub struct Validator;

impl toolbox::Validator for Validator {
    async fn run(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {
        sandbox.exec("uv run main.py").await.map(|result| {
            if result.exit_code == 0 {
                Ok(())
            } else {
                Err(format!(
                    "code: {}\nstdout: {}\nstderr: {}",
                    result.exit_code, result.stdout, result.stderr
                ))
            }
        })
    }
}
