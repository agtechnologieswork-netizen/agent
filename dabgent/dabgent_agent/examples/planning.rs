use dabgent_agent::execution::run_execution_worker;
use dabgent_agent::orchestrator::Orchestrator;
use dabgent_agent::planner_events::PlannerEvent;
use dabgent_mq::db::{EventStore, Metadata, Query};
use dabgent_sandbox::Sandbox;
use dabgent_sandbox::utils::create_sandbox;
use eyre::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    run().await.unwrap();
}

async fn run() -> Result<()> {
    let result = dagger_sdk::connect(|client| async move {
        let sandbox = create_sandbox(&client, "./examples", "Dockerfile").await?;
        let sandbox = Arc::new(Mutex::new(sandbox.boxed()));
        let store = dabgent_mq::test_utils::create_memory_store().await;

        // Start persistent execution worker
        let execution_stream = "example_execution".to_string();
        let worker_sandbox = sandbox.clone();
        let worker_store = store.clone();

        tokio::spawn(async move {
            let _ = run_execution_worker(
                worker_store,
                execution_stream,
                "demo".to_string(),
                worker_sandbox
            ).await;
        });

        // Create orchestrator
        let mut orchestrator = Orchestrator::new(
            store.clone(),
            "example".to_string(),
            "demo".to_string()
        );

        // Task to execute
        let task = "Create a simple Python script that fetches weather data from an API and saves it to a CSV file";

        println!("📝 Creating plan for task: {}", task);

        // Phase 1: Create and present plan
        orchestrator.create_plan(task.to_string()).await?;

        // Display the plan
        let planning_stream = "example_planning".to_string();
        let events = store.load_events::<PlannerEvent>(&Query {
            stream_id: planning_stream.clone(),
            event_type: Some("plan_presented".to_string()),
            aggregate_id: Some("demo".to_string()),
        }, None).await?;

        if let Some(PlannerEvent::PlanPresented { tasks }) = events.last() {
            println!("\n📋 Proposed Plan:");
            for task in tasks {
                println!("  {}. {}", task.id + 1, task.description);
            }
        }

        // Phase 2: Simulate user approval
        println!("\n✅ Simulating user approval...");
        store.push_event(
            &planning_stream,
            "demo",
            &PlannerEvent::PlanApproved,
            &Metadata::default()
        ).await?;

        // Wait for approval to be processed
        let approved = orchestrator.wait_for_approval().await?;
        if !approved {
            println!("❌ Plan was rejected");
            return Ok(());
        }

        println!("✅ Plan approved! Starting execution...\n");

        // Phase 3: Queue execution to the persistent worker
        orchestrator.queue_execution().await?;

        // Monitor progress
        orchestrator.monitor_execution(|status| {
            Box::pin(async move {
                println!("{}", status);
                Ok(())
            })
        }).await?;

        println!("\n🎉 Example completed successfully!");
        Ok(())
    }).await;

    result.map_err(|e| eyre::eyre!("Dagger connection error: {}", e))
}