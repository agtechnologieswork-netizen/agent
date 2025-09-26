use clap::Parser;
use dabgent_cli::{App, agent::run_pipeline};
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

    /// Show debug events in the UI
    #[arg(long, default_value = "false")]
    show_debug: bool,
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
    let app = App::new(store.clone(), stream_id.clone(), args.show_debug)?;

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
