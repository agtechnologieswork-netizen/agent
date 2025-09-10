//! Full planning example with executor integration
//! 
//! This example demonstrates the complete planner-executor integration,
//! showing how tasks are planned, dispatched, executed, and completed.

use dabgent_agent::handler::Handler;
use dabgent_agent::llm::{LLMClient, LLMClientDyn};
use dabgent_agent::planner::{
    Planner, Command, Event, Executor, ExecutorCommand, ExecutorEventOutput,
    TaskPlan, ExecutorEvent
};
use dabgent_agent::planner::llm::LLMPlanner;
use dabgent_mq::db::sqlite::SqliteStore;
use eyre::Result;
use rig::client::ProviderClient;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().init();

    // Initialize LLM client
    let llm = rig::providers::anthropic::Client::from_env();

    // Initialize event store (in-memory for example)
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await?;
    sqlx::migrate!("../dabgent_mq/migrations/sqlite").run(&pool).await?;
    let store = SqliteStore::new(pool);

    // Example input
    let input = "Create a Python script that fetches weather data for New York and saves it to a JSON file";

    // Run the full planning and execution
    run_planning_with_execution(llm, store, input.to_string()).await?;

    Ok(())
}

async fn run_planning_with_execution<T, E>(
    llm: T,
    store: E,
    input: String,
) -> Result<()>
where
    T: LLMClient + Clone + 'static,
    E: dabgent_mq::EventStore + Clone,
{
    println!("🎯 Starting planning for: {}", input);
    println!("{}", "─".repeat(80));

    // Parse tasks using LLM
    let llm_planner = LLMPlanner::new(
        Box::new(llm.clone()) as Box<dyn LLMClientDyn>,
        std::env::var("PLANNER_MODEL").unwrap_or_else(|_| "sonnet-4.1".to_string()),
    );
    
    println!("🤖 Parsing tasks with LLM...");
    let parsed_tasks = llm_planner.parse_tasks(&input).await?;
    
    // Convert to TaskPlan
    let tasks: Vec<TaskPlan> = parsed_tasks.into_iter()
        .map(|t| t.into())
        .collect();
    
    println!("📋 Parsed {} tasks", tasks.len());
    println!();

    // Initialize planner and executor
    let mut planner = Planner::new();
    let mut executor = Executor::new();
    
    // Initialize planner with parsed tasks
    let initial_events = planner.process(Command::Initialize { tasks })?;
    
    // Process initial events
    for event in &initial_events {
        match event {
            Event::TasksPlanned { tasks } => {
                println!("📝 Planned Tasks:");
                for task in tasks {
                    println!("   [{}] {} ({:?})", task.id, task.description, task.kind);
                }
                println!();
            }
            Event::TaskDispatched { task_id, .. } => {
                println!("🚀 Dispatched task {}", task_id);
            }
            _ => {}
        }
        
        // Store event
        store.push_event("planner", "demo", event, &Default::default()).await?;
    }
    
    // Collect dispatched tasks
    let mut pending_tasks = Vec::new();
    for event in initial_events {
        if let Event::TaskDispatched { task_id, command } = event {
            pending_tasks.push((task_id, command));
        }
    }
    
    // Execute tasks and handle feedback loop
    let mut task_count = 0;
    while !pending_tasks.is_empty() {
        let (task_id, command) = pending_tasks.remove(0);
        task_count += 1;
        
        println!("⚙️  Executing task {}...", task_id);
        
        // Execute the task
        let exec_events = executor.process(ExecutorCommand::ExecuteTask(command))?;
        
        // Process executor events
        for exec_event in exec_events {
            match &exec_event {
                ExecutorEventOutput::TaskStarted { task_id } => {
                    println!("   ▶️  Started task {}", task_id);
                }
                ExecutorEventOutput::TaskCompleted { task_id, result } => {
                    println!("   ✅ Completed task {}: {}", task_id, result);
                }
                ExecutorEventOutput::TaskFailed { task_id, error } => {
                    println!("   ❌ Failed task {}: {}", task_id, error);
                }
                ExecutorEventOutput::NeedsClarification { task_id, question } => {
                    println!("   ❓ Task {} needs clarification: {}", task_id, question);
                    
                    // Simulate providing clarification
                    println!("   💬 Providing clarification...");
                    let clarify_events = executor.process(ExecutorCommand::ProvideClarification {
                        task_id: *task_id,
                        answer: "Use OpenWeatherMap API with default settings".to_string(),
                    })?;
                    
                    for clarify_event in clarify_events {
                        store.push_event("executor", "demo", &clarify_event, &Default::default()).await?;
                    }
                }
                _ => {}
            }
            
            // Store executor event
            store.push_event("executor", "demo", &exec_event, &Default::default()).await?;
            
            // Convert relevant executor events back to planner
            if let Some(planner_exec_event) = convert_to_planner_event(&exec_event) {
                let planner_response = planner.process(
                    Command::HandleExecutorEvent(planner_exec_event)
                )?;
                
                // Process planner's response
                for response_event in planner_response {
                    match &response_event {
                        Event::TaskDispatched { task_id: new_id, command: new_cmd } => {
                            println!("🚀 Dispatched next task {}", new_id);
                            pending_tasks.push((*new_id, new_cmd.clone()));
                        }
                        Event::PlanningCompleted { summary } => {
                            println!();
                            println!("{}", "─".repeat(80));
                            println!("🎉 Planning completed: {}", summary);
                            println!("📊 Total tasks executed: {}", task_count);
                            return Ok(());
                        }
                        _ => {}
                    }
                    
                    // Store planner event
                    store.push_event("planner", "demo", &response_event, &Default::default()).await?;
                }
            }
        }
        
        println!();
    }
    
    // Check if planning is complete
    let final_events = planner.process(Command::Continue)?;
    for event in final_events {
        if let Event::PlanningCompleted { summary } = &event {
            println!("{}", "─".repeat(80));
            println!("🎉 Planning completed: {}", summary);
            println!("📊 Total tasks executed: {}", task_count);
        }
        store.push_event("planner", "demo", &event, &Default::default()).await?;
    }
    
    Ok(())
}

/// Convert executor events to planner's executor events
fn convert_to_planner_event(event: &ExecutorEventOutput) -> Option<ExecutorEvent> {
    match event {
        ExecutorEventOutput::TaskCompleted { task_id, result } => {
            Some(ExecutorEvent::TaskCompleted { 
                node_id: *task_id, 
                result: result.clone() 
            })
        }
        ExecutorEventOutput::TaskFailed { task_id, error } => {
            Some(ExecutorEvent::TaskFailed { 
                node_id: *task_id, 
                error: error.clone() 
            })
        }
        ExecutorEventOutput::NeedsClarification { task_id, question } => {
            Some(ExecutorEvent::NeedsClarification { 
                node_id: *task_id, 
                question: question.clone() 
            })
        }
        ExecutorEventOutput::ClarificationProvided { task_id, answer } => {
            Some(ExecutorEvent::ClarificationProvided { 
                node_id: *task_id, 
                answer: answer.clone() 
            })
        }
        _ => None
    }
}
