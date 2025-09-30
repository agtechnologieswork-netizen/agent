use crate::llm::LLMClientDyn;
use crate::processor::sandbox::{ExecutionCallback, Sandbox, SandboxServices};
use crate::processor::thread::{self, CompletionCallback, Thread};
use crate::processor::utils::LoggerCallback;
use crate::processor::worker::{self, Worker};
use crate::processor::worker_callbacks::{SandboxWatcher, ThreadWatcher, WorkerOrchestrator};
use crate::toolbox::ToolDyn;
use dabgent_mq::listener::PollingQueue;
use dabgent_mq::{EventStore, Handler};
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use rig::completion::ToolDefinition;
use std::sync::Arc;
use tokio::task::JoinSet;
use uuid::Uuid;

pub struct ThreadConfig {
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u64,
    pub preamble: Option<String>,
    pub tools: Option<Vec<ToolDefinition>>,
}

impl Default for ThreadConfig {
    fn default() -> Self {
        Self {
            model: "claude-3-5-sonnet-20241022".to_string(),
            temperature: 1.0,
            max_tokens: 8192,
            preamble: None,
            tools: None,
        }
    }
}

pub fn spawn_listeners<ES: EventStore + 'static>(
    worker_handler: Handler<Worker, ES>,
    thread_handler: Handler<Thread, ES>,
    sandbox_handler: Handler<Sandbox, ES>,
    queue: PollingQueue<ES>,
) -> JoinSet<Result<()>> {
    let mut set = JoinSet::new();

    // Worker orchestrator
    let orchestrator = WorkerOrchestrator::new(thread_handler.clone(), sandbox_handler.clone());
    let mut worker_listener = queue.listener();
    worker_listener.register(orchestrator);
    worker_listener.register(LoggerCallback::new());

    // Thread callbacks
    let mut thread_listener = queue.listener();

    let thread_watcher = ThreadWatcher::new(worker_handler.clone());
    thread_listener.register(thread_watcher);

    let completion_callback = CompletionCallback::new(thread_handler.clone());
    thread_listener.register(completion_callback);

    // Sandbox callbacks
    let mut sandbox_listener = queue.listener();

    let sandbox_watcher = SandboxWatcher::new(worker_handler.clone());
    sandbox_listener.register(sandbox_watcher);

    let execution_callback = ExecutionCallback::new(sandbox_handler.clone());
    sandbox_listener.register(execution_callback);

    set.spawn(async move { thread_listener.run().await });
    set.spawn(async move { worker_listener.run().await });
    set.spawn(async move { sandbox_listener.run().await });

    set
}

pub async fn start_worker<ES: EventStore>(
    worker_handler: &Handler<Worker, ES>,
    thread_handler: &Handler<Thread, ES>,
    config: ThreadConfig,
    message: String,
) -> Result<String> {
    let workflow_id = Uuid::new_v4();
    let worker_id = format!("worker-{}", workflow_id);
    let thread_id = format!("thread-{}", workflow_id);
    let sandbox_id = format!("sandbox-{}", workflow_id);

    // Setup thread
    thread_handler
        .execute(
            &thread_id,
            thread::Command::Setup {
                model: config.model,
                temperature: config.temperature,
                max_tokens: config.max_tokens,
                preamble: config.preamble,
                tools: config.tools,
            },
        )
        .await?;

    // Start worker
    worker_handler
        .execute(
            &worker_id,
            worker::Command::Start {
                message,
                thread_id,
                sandbox_id,
            },
        )
        .await?;

    Ok(worker_id)
}

pub fn create_handlers<ES: EventStore>(
    event_store: ES,
    llm: Arc<dyn LLMClientDyn>,
    sandbox: Box<dyn SandboxDyn>,
    tools: Vec<Box<dyn ToolDyn>>,
) -> (
    Handler<Worker, ES>,
    Handler<Thread, ES>,
    Handler<Sandbox, ES>,
) {
    let worker_handler = Handler::<Worker, ES>::new(event_store.clone(), ());
    let thread_handler = Handler::<Thread, ES>::new(event_store.clone(), llm);
    let sandbox_services = Arc::new(SandboxServices::new(sandbox, tools));
    let sandbox_handler = Handler::<Sandbox, ES>::new(event_store, sandbox_services);

    (worker_handler, thread_handler, sandbox_handler)
}
