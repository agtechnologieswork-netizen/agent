use crate::db::*;
use chrono::Utc;
use serde_json;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/sqlite");

#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
    watchers: Arc<Mutex<HashMap<Query, broadcast::WeakSender<Event>>>>,
    write_lock: Arc<tokio::sync::Mutex<()>>,
}

impl SqliteStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            watchers: Arc::new(Mutex::new(HashMap::new())),
            write_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub async fn migrate(&self) {
        MIGRATOR.run(&self.pool).await.expect("Migration failed")
    }

    fn build_events_query(query: &Query, last_sequence: Option<i64>) -> (String, Vec<String>) {
        let mut conditions = vec!["stream_id = ?".to_string()];
        let mut params = vec![query.stream_id.clone()];

        if let Some(event_type) = &query.event_type {
            conditions.push("event_type = ?".to_string());
            params.push(event_type.clone());
        }

        if let Some(aggregate_id) = &query.aggregate_id {
            conditions.push("aggregate_id = ?".to_string());
            params.push(aggregate_id.clone());
        }

        if let Some(last_seq) = last_sequence {
            conditions.push("sequence > ?".to_string());
            params.push(last_seq.to_string());
        }

        let where_clause = conditions.join(" AND ");
        let sql = format!(
            "SELECT * FROM events WHERE {} ORDER BY sequence ASC",
            where_clause
        );
        (sql, params)
    }

    async fn poll_events(
        pool: &SqlitePool,
        query: &Query,
        last_sequence: i64,
    ) -> Result<Vec<Event>, Error> {
        const POLL_INTERVAL: Duration = Duration::from_millis(500);

        let (sql, params) = Self::build_events_query(query, Some(last_sequence));
        loop {
            let mut sqlx_query = sqlx::query_as::<_, Event>(&sql);
            for param in params.iter() {
                sqlx_query = sqlx_query.bind(param);
            }
            let events = sqlx_query.fetch_all(pool).await.map_err(Error::Database)?;
            if !events.is_empty() {
                return Ok(events);
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }
}

impl EventStore for SqliteStore {
    async fn push_event<T: models::Event>(
        &self,
        stream_id: &str,
        aggregate_id: &str,
        event: &T,
        metadata: &Metadata,
    ) -> Result<(), Error> {
        let event_data = serde_json::to_value(event).map_err(Error::Serialization)?;
        let metadata_json = serde_json::to_value(metadata).map_err(Error::Serialization)?;

        let _write_lock = self.write_lock.lock().await;
        let mut tx = self.pool.begin().await.map_err(Error::Database)?;

        let next_sequence: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM events WHERE stream_id = ?",
        )
        .bind(stream_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(Error::Database)?;

        sqlx::query(
            r#"
            INSERT INTO events (stream_id, event_type, aggregate_id, sequence, event_version, data, metadata, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(stream_id)
        .bind(T::event_type())
        .bind(aggregate_id)
        .bind(next_sequence)
        .bind(T::EVENT_VERSION)
        .bind(event_data)
        .bind(metadata_json)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;

        tx.commit().await.map_err(Error::Database)?;

        Ok(())
    }

    async fn load_events<T: models::Event>(&self, query: &Query) -> Result<Vec<T>, Error> {
        let (sql, params) = Self::build_events_query(query, None);
        let mut sqlx_query = sqlx::query_as::<_, Event>(&sql);
        for param in params {
            sqlx_query = sqlx_query.bind(param);
        }

        let rows = sqlx_query
            .fetch_all(&self.pool)
            .await
            .map_err(Error::Database)?;

        rows.into_iter()
            .map(|row| serde_json::from_value::<T>(row.data).map_err(Error::Serialization))
            .collect::<Result<Vec<T>, Error>>()
    }

    fn get_or_create_watcher(&self, query: &Query) -> broadcast::Receiver<Event> {
        let mut watchers = self.watchers.lock().unwrap();
        if let Some(tx) = watchers
            .get(query)
            .and_then(|tx| tx.upgrade().map(|tx| tx.subscribe()))
        {
            return tx;
        }
        let pool = self.pool.clone();
        let query = query.clone();
        let (tx, rx) = broadcast::channel(1);
        watchers.insert(query.clone(), tx.downgrade());

        tokio::spawn(async move {
            tracing::info!(?query, "watcher started");
            let mut last_seen = 0i64;
            'main: loop {
                match Self::poll_events(&pool, &query, last_seen).await {
                    Ok(events) => {
                        tracing::info!(num_events = events.len(), "fetched events");
                        for event in events {
                            tracing::info!(?event.sequence, "sending event");
                            let sequence = event.sequence;
                            if let Err(err) = tx.send(event) {
                                tracing::error!(?err, "error sending event");
                                break 'main;
                            }
                            last_seen = last_seen.max(sequence);
                        }
                    }
                    Err(err) => {
                        tracing::error!(?err, "error polling events");
                        break 'main;
                    }
                }
            }
            tracing::info!(?query, "watcher stopped");
        });
        rx
    }
}
