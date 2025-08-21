use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use std::env;
use tower_http::cors::CorsLayer;
use tracing_subscriber;

pub mod models;
pub mod schema;

// type DbPool = Pool<ConnectionManager<PgConnection>>;

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "healthy")
}

async fn index() -> impl IntoResponse {
    let html = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Created with ♥️ by app.build</title>
        <script src="https://unpkg.com/htmx.org@1.9.10"></script>
        <style>
            body { font-family: Arial, sans-serif; margin: 2rem; }
            .container { max-width: 800px; margin: 0 auto; }
        </style>
    </head>
    <body>
        <div class="container">
            <h1>Welcome to your Rust App</h1>
            <p>Built with Axum + HTMX + Diesel</p>
        </div>
    </body>
    </html>
    "#;
    Html(html)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = Pool::builder()
        .build(manager)
        .expect("Failed to create pool");

    let app = Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .layer(CorsLayer::permissive())
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server running on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}