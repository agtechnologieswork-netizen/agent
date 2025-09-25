use dabgent_agent::pipeline::PipelineBuilder;
use dabgent_agent::processor::{Pipeline, Processor, ThreadProcessor, ToolProcessor};
use dabgent_agent::toolbox::{self, basic::toolset, planning::planning_toolset};
use dabgent_mq::{EventStore, Query};
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::{Sandbox, SandboxDyn};
use eyre::Result;
use rig::client::ProviderClient;

pub const SYSTEM_PROMPT: &str = "
You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.
";

const PLANNING_PROMPT: &str = "
You are a planning assistant that breaks down complex tasks into actionable steps.

Create a clear, actionable plan that an engineer can follow.

When creating a plan:
1. Break down the task into clear, specific steps
2. Each step should be a concrete action
3. Order the steps logically
4. Use the create_plan tool to submit your plan

The create_plan tool expects an array of task descriptions.
Each task should be a concrete, actionable step that can be independently executed.
";

const MODEL: &str = "claude-sonnet-4-20250514";
const RECIPIENT: &str = "sandbox";
const DEFAULT_TEMPERATURE: f64 = 0.0;
const DEFAULT_MAX_TOKENS: u64 = 4_096;

pub struct PlanningAgent<S: EventStore> {
    store: S,
    stream_id: String,
    llm: rig::providers::anthropic::Client,
}

impl<S: EventStore + Clone + Send + Sync + 'static> PlanningAgent<S> {
    pub fn new(store: S, stream_id: String) -> Self {
        Self {
            store,
            stream_id,
            llm: rig::providers::anthropic::Client::from_env(),
        }
    }

    pub async fn create_plan(&self, task: &str) -> Result<Vec<String>> {
        let planning_config = dabgent_agent::event::Event::LLMConfig {
            model: MODEL.to_string(),
            temperature: 0.7,
            max_tokens: DEFAULT_MAX_TOKENS,
            preamble: Some(PLANNING_PROMPT.to_string()),
            tools: Some(
                planning_toolset(self.store.clone(), self.stream_id.clone())
                    .iter()
                    .map(|tool| tool.definition())
                    .collect()
            ),
            recipient: Some("planner".to_string()),
            parent: None,
        };
        self.store
            .push_event(&self.stream_id, "planner", &planning_config, &Default::default())
            .await?;

        // Send the task to plan
        let user_message = dabgent_agent::event::Event::UserMessage(
            rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
                text: format!("Please create a plan for the following task: {}", task),
            }))
        );
        self.store
            .push_event(&self.stream_id, "planner", &user_message, &Default::default())
            .await?;

        // Create planning pipeline with dummy sandbox
        let planning_sandbox = DummySandbox::new();
        let planning_tools = planning_toolset(self.store.clone(), self.stream_id.clone());

        let planning_thread = ThreadProcessor::new(self.llm.clone(), self.store.clone());
        let planning_tool_processor = ToolProcessor::new(
            Box::new(planning_sandbox) as Box<dyn SandboxDyn>,
            self.store.clone(),
            planning_tools,
            Some("planner".to_string()),
        );

        let planning_pipeline = Pipeline::new(
            self.store.clone(),
            vec![planning_thread.boxed(), planning_tool_processor.boxed()],
        );

        // Run planning pipeline
        let pipeline_handle = tokio::spawn({
            let stream_id = self.stream_id.clone();
            async move {
                planning_pipeline.run(stream_id).await
            }
        });

        // Wait for plan creation
        let mut attempts = 0;
        let max_attempts = 60; // 30 seconds timeout
        let mut plan_tasks = Vec::new();

        while attempts < max_attempts {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            attempts += 1;

            let query = Query::stream(&self.stream_id).aggregate("planner");
            let events = self.store.load_events::<dabgent_agent::event::Event>(&query, None).await?;

            for event in events.iter() {
                if let dabgent_agent::event::Event::PlanCreated { tasks } = event {
                    plan_tasks = tasks.clone();
                    break;
                }
            }

            if !plan_tasks.is_empty() {
                break;
            }
        }

        // Stop the planning pipeline
        pipeline_handle.abort();

        Ok(plan_tasks)
    }

    /// Execute a plan with worker LLMs
    pub async fn execute_plan(
        &self,
        tasks: Vec<String>,
        sandbox: Box<dyn SandboxDyn>,
    ) -> Result<()> {
        let execution_tools = toolset(Validator);

        let execution_thread = ThreadProcessor::new(self.llm.clone(), self.store.clone());
        let execution_tool_processor = ToolProcessor::new(
            sandbox,
            self.store.clone(),
            execution_tools,
            None,
        );

        let execution_pipeline = Pipeline::new(
            self.store.clone(),
            vec![execution_thread.boxed(), execution_tool_processor.boxed()],
        );

        let exec_handle = tokio::spawn({
            let stream_id = self.stream_id.clone();
            async move {
                execution_pipeline.run(stream_id).await
            }
        });

        // Execute tasks sequentially
        for (i, task_desc) in tasks.iter().enumerate() {
            let thread_id = format!("task-{}", i);

            // Configure worker LLM for this task
            let worker_config = dabgent_agent::event::Event::LLMConfig {
                model: MODEL.to_string(),
                temperature: DEFAULT_TEMPERATURE,
                max_tokens: DEFAULT_MAX_TOKENS,
                preamble: Some(SYSTEM_PROMPT.to_string()),
                tools: Some(
                    toolset(Validator)
                        .iter()
                        .map(|tool| tool.definition())
                        .collect()
                ),
                recipient: None,
                parent: None,
            };
            self.store
                .push_event(&self.stream_id, &thread_id, &worker_config, &Default::default())
                .await?;

            // Send task to worker
            let task_message = dabgent_agent::event::Event::UserMessage(
                rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
                    text: format!("{}
When complete, call the 'done' tool to mark this task as finished.", task_desc),
                }))
            );
            self.store
                .push_event(&self.stream_id, &thread_id, &task_message, &Default::default())
                .await?;

            // Wait for task completion
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                let query = Query::stream(&self.stream_id);
                let events = self.store.load_events::<dabgent_agent::event::Event>(&query, None).await?;

                let completed_count = events.iter()
                    .filter(|e| matches!(e, dabgent_agent::event::Event::TaskCompleted { .. }))
                    .count();

                if completed_count > i {
                    break;
                }
            }
        }

        exec_handle.abort();
        Ok(())
    }
}

/// Dummy sandbox for planning tools that don't need actual execution
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

pub async fn run_pipeline(store: impl EventStore, stream_id: String) {
    const AGGREGATE_ID: &str = "thread";

    let opts = ConnectOpts::default();
    opts.connect(|client| async move {
        let llm = rig::providers::anthropic::Client::from_env();
        let sandbox = sandbox(&client).await?;
        let tools = toolset(Validator);
        let pipeline = PipelineBuilder::new()
            .llm(llm)
            .store(store)
            .sandbox(sandbox.boxed())
            .model(MODEL.to_owned())
            .temperature(DEFAULT_TEMPERATURE)
            .max_tokens(DEFAULT_MAX_TOKENS)
            .preamble(SYSTEM_PROMPT.to_owned())
            .recipient(RECIPIENT.to_owned())
            .tools(tools)
            .build()?;

        pipeline
            .run(stream_id.to_owned(), AGGREGATE_ID.to_owned())
            .await
    })
    .await
    .unwrap();
}

pub async fn run_planning_pipeline(
    store: impl EventStore + Clone + Send + Sync + 'static,
    stream_id: String,
    task: String,
) {
    let opts = ConnectOpts::default();
    opts.connect(|client| async move {
        // Create planning agent
        let planning_agent = PlanningAgent::new(store, stream_id);

        // Create plan
        let tasks = planning_agent.create_plan(&task).await?;

        if tasks.is_empty() {
            println!("No tasks created in plan");
            return Ok(());
        }

        println!("Created plan with {} tasks", tasks.len());
        for (i, task) in tasks.iter().enumerate() {
            println!("  {}. {}", i + 1, task);
        }

        // Get sandbox for execution
        let sandbox = sandbox(&client).await?;

        // Execute the plan
        planning_agent.execute_plan(tasks, sandbox.boxed()).await?;

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
    let sandbox = DaggerSandbox::from_container(ctr, client.clone());
    Ok(sandbox)
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
