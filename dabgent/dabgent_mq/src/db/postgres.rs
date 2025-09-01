use crate::db::*;
use chrono::Utc;
use serde_json;
use sqlx::PgPool;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::{broadcast, mpsc};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/postgres");

#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
    watchers: Arc<Mutex<HashMap<Query, broadcast::Receiver<Event>>>>,
}

impl PostgresStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            watchers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn migrate(&self) {
        MIGRATOR.run(&self.pool).await.expect("Migration failed")
    }

    fn start_subscriber(&self, query: &Query) -> broadcast::Receiver<Event> {
        let (where_clause, params) = Self::build_where(query);

        let pool = self.pool.clone();
        let (tx, rx) = broadcast::channel(1);
        tokio::spawn(async move {
            let sql = format!(
                "SELECT * FROM events WHERE {} ORDER BY sequence ASC",
                where_clause
            );
            loop {
                let mut sqlx_query = sqlx::query_as::<_, Event>(&sql);
                for param in params.iter() {
                    sqlx_query = sqlx_query.bind(param);
                }
                let events = match sqlx_query.fetch_all(&pool).await {
                    Ok(events) => events,
                    Err(err) => {
                        tracing::error!("Error fetching events: {}", err);
                        continue;
                    }
                };
                for event in events {
                    let _ = tx.send(event);
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        });
        let mut subscribers = self.watchers.lock().unwrap();
        subscribers.insert(query.clone(), rx);
        subscribers[query].resubscribe()
    }

    fn build_where(query: &Query) -> (String, Vec<String>) {
        let mut conditions = vec!["stream_id = $1".to_string()];
        let mut params = vec![query.stream_id.clone()];
        let mut param_counter = 2;

        if let Some(event_type) = &query.event_type {
            conditions.push(format!("event_type = ${}", param_counter));
            params.push(event_type.clone());
            param_counter += 1;
        }

        if let Some(aggregate_id) = &query.aggregate_id {
            conditions.push(format!("aggregate_id = ${}", param_counter));
            params.push(aggregate_id.clone());
        }

        let where_clause = conditions.join(" AND ");
        (where_clause, params)
    }
}

impl EventStore for PostgresStore {
    async fn push_event<T: models::Event>(
        &self,
        stream_id: &str,
        aggregate_id: &str,
        event: &T,
        metadata: &Metadata,
    ) -> Result<(), Error> {
        let event_data = serde_json::to_value(event).map_err(Error::Serialization)?;
        let metadata_json = serde_json::to_value(metadata).map_err(Error::Serialization)?;

        let mut tx = self.pool.begin().await.map_err(Error::Database)?;

        let next_sequence: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM events WHERE stream_id = ?",
        )
        .bind(stream_id)
        .bind(aggregate_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(Error::Database)?;

        sqlx::query(
            r#"
            INSERT INTO events (stream_id, event_type, aggregate_id, sequence, event_version, data, metadata, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
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
        let (where_clause, params) = Self::build_where(query);

        let sql = format!(
            "SELECT * FROM events WHERE {} ORDER BY sequence ASC",
            where_clause
        );

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

    fn subscribe<T: models::Event + 'static>(
        &self,
        query: &Query,
    ) -> Result<mpsc::Receiver<T>, Error> {
        let watchers = self.watchers.lock().unwrap();
        let mut event_rx = match watchers.get(query) {
            Some(rx) => rx.resubscribe(),
            None => self.start_subscriber(query),
        };
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            while let Ok(event) = event_rx.recv().await {
                match serde_json::from_value::<T>(event.data) {
                    Ok(event) => {
                        let _ = tx.send(event).await;
                    }
                    Err(err) => {
                        tracing::error!("Failed to deserialize event: {}", err);
                    }
                }
            }
        });
        Ok(rx)
    }
}
