use dabgent_agent::agent::{self};
use dabgent_agent::handler::Handler;
use dabgent_agent::thread::{self};
use dabgent_agent::toolbox::{self, basic::toolset};
use dabgent_mq::EventStore;
use dabgent_mq::db::{Query, sqlite::SqliteStore};
use dabgent_sandbox::dagger::Sandbox as DaggerSandbox;
use dabgent_sandbox::{Sandbox, SandboxDyn};
use eyre::{eyre, Result};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    run().await;
}

async fn run() {
    dagger_sdk::connect(|client| async move {
        dotenvy::dotenv().ok();
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .expect("ANTHROPIC_API_KEY must be set in environment or .env file");
        let llm = rig::providers::anthropic::Client::new(api_key.as_str());
        let sandbox = sandbox(&client).await?;
        let store = store().await;

        let tools = toolset(Validator);
        let planning_worker = agent::Worker::new(llm, store.clone(), SYSTEM_PROMPT.to_owned(), tools);

        let tools = toolset(Validator);
        let mut sandbox_worker = agent::ToolWorker::new(sandbox.boxed(), store.clone(), tools);

        tokio::spawn(async move {
            let _ = planning_worker.run("planning", "thread").await;
        });
        tokio::spawn(async move {
            let _ = sandbox_worker.run("planning", "thread").await;
        });

        let event = thread::Event::Prompted(
            "Implement a service that takes CSV file as input and produces Hypermedia API as output. Make sure to run it in such a way it does not block the agent while running (it will be run by uv run main.py command)".to_owned(),
        );
        store
            .push_event("planning", "thread", &event, &Default::default())
            .await?;

        let query = Query {
            stream_id: "planning".to_owned(),
            event_type: None,
            aggregate_id: Some("thread".to_owned()),
        };

        let mut receiver = store.subscribe::<thread::Event>(&query)?;
        let mut events = store.load_events(&query, None).await?;
        while let Some(event) = receiver.next().await {
            let event = event?;
            events.push(event.clone());
            let thread = thread::Thread::fold(&events);
            tracing::info!(?thread.state, ?event, "event");
            match thread.state {
                thread::State::Done => break,
                _ => continue,
            }
        }

        Ok(())
    })
    .await
    .unwrap();
}

async fn sandbox(client: &dagger_sdk::DaggerConn) -> Result<DaggerSandbox> {
    let opts = dagger_sdk::ContainerBuildOptsBuilder::default()
        .dockerfile("Dockerfile")
        .build()?;
    let ctr = client
        .container()
        .build_opts(client.host().directory("./examples"), opts);
    ctr.sync().await?;
    let sandbox = DaggerSandbox::from_container(ctr);
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

const SYSTEM_PROMPT: &str = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.
You are also a planning expert who breaks down complex tasks to planning.md file and updates them there after each step.
";

pub struct Validator;

impl toolbox::Validator for Validator {
    async fn run(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {
        let timeout = std::time::Duration::from_secs(30);
        tokio::time::timeout(timeout, sandbox.exec("uv run main.py"))
            .await
            .map_err(|_| eyre!("Validation timed out after 30 seconds"))?
            .map(|result| {
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
