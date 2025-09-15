use crate::session::{ChatCommand, ChatEvent, ChatSession};
use dabgent_agent::execution::run_execution_worker;
use dabgent_agent::handler::Handler;
use dabgent_agent::orchestrator::Orchestrator;
use dabgent_agent::planner_events::PlannerEvent;
use dabgent_agent::utils;
use dabgent_mq::db::{EventStore, Metadata, Query};
use dabgent_sandbox::Sandbox;
use dabgent_sandbox::utils::create_sandbox;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Agent<S: EventStore> {
    store: S,
    stream_id: String,
    aggregate_id: String,
}

impl<S: EventStore> Agent<S> {
    pub fn new(store: S, stream_id: String, aggregate_id: String) -> Self {
        Self {
            store,
            stream_id,
            aggregate_id,
        }
    }

    pub async fn run(self) -> color_eyre::Result<()> {
        dagger_sdk::connect(|client| async move {
            // Note: LLM would be used here for actual task execution
            // For now, the execution worker uses simple simulation
            let _llm = utils::create_llm_client().map_err(|e| color_eyre::eyre::eyre!(e))?;

            // Get sandbox configuration from environment
            let context_dir = std::env::var("SANDBOX_CONTEXT_DIR")
                .unwrap_or_else(|_| {
                    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                    path.push("../dabgent_agent/examples");
                    path.canonicalize()
                        .unwrap_or_else(|_| std::path::PathBuf::from("./dabgent_agent/examples"))
                        .to_string_lossy()
                        .to_string()
                });
            let dockerfile = std::env::var("SANDBOX_DOCKERFILE")
                .unwrap_or_else(|_| "Dockerfile".to_owned());

            // Create a single sandbox that will be shared across all requests
            let sandbox = create_sandbox(&client, &context_dir, &dockerfile).await?;
            let sandbox = Arc::new(Mutex::new(sandbox.boxed()));

            // Start a single persistent execution worker
            let execution_stream = format!("{}_execution", self.stream_id);
            let worker_sandbox = sandbox.clone();
            let worker_store = self.store.clone();
            let worker_stream = execution_stream.clone();
            let worker_aggregate = self.aggregate_id.clone();

            tokio::spawn(async move {
                let _ = run_execution_worker(
                    worker_store,
                    worker_stream,
                    worker_aggregate,
                    worker_sandbox
                ).await;
            });

            let mut event_stream = self.store.subscribe::<ChatEvent>(&Query {
                stream_id: self.stream_id.clone(),
                event_type: Some("user_message".to_string()),
                aggregate_id: Some(self.aggregate_id.clone()),
            })?;

            while let Some(Ok(ChatEvent::UserMessage { content, .. })) = event_stream.next().await {
                // Create a new orchestrator for this request
                let mut orchestrator = Orchestrator::new(
                    self.store.clone(),
                    self.stream_id.clone(),
                    self.aggregate_id.clone(),
                );

                // Phase 1: Create and present plan
                send_agent_message(
                    &self.store,
                    &self.stream_id,
                    &self.aggregate_id,
                    "📝 Creating plan...".to_string()
                ).await?;

                orchestrator.create_plan(content.clone()).await?;

                // Show plan to user
                let planning_stream = format!("{}_planning", self.stream_id);
                let events = self.store.load_events::<PlannerEvent>(&Query {
                    stream_id: planning_stream.clone(),
                    event_type: Some("plan_presented".to_string()),
                    aggregate_id: Some(self.aggregate_id.clone()),
                }, None).await?;

                if let Some(PlannerEvent::PlanPresented { tasks }) = events.last() {
                    let mut plan_msg = "📋 **Proposed Plan:**\n".to_string();
                    for task in tasks {
                        plan_msg.push_str(&format!("{}. {}\n", task.id + 1, task.description));
                    }
                    plan_msg.push_str("\n✅ Type 'approve' to execute or provide feedback");

                    send_agent_message(
                        &self.store,
                        &self.stream_id,
                        &self.aggregate_id,
                        plan_msg
                    ).await?;
                }

                // Phase 2: Wait for approval
                let approved = wait_for_user_approval(
                    &self.store,
                    &self.stream_id,
                    &self.aggregate_id,
                    &planning_stream
                ).await?;

                if !approved {
                    send_agent_message(
                        &self.store,
                        &self.stream_id,
                        &self.aggregate_id,
                        "❌ Plan rejected. Please provide a new request.".to_string()
                    ).await?;
                    continue;
                }

                // Phase 3: Queue execution to the persistent worker
                send_agent_message(
                    &self.store,
                    &self.stream_id,
                    &self.aggregate_id,
                    "✅ Plan approved! Starting execution...".to_string()
                ).await?;

                // Send plan to execution worker via event
                orchestrator.queue_execution().await?;

                // Monitor and report progress
                let store = self.store.clone();
                let stream_id = self.stream_id.clone();
                let aggregate_id = self.aggregate_id.clone();

                orchestrator.monitor_execution(move |status| {
                    let store = store.clone();
                    let stream_id = stream_id.clone();
                    let aggregate_id = aggregate_id.clone();
                    Box::pin(async move {
                        send_agent_message(&store, &stream_id, &aggregate_id, status).await
                            .map_err(|e| eyre::eyre!(e))
                    })
                }).await?;
            }

            Ok(())
        }).await?;
        Ok(())
    }
}

/// Send a message from the agent to the chat session
async fn send_agent_message<S: EventStore>(
    store: &S,
    stream_id: &str,
    aggregate_id: &str,
    content: String,
) -> color_eyre::Result<()> {
    let events = store.load_events::<ChatEvent>(&Query {
        stream_id: stream_id.to_string(),
        event_type: None,
        aggregate_id: Some(aggregate_id.to_string()),
    }, None).await?;

    let mut session = ChatSession::fold(&events);
    let new_events = session.process(ChatCommand::AgentRespond(content))?;

    for event in new_events {
        store
            .push_event(stream_id, aggregate_id, &event, &Metadata::default())
            .await?;
    }
    Ok(())
}

/// Wait for user approval of the plan
async fn wait_for_user_approval<S: EventStore>(
    store: &S,
    stream_id: &str,
    aggregate_id: &str,
    planning_stream: &str,
) -> color_eyre::Result<bool> {
    // Subscribe to user messages for approval
    let mut event_stream = store.subscribe::<ChatEvent>(&Query {
        stream_id: stream_id.to_string(),
        event_type: Some("user_message".to_string()),
        aggregate_id: Some(aggregate_id.to_string()),
    })?;

    while let Some(Ok(ChatEvent::UserMessage { content, .. })) = event_stream.next().await {
        let content_lower = content.to_lowercase();

        if content_lower.contains("approve") || content_lower == "yes" || content_lower == "ok" {
            // Publish approval event
            store.push_event(
                planning_stream,
                aggregate_id,
                &PlannerEvent::PlanApproved,
                &Metadata::default()
            ).await?;
            return Ok(true);
        } else if content_lower.contains("reject") || content_lower == "no" {
            // Publish rejection event
            store.push_event(
                planning_stream,
                aggregate_id,
                &PlannerEvent::PlanRejected { reason: content },
                &Metadata::default()
            ).await?;
            return Ok(false);
        } else {
            // User feedback - they might want to modify the plan
            store.push_event(
                planning_stream,
                aggregate_id,
                &PlannerEvent::UserFeedback { content: content.clone() },
                &Metadata::default()
            ).await?;

            send_agent_message(
                store,
                stream_id,
                aggregate_id,
                "💬 Feedback noted. Type 'approve' to execute or 'reject' to cancel.".to_string()
            ).await?;
        }
    }

    Ok(false)
}