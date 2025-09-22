use dabgent_agent::processor::{Pipeline, Processor, ThreadProcessor, ToolProcessor};
use dabgent_agent::toolbox::{self, basic::toolset};
use dabgent_mq::{EventStore, db::sqlite::SqliteStore};
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::{Sandbox, SandboxDyn};
use eyre::Result;
use rig::client::ProviderClient;

const MODEL: &str = "claude-sonnet-4-20250514";

const SYSTEM_PROMPT: &str = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.
";

const PLANNING_PROMPT: &str = "
You are an engineering manager.
Engineer should follow the plan and implement the solution.
You should provide the plan to the engineer.

Sample plan:
- Create a new file called main.py
- Write a simple hello world program
- Run the program using uv run main.py
";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    const STREAM_ID: &str = "planning-pipeline";
    let prompt = "Weather app that uses the weather api to get the weather for a given city";

    let store = store().await;
    push_prompt(&store, STREAM_ID, "", prompt).await.unwrap();
    pipeline_fn(STREAM_ID, store).await.unwrap();
}

pub async fn pipeline_fn(stream_id: &str, store: impl EventStore) -> Result<()> {
    let stream_id = stream_id.to_owned();
    let opts = ConnectOpts::default();
    opts.connect(|client| async move {
        let llm = rig::providers::anthropic::Client::from_env();
        let sandbox = sandbox(&client).await?;
        let tools = toolset(Validator);
        let planning_tools = toolset();

        let thread_processor = ThreadProcessor::new(
            llm.clone(),
            store.clone(),
            MODEL.to_owned(),
            SYSTEM_PROMPT.to_owned(),
            tools.iter().map(|tool| tool.definition()).collect(),
        );
        let planning_processor = PlanningProcessor::new(
            llm.clone(),
            store.clone(),
            MODEL.to_owned(),
            PLANNING_PROMPT.to_owned(),
            planning_tools.iter().map(|tool| tool.definition()).collect(),
        );
        let tool_processor = ToolProcessor::new(sandbox.boxed(), store.clone(), tools);
        let planning_tool_processor = ToolProcessor::new(sandbox.boxed(), store.clone(), planning_tools);
        let pipeline = Pipeline::new(
            store.clone(),
            vec![thread_processor.boxed(), tool_processor.boxed(), planning_processor.boxed(), planning_tool_processor.boxed()],
        );
        pipeline.run(stream_id.clone()).await?;
        Ok(())
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
    let event = dabgent_agent::event::Event::Prompted(prompt.to_owned());
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

