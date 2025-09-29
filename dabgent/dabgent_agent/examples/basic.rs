use dabgent_agent::event::Event;
use dabgent_agent::processor::{Pipeline, Processor, ThreadProcessor, ToolProcessor};
use dabgent_agent::toolbox::{self, basic::toolset};
use dabgent_mq::{EventStore, db::{Query, sqlite::SqliteStore}};
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::{Sandbox, SandboxDyn};
use eyre::Result;
use rig::client::ProviderClient;

const MODEL: &str = "claude-sonnet-4-20250514";
const AGGREGATE_ID: &str = "thread";
const TEMPERATURE: f64 = 0.0;
const MAX_TOKENS: u64 = 4_096;

const SYSTEM_PROMPT: &str = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.
";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    const STREAM_ID: &str = "pipeline";
    let prompt = "minimal script that fetches my ip using some api like ipify.org";

    let store = store().await;
    let tool_definitions = {
        let tools = toolset(Validator);
        tools
            .iter()
            .map(|tool| tool.definition())
            .collect::<Vec<_>>()
    };
    ensure_thread_config(&store, STREAM_ID, AGGREGATE_ID, &tool_definitions)
        .await
        .unwrap();
    push_prompt(&store, STREAM_ID, AGGREGATE_ID, prompt)
        .await
        .unwrap();
    pipeline_fn(STREAM_ID, AGGREGATE_ID, store)
        .await
        .unwrap();
}

pub async fn pipeline_fn(
    stream_id: &str,
    aggregate_id: &str,
    store: impl EventStore,
) -> Result<()> {
    let stream_id = stream_id.to_owned();
    let aggregate_id = aggregate_id.to_owned();
    ConnectOpts::default()
        .connect(move |client| {
            let stream_id = stream_id.clone();
            let aggregate_id = aggregate_id.clone();
            let store = store.clone();
            let llm = rig::providers::anthropic::Client::from_env();

            async move {
                let sandbox = sandbox(&client).await?;
                let tools = toolset(Validator);
                let tool_definitions = tools
                    .iter()
                    .map(|tool| tool.definition())
                    .collect::<Vec<_>>();
                ensure_thread_config(&store, &stream_id, &aggregate_id, &tool_definitions)
                    .await?;
                let thread_processor = ThreadProcessor::new(llm.clone(), store.clone());
                let tool_processor =
                    ToolProcessor::new(sandbox.boxed(), store.clone(), tools, None);
                let pipeline = Pipeline::new(
                    store.clone(),
                    vec![thread_processor.boxed(), tool_processor.boxed()],
                );
                pipeline.run(stream_id.clone()).await?;
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
) -> Result<()> {
    let user_content = rig::message::UserContent::Text(rig::message::Text {
        text: prompt.to_owned(),
    });
    let event = Event::UserMessage(rig::OneOrMany::one(user_content));
    store
        .push_event(stream_id, aggregate_id, &event, &Default::default())
        .await
        .map_err(Into::into)
}

async fn ensure_thread_config<S: EventStore>(
    store: &S,
    stream_id: &str,
    aggregate_id: &str,
    tools: &[rig::completion::ToolDefinition],
) -> Result<()> {
    let query = Query::stream(stream_id).aggregate(aggregate_id);
    let events = store.load_events::<Event>(&query, None).await?;
    if events
        .iter()
        .any(|event| matches!(event, Event::LLMConfig { .. }))
    {
        return Ok(());
    }

    let event = Event::LLMConfig {
        model: MODEL.to_owned(),
        temperature: TEMPERATURE,
        max_tokens: MAX_TOKENS,
        preamble: Some(SYSTEM_PROMPT.to_owned()),
        tools: if tools.is_empty() {
            None
        } else {
            Some(tools.to_vec())
        },
        recipient: None,
        parent: None,
    };
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
