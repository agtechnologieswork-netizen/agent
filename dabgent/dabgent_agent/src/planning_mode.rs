use crate::event::Event;
use crate::llm::LLMClient;
use crate::processor::{Pipeline, Processor, ThreadProcessor, ToolProcessor};
use crate::toolbox::planning::planning_toolset;
use crate::toolbox::basic::toolset;
use crate::utils::PythonValidator;
use dabgent_mq::{EventStore, Query};
use dabgent_sandbox::{NoOpSandbox, Sandbox};
use eyre::Result;
use std::collections::HashSet;

pub const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
pub const PYTHON_SYSTEM_PROMPT: &str = "You are a python software engineer.
Workspace is already set up using uv init.
Use uv package manager if you need to add extra libraries.
Program will be run using uv run main.py command.";

pub const PLANNING_PROMPT: &str = "
You are a planning assistant that breaks down complex tasks into actionable steps.

Create a clear, actionable plan that an engineer can follow.

When creating a plan:
1. Break down the task into clear, specific steps
2. Each step should be a concrete action
3. Order the steps logically
4. Use the create_plan tool to submit your plan

The create_plan tool expects an array of task descriptions.
Each task should be a concrete, actionable step that can be independently executed.

IMPORTANT: After all tasks are completed, you will see a UserInputRequested event.
When you see this event, you should respond to the user with a summary and ask what they would like to do next.
Do NOT ignore UserInputRequested events - always respond to them.
";

#[derive(Clone)]
pub struct ModelConfig {
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u64,
    pub preamble: String,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            preamble: PYTHON_SYSTEM_PROMPT.to_string(),
        }
    }
}

/// Create a planner pipeline with standard configuration
pub fn create_planner_pipeline<C: LLMClient + 'static, E: EventStore + Clone + 'static>(
    llm: C,
    store: E,
    stream_id: String,
) -> Pipeline<E, Event> {
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

    Pipeline::new(
        store.clone(),
        vec![planning_thread.boxed(), planning_tool_processor.boxed()],
    )
}

/// Create an executor pipeline with provided sandbox and tools
pub fn create_executor_pipeline<C, E, S>(
    llm: C,
    store: E,
    sandbox: S,
    tools: Vec<Box<dyn crate::toolbox::ToolDyn>>,
) -> Pipeline<E, Event>
where
    C: LLMClient + 'static,
    E: EventStore + Clone + 'static,
    S: Sandbox + Send + Sync + 'static,
{
    let execution_thread = ThreadProcessor::new(llm, store.clone())
        .with_recipient_filter("task-*".to_string());

    // ToolProcessor doesn't support wildcard recipients, so we use None
    // but this means it will process ALL tool calls, not just task-* ones
    let execution_tool_processor = ToolProcessor::new(
        sandbox.boxed(),
        store.clone(),
        tools,
        None,  // TODO: Need to implement wildcard support in ToolProcessor
    );

    Pipeline::new(
        store.clone(),
        vec![execution_thread.boxed(), execution_tool_processor.boxed()],
    )
}

/// Monitor plan execution and coordinate task execution
pub async fn monitor_plan_execution(
    store: impl EventStore,
    stream_id: String,
    model_config: ModelConfig,
) -> Result<()> {
    tracing::info!("Monitor plan execution starting for stream {}", stream_id);
    let mut last_plan_tasks: Vec<String> = Vec::new();
    let mut current_task_index: Option<usize> = None;
    let mut tasks_started: HashSet<usize> = HashSet::new();
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
    let mut plan_acknowledged = false;
    let mut waiting_for_user_input = false;

    loop {
        interval.tick().await;

        // Load planner events to check for new plans
        let query = Query::stream(&stream_id).aggregate("planner");
        let events = store.load_events::<Event>(&query, None).await?;

        if !events.is_empty() {
            tracing::debug!("Monitor loaded {} events from planner aggregate", events.len());
        }

        // Check for UserInputRequested events
        let has_user_input_request = events.iter().rev().any(|e|
            matches!(e, Event::UserInputRequested { .. })
        );

        // Check if user has responded (UserMessage after UserInputRequested)
        if has_user_input_request {
            let user_input_pos = events.iter().rposition(|e|
                matches!(e, Event::UserInputRequested { .. })
            );

            if let Some(pos) = user_input_pos {
                // Check if there's a UserMessage after the UserInputRequested
                let has_user_response = events.iter().skip(pos + 1).any(|e|
                    matches!(e, Event::UserMessage(_))
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
                Event::PlanCreated { tasks } |
                Event::PlanUpdated { tasks } => {
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
                    let task_events = store.load_events::<Event>(&task_query, None).await?;

                    let is_completed = task_events.iter().any(|e| matches!(e, Event::TaskCompleted { .. }));
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

                    // Check if task needs to be started
                    if !already_started && !is_completed {
                        // Start this task (only once)
                        tracing::info!("Monitor starting task {}: {}", index, tasks[index]);
                        tasks_started.insert(index);
                        let task = &tasks[index];
                        let thread_id = format!("task-{}", index);

                        // Configure the execution thread for this task
                        let worker_config = Event::LLMConfig {
                            model: model_config.model.clone(),
                            temperature: model_config.temperature,
                            max_tokens: model_config.max_tokens,
                            preamble: Some(model_config.preamble.clone()),
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

                        let task_message = Event::UserMessage(
                            rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
                                text: format!("Task {}: {}\n\nIMPORTANT: Complete only this specific task, then immediately call the 'done' tool to mark it as finished. Do not continue with other tasks.", index, task),
                            }))
                        );
                        store.push_event(&stream_id, &thread_id, &task_message, &Default::default()).await?;
                        tracing::info!("Monitor pushed UserMessage for task {}: {}", index, task);
                    }

                    // Check if task is completed (regardless of whether it was just started)
                    if is_completed {
                        tracing::info!("Task {} is completed, moving to next", index);

                        // Move to next task
                        if index < tasks.len() - 1 {
                            current_task_index = Some(index + 1);
                            tracing::info!("Moving to next task: {}", index + 1);
                        } else {
                            tracing::info!("All {} tasks completed!", tasks.len());

                            // Create a summary of completed tasks
                            let task_summary = tasks.iter().enumerate()
                                .map(|(i, t)| format!("{}. {}", i + 1, t))
                                .collect::<Vec<_>>()
                                .join("\n");

                            // First, send a completion event to show in the UI
                            let completion_event = Event::PlanCompleted {
                                tasks: tasks.clone(),
                                message: format!("All {} tasks completed successfully", tasks.len()),
                            };
                            store.push_event(&stream_id, "planner", &completion_event, &Default::default()).await?;

                            // Then show prompt to the user for next steps
                            let completion_display = Event::UserInputRequested {
                                prompt: format!(
                                    "✅ All {} tasks have been completed successfully!\n\n{}\n\n=== What would you like to do next? ===\n• Type 'review' to see a detailed summary\n• Type 'continue' to create follow-up tasks\n• Type 'done' to end the session\n• Or provide any other instructions for the planner",
                                    tasks.len(),
                                    task_summary
                                ),
                                context: Some(serde_json::json!({
                                    "event": "all_tasks_completed",
                                    "task_count": tasks.len(),
                                    "completed_tasks": tasks
                                })),
                            };

                            // Send to planner aggregate since we're in planning mode
                            store.push_event(&stream_id, "planner", &completion_display, &Default::default()).await?;
                            tracing::info!("Displayed completion summary to user");

                            current_task_index = None; // Reset to wait for new plan or user input
                        }
                    }
                }
            }
        }
    }
}