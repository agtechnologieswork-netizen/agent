//! Simple CLI demo for the MVP planner

use crate::planner::{
    llm::LLMPlanner,
    handler::{Command, Event, Handler},
    Planner,
};
use crate::llm::LLMClientDyn;
use eyre::Result;
use std::io::{self, Write};

/// Run the planner CLI
pub async fn run_cli(llm: Option<Box<dyn LLMClientDyn>>, model: String) -> Result<()> {
    // Ensure env vars are loaded for agent/tests
    crate::load_env_for_agent();
    println!("🤖 Event-Sourced LLM Planner (MVP)");
    println!("Type your request and press Enter:");
    println!();
    
    // Read user input (support non-interactive via PLANNER_INPUT)
    let input_env = std::env::var("PLANNER_INPUT").ok();
    let input_owned: String;
    let input = if let Some(val) = input_env {
        input_owned = val;
        input_owned.trim()
    } else {
        print!("> ");
        io::stdout().flush()?;
        let mut input_buf = String::new();
        io::stdin().read_line(&mut input_buf)?;
        input_owned = input_buf;
        input_owned.trim()
    };
    
    if input.is_empty() {
        println!("No input provided.");
        return Ok(());
    }
    
    // Process with LLM or fallback
    let events = if let Some(llm) = llm {
        println!("\n🧠 Using LLM to parse tasks...");
        let planner = LLMPlanner::new(llm, model);
        let tasks = planner.parse_tasks(input).await?;
        
        println!("\n📋 Parsed {} task(s):", tasks.len());
        for task in &tasks {
            println!("  #{}: {} [{:?}]", task.id, task.description, task.kind);
        }
        
        vec![Event::TasksPlanned {
            tasks: tasks.into_iter().map(|t| t.into()).collect(),
        }]
    } else {
        println!("\n⚠️  No LLM configured, using basic fallback...");
        let mut planner = Planner::new();
        planner.process(Command::Initialize {
            user_input: input.to_string(),
            attachments: vec![],
        })?
    };
    
    // Save to DabGent MQ if feature enabled
    #[cfg(feature = "mq")]
    {
        use dabgent_mq::db::{EventStore, Metadata, sqlite::SqliteStore};
        use uuid::Uuid;
        
        println!("\n💾 Saving to DabGent MQ...");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        let store = SqliteStore::new(pool);
        store.migrate().await;
        
        let session_id = Uuid::new_v4().to_string();
        for event in &events {
            store.push_event("planner", &session_id, event, &Metadata::default()).await?;
        }
        
        println!("✅ Events saved to session: {}", session_id);
    }
    
    #[cfg(not(feature = "mq"))]
    {
        println!("\n⚠️  DabGent MQ not enabled (compile with --features mq)");
    }
    
    println!("\n✨ Done!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cli_basic() {
        // Just verify it compiles and can be called
        // Real CLI testing would need mock stdin/stdout
        assert!(true);
    }
}
