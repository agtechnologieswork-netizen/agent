use clap::Parser;
use dabgent_cli::{App, agent::{run_pipeline, run_planning_pipeline}};
use dabgent_mq::db::sqlite::SqliteStore;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "dabgent")]
#[command(about = "Dabgent CLI - AI agent with planning capabilities")]
struct Args {
    #[arg(long, default_value = ":memory:")]
    database: String,

    /// Load environment from .env file
    #[arg(long, default_value = "true")]
    dotenv: bool,

    /// Enable planning mode to break down complex tasks
    #[arg(long, short = 'p')]
    planning: bool,

    /// Task to plan and execute (for non-interactive planning mode)
    #[arg(long, short = 't')]
    task: Option<String>,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let args = Args::parse();
    if args.dotenv {
        let _ = dotenvy::dotenv();
    }
    let pool = SqlitePool::connect(&args.database).await?;
    let store = SqliteStore::new(pool);
    store.migrate().await;

    let stream_id = format!("{}_session", Uuid::now_v7());

    if args.planning && args.task.is_some() {
        let task = args.task.unwrap();
        println!("Running in planning mode with task: {}", task);
        run_planning_pipeline(store, stream_id, task).await;
        return Ok(());
    }

    let app = App::new(store.clone(), stream_id.clone())?;
    let terminal = ratatui::init();
    let result = tokio::select! {
        _ = run_pipeline(store, stream_id) => {
            Ok(())
        },
        res = app.run(terminal) => {
            res
        }
    };
    // ratatui::restore();
    result
}
