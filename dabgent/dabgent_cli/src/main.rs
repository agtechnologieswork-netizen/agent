use clap::Parser;
use dabgent_cli::{App, agent::run_pipeline};
use dabgent_mq::db::sqlite::SqliteStore;
use dabgent_mq::db::postgres::PostgresStore;
use sqlx::{SqlitePool, PgPool};
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
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let args = Args::parse();
    if args.dotenv {
        let _ = dotenvy::dotenv();
    }

    let stream_id = format!("{}_session", Uuid::now_v7());

    // Check for POSTGRES_URL environment variable
    if let Ok(postgres_url) = std::env::var("POSTGRES_URL") {
        // Use PostgreSQL if POSTGRES_URL is set
        eprintln!("🔌 Connecting to PostgreSQL database...");
        let pool = PgPool::connect(&postgres_url).await?;
        let store = PostgresStore::new(pool);
        store.migrate().await;
        eprintln!("✅ Connected to PostgreSQL");

        let app = App::new(store.clone(), stream_id.clone())?;
        let terminal = ratatui::init();
        let result = tokio::select! {
            _ = run_pipeline(store.clone(), stream_id) => {
                Ok(())
            },
            res = app.run(terminal) => {
                res
            }
        };
        // ratatui::restore();
        result
    } else {
        // Use in-memory SQLite by default (or file if database arg is provided)
        let db_url = if args.database == ":memory:" {
            eprintln!("📦 Using in-memory SQLite database");
            ":memory:"
        } else {
            eprintln!("📦 Using SQLite database: {}", args.database);
            &args.database
        };

        let pool = SqlitePool::connect(db_url).await?;
        let store = SqliteStore::new(pool);
        store.migrate().await;

        let app = App::new(store.clone(), stream_id.clone())?;
        let terminal = ratatui::init();
        let result = tokio::select! {
            _ = run_pipeline(store.clone(), stream_id) => {
                Ok(())
            },
            res = app.run(terminal) => {
                res
            }
        };
        // ratatui::restore();
        result
    }
}
