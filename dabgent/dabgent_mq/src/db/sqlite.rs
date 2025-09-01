use crate::db::*;
use chrono::Utc;
use serde_json;
use sqlx::SqlitePool;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/sqlite");

pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) {
        MIGRATOR.run(&self.pool).await.expect("Migration failed")
    }

    fn build_where(query: &Query) -> (String, Vec<String>) {
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

        let where_clause = conditions.join(" AND ");
        (where_clause, params)
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

        let mut tx = self.pool.begin().await.map_err(Error::Database)?;

        let next_sequence: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM events WHERE stream_id = ? AND aggregate_id = ?",
        )
        .bind(stream_id)
        .bind(aggregate_id)
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
}
