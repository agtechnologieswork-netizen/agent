use crate::db::sqlite::SqliteStore;

/// Create an in-memory SQLite store for testing
pub async fn create_memory_store() -> SqliteStore {
    let pool = sqlx::SqlitePool::connect(":memory:").await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}