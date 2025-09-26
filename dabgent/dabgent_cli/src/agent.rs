use dabgent_agent::llm::LLMClient;
use dabgent_agent::pipeline::PipelineBuilder;
use dabgent_agent::processor::{Pipeline, Processor, ThreadProcessor, ToolProcessor};
use dabgent_agent::toolbox::basic::toolset;
use dabgent_agent::toolbox::planning::planning_toolset;
use dabgent_agent::utils::PythonValidator;
use dabgent_mq::{EventStore, Query};
use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::{NoOpSandbox, Sandbox};
use eyre::Result;
use rig::client::ProviderClient;

const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
const PYTHON_SYSTEM_PROMPT: &str = "You are a python software engineer executing a single specific task.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.

CRITICAL: When you complete the assigned task, you MUST call the 'done' tool immediately.
Do not continue with additional tasks - only complete the one specific task given to you.";

async fn create_dagger_sandbox(
    client: &dagger_sdk::DaggerConn,
    examples_path: &str,
) -> Result<DaggerSandbox> {
    tracing::info!("Creating Dagger sandbox from {}/Dockerfile", examples_path);
    let opts = dagger_sdk::ContainerBuildOptsBuilder::default()
        .dockerfile("Dockerfile")
        .build()?;
    let ctr = client
        .container()
        .build_opts(client.host().directory(examples_path), opts);
    tracing::info!("Syncing container...");
    ctr.sync().await?;
    let sandbox = DaggerSandbox::from_container(ctr, client.clone());
    tracing::info!("Dagger sandbox created successfully");
    Ok(sandbox)
}

pub async fn run_pipeline(store: impl EventStore, stream_id: String, planning: bool) {
    let opts = ConnectOpts::default();

    opts.connect(move |client| async move {
        let llm = rig::providers::anthropic::Client::from_env();

        if planning {
            run_planning_mode(store, stream_id, llm, &client).await
        } else {
            run_standard_mode(store, stream_id, llm, &client).await
        }
    })
    .await
    .unwrap();
}

const PLANNING_PROMPT: &str = "
You are a planning assistant that breaks down complex tasks into actionable steps.

Create a clear, actionable plan that an engineer can follow.

When creating a plan:
1. Break down the task into clear, specific steps
2. Each step should be a concrete action
3. Order the steps logically
4. Use the create_plan tool to submit your plan

IMPORTANT: The create_plan tool will automatically send a UserInputRequested event for user feedback.
Do NOT send any other messages asking for user input. Only use tools to manage the plan.

When you receive task status updates:
- Use get_plan_status to check overall progress
- Use update_plan if tasks need to be modified
- The system will handle all task execution automatically

When all tasks are completed (you'll receive a message listing all completed tasks):
- Provide a comprehensive summary of what was accomplished
- Highlight any important results or outputs
- Suggest any follow-up actions if needed

The create_plan tool expects an array of task descriptions.
Each task should be a concrete, actionable step that can be independently executed.
";

async fn run_standard_mode<C: LLMClient + 'static>(
    store: impl EventStore,
    stream_id: String,
    llm: C,
    client: &dagger_sdk::DaggerConn,
) -> Result<()> {
    let sandbox = create_dagger_sandbox(&client, "./examples").await?;
    let tools = toolset(PythonValidator);

    let pipeline = PipelineBuilder::new()
        .llm(llm)
        .store(store)
        .sandbox(sandbox.boxed())
        .model(DEFAULT_MODEL.to_string())
        .temperature(0.0)
        .max_tokens(4096)
        .preamble(PYTHON_SYSTEM_PROMPT.to_string())
        .recipient("sandbox".to_string())
        .tools(tools)
        .build()?;

    pipeline
        .run(stream_id.to_owned(), "thread".to_owned())
        .await
}

async fn run_planning_mode<C: LLMClient + 'static>(
    store: impl EventStore + Clone,
    stream_id: String,
    llm: C,
    client: &dagger_sdk::DaggerConn,
) -> Result<()> {
    // Set up the planner configuration first
    // This needs to be done before the user sends messages
    let planning_config = dabgent_agent::event::Event::LLMConfig {
        model: DEFAULT_MODEL.to_string(),
        temperature: 0.7,
        max_tokens: 4096,
        preamble: Some(PLANNING_PROMPT.to_string()),
        tools: Some(
            planning_toolset(store.clone(), stream_id.clone())
                .iter()
                .map(|tool| tool.definition())
                .collect()
        ),
        recipient: Some("planner".to_string()),
        parent: None,
    };

    store
        .push_event(&stream_id, "planner", &planning_config, &Default::default())
        .await?;

    // Create two separate pipelines: planner and executor
    let planner_pipeline = create_planner_pipeline(store.clone(), stream_id.clone(), llm.clone()).await?;
    let executor_pipeline = create_executor_pipeline(store.clone(), stream_id.clone(), llm.clone(), client).await?;

    // Run both pipelines concurrently
    let planner_stream_id = stream_id.clone();
    let executor_stream_id = stream_id.clone();

    let planner_handle = tokio::spawn(async move {
        planner_pipeline.run(planner_stream_id).await
    });
    let executor_handle = tokio::spawn(async move {
        executor_pipeline.run(executor_stream_id).await
    });

    // Wait for both pipelines
    tokio::select! {
        result = planner_handle => {
            match result {
                Ok(Ok(_)) => tracing::info!("Planner pipeline completed"),
                Ok(Err(e)) => tracing::error!("Planner pipeline error: {:?}", e),
                Err(e) => tracing::error!("Planner pipeline panic: {:?}", e),
            }
        }
        result = executor_handle => {
            match result {
                Ok(Ok(_)) => tracing::info!("Executor pipeline completed"),
                Ok(Err(e)) => tracing::error!("Executor pipeline error: {:?}", e),
                Err(e) => tracing::error!("Executor pipeline panic: {:?}", e),
            }
        }
    }

    Ok(())
}

async fn create_planner_pipeline<C: LLMClient + 'static>(
    store: impl EventStore + Clone,
    stream_id: String,
    llm: C,
) -> Result<Pipeline<impl EventStore, dabgent_agent::event::Event>> {
    // Configuration is already set up in run_planning_mode
    let planning_sandbox = NoOpSandbox::new();
    let planning_tools = planning_toolset(store.clone(), stream_id.clone());

    let planning_thread = ThreadProcessor::new(llm, store.clone())
        .with_recipient_filter("planner".to_string());
    let planning_tool_processor = ToolProcessor::new(
        planning_sandbox.boxed(),
        store.clone(),
        planning_tools,
        Some("planner".to_string()),
    );

    Ok(Pipeline::new(
        store.clone(),
        vec![planning_thread.boxed(), planning_tool_processor.boxed()],
    ))
}

async fn create_executor_pipeline<C: LLMClient + 'static>(
    store: impl EventStore + Clone,
    stream_id: String,
    llm: C,
    client: &dagger_sdk::DaggerConn,
) -> Result<Pipeline<impl EventStore, dabgent_agent::event::Event>> {
    let execution_sandbox = create_dagger_sandbox(&client, "./examples").await?;
    let execution_tools = toolset(PythonValidator);

    let execution_thread = ThreadProcessor::new(llm, store.clone())
        .with_recipient_filter("task-*".to_string());
    // ToolProcessor doesn't support wildcard recipients, so we use None
    // but this means it will process ALL tool calls, not just task-* ones
    let execution_tool_processor = ToolProcessor::new(
        execution_sandbox.boxed(),
        store.clone(),
        execution_tools,
        None,  // TODO: Need to implement wildcard support in ToolProcessor
    );

    // Set up a task monitor that will:
    // 1. Watch for PlanCreated/PlanUpdated events from the planner
    // 2. Create execution tasks for each planned task
    // 3. Monitor TaskCompleted events and inform the planner
    let monitor_store = store.clone();
    let monitor_stream_id = stream_id.clone();
    tokio::spawn(async move {
        tracing::info!("Monitor task started");
        if let Err(e) = monitor_plan_execution(monitor_store, monitor_stream_id).await {
            tracing::error!("Monitor task failed: {:?}", e);
        }
    });

    Ok(Pipeline::new(
        store.clone(),
        vec![execution_thread.boxed(), execution_tool_processor.boxed()],
    ))
}

async fn monitor_plan_execution(
    store: impl EventStore,
    stream_id: String,
) -> Result<()> {
    tracing::info!("Monitor plan execution starting for stream {}", stream_id);
    let mut last_plan_tasks: Vec<String> = Vec::new();
    let mut current_task_index: Option<usize> = None;
    let mut tasks_started: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
    let mut plan_acknowledged = false;
    let mut waiting_for_user_input = false;

    loop {
        interval.tick().await;

        // Load planner events to check for new plans
        let query = Query::stream(&stream_id).aggregate("planner");
        let events = store.load_events::<dabgent_agent::event::Event>(&query, None).await?;

        if !events.is_empty() {
            tracing::debug!("Monitor loaded {} events from planner aggregate", events.len());
        }

        // Check for UserInputRequested events
        let has_user_input_request = events.iter().rev().any(|e|
            matches!(e, dabgent_agent::event::Event::UserInputRequested { .. })
        );

        // Check if user has responded (UserMessage after UserInputRequested)
        if has_user_input_request {
            let user_input_pos = events.iter().rposition(|e|
                matches!(e, dabgent_agent::event::Event::UserInputRequested { .. })
            );

            if let Some(pos) = user_input_pos {
                // Check if there's a UserMessage after the UserInputRequested
                let has_user_response = events.iter().skip(pos + 1).any(|e|
                    matches!(e, dabgent_agent::event::Event::UserMessage(_))
                );

                if !has_user_response {
                    if !waiting_for_user_input {
                        tracing::info!("Planner requested user input, pausing task execution");
                        waiting_for_user_input = true;
                    }
                    continue; // Skip task execution while waiting for user
                } else if waiting_for_user_input {
                    tracing::info!("User responded, resuming task execution");
                    waiting_for_user_input = false;
                }
            }
        }

        // Find the latest plan
        let mut latest_plan: Option<Vec<String>> = None;
        for event in events.iter() {
            match event {
                dabgent_agent::event::Event::PlanCreated { tasks } |
                dabgent_agent::event::Event::PlanUpdated { tasks } => {
                    tracing::info!("Monitor found plan with {} tasks", tasks.len());
                    latest_plan = Some(tasks.clone());
                }
                _ => {}
            }
        }

        if let Some(tasks) = latest_plan {
            // If this is a new plan, reset execution
            if tasks != last_plan_tasks {
                tracing::info!("Monitor detected new plan, resetting execution");
                last_plan_tasks = tasks.clone();
                current_task_index = Some(0);
                tasks_started.clear();
                plan_acknowledged = false;
            }

            // For now, skip the acknowledgment check and proceed directly
            // The planner should handle tool results properly
            if !plan_acknowledged {
                tracing::info!("Plan detected, proceeding to start tasks");
                plan_acknowledged = true;
            }

            // Check if we should start the next task
            if let Some(index) = current_task_index {
                if index < tasks.len() {
                    // Check if current task is completed
                    let task_thread_id = format!("task-{}", index);
                    let task_query = Query::stream(&stream_id).aggregate(&task_thread_id);
                    let task_events = store.load_events::<dabgent_agent::event::Event>(&task_query, None).await?;

                    let is_completed = task_events.iter().any(|e| matches!(e, dabgent_agent::event::Event::TaskCompleted { .. }));
                    let already_started = tasks_started.contains(&index);

                    if !task_events.is_empty() {
                        tracing::debug!("Task {} has {} events, completed: {}, started: {}",
                            index, task_events.len(), is_completed, already_started);
                    }

                    if is_completed && !already_started {
                        // This shouldn't happen - task completed without being started
                        tracing::warn!("Task {} is marked as completed but was never started", index);
                        tasks_started.insert(index); // Mark it as started to avoid confusion
                    }

                    if !already_started && !is_completed {
                        // Start this task (only once)
                        tracing::info!("Monitor starting task {}: {}", index, tasks[index]);
                        tasks_started.insert(index);
                        let task = &tasks[index];
                        let thread_id = format!("task-{}", index);

                        // Configure the execution thread for this task
                        let worker_config = dabgent_agent::event::Event::LLMConfig {
                            model: DEFAULT_MODEL.to_string(),
                            temperature: 0.7,
                            max_tokens: 4096,
                            preamble: Some(PYTHON_SYSTEM_PROMPT.to_string()),
                            tools: Some(
                                toolset(PythonValidator)
                                    .iter()
                                    .map(|tool| tool.definition())
                                    .collect()
                            ),
                            recipient: Some(thread_id.clone()),
                            parent: None,
                        };
                        store.push_event(&stream_id, &thread_id, &worker_config, &Default::default()).await?;
                        tracing::info!("Monitor pushed LLMConfig for task {}", index);

                        let task_message = dabgent_agent::event::Event::UserMessage(
                            rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
                                text: format!("Task {}: {}\\n\\nIMPORTANT: Complete only this specific task, then immediately call the 'done' tool to mark it as finished. Do not continue with other tasks.", index, task),
                            }))
                        );
                        store.push_event(&stream_id, &thread_id, &task_message, &Default::default()).await?;
                        tracing::info!("Monitor pushed UserMessage for task {}: {}", index, task);
                    } else if is_completed {
                        tracing::info!("Task {} is completed, moving to next", index);

                        // Move to next task
                        if index < tasks.len() - 1 {
                            current_task_index = Some(index + 1);
                            tracing::info!("Moving to next task: {}", index + 1);
                        } else {
                            tracing::info!("All {} tasks completed! Notifying planner", tasks.len());

                            // Send a completion summary to the planner
                            let completion_message = dabgent_agent::event::Event::UserMessage(
                                rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
                                    text: format!(
                                        "All {} tasks have been completed successfully:\n\n{}\n\nPlease provide a summary of what was accomplished.",
                                        tasks.len(),
                                        tasks.iter().enumerate()
                                            .map(|(i, t)| format!("✓ Task {}: {}", i, t))
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    ),
                                }))
                            );

                            // Wait a bit to ensure the planner is ready to receive the message
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            store.push_event(&stream_id, "planner", &completion_message, &Default::default()).await?;
                            tracing::info!("Sent completion notification to planner");

                            current_task_index = None; // Reset to wait for new plan
                        }
                    }
                }
            }
        }
    }
}
