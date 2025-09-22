use super::*;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/postgres");

#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
    stream_id: String,
}

impl PostgresStore {
    pub fn new<T: AsRef<str>>(pool: PgPool, stream_id: T) -> Self {
        Self {
            pool,
            stream_id: stream_id.as_ref().to_string(),
        }
    }

    fn select_query<T: AsRef<str>>(
        &self,
        aggregate_type: T,
        aggregate_id: Option<T>,
        offset: Option<i64>,
    ) -> (String, Vec<String>) {
        let mut conditions = vec![
            "stream_id = $1".to_owned(),
            "aggregate_type = $2".to_owned(),
        ];
        let mut params = vec![self.stream_id.clone(), aggregate_type.as_ref().to_string()];
        if let Some(aggregate_id) = aggregate_id {
            conditions.push("aggregate_id = $3".to_owned());
            params.push(aggregate_id.as_ref().to_string());
        }
        if let Some(offset) = offset {
            conditions.push("sequence > $4".to_owned());
            params.push(offset.to_string());
        }
        let where_clause = conditions.join(" AND ");
        let sql = format!("SELECT * FROM events WHERE {where_clause} ORDER BY sequence ASC");
        (sql, params)
    }
}

impl PostgresStore {
    pub async fn migrate(&self) {
        MIGRATOR.run(&self.pool).await.expect("Migration failed")
    }
}

impl EventStore for PostgresStore {
    async fn commit<A: Aggregate>(
        &self,
        events: Vec<A::Event>,
        metadata: Metadata,
        context: AggregateContext<A>,
    ) -> Result<Vec<Envelope<A>>, Error> {
        let wrapped = wrap_events::<A>(
            &context.aggregate_id,
            context.current_sequence,
            events,
            metadata,
        );
        let serialized = wrapped
            .iter()
            .map(SerializedEvent::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let mut tx = self.pool.begin().await.map_err(Error::Database)?;
        for event in serialized.into_iter() {
            sqlx::query(
                r#"
                INSERT INTO events (stream_id, aggregate_type, aggregate_id, sequence, event_type, event_version, data, metadata)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8);
                "#
            )
            .bind(&self.stream_id)
            .bind(A::TYPE)
            .bind(event.aggregate_id)
            .bind(event.sequence)
            .bind(event.event_type)
            .bind(event.event_version)
            .bind(event.data)
            .bind(event.metadata)
            .execute(&mut *tx)
            .await
            .map_err(Error::Database)?;
        }
        tx.commit().await.map_err(Error::Database)?;
        Ok(wrapped)
    }

    async fn load_aggregate<A: Aggregate>(
        &self,
        aggregate_id: &str,
    ) -> Result<AggregateContext<A>, Error> {
        let events = self.load_events::<A>(aggregate_id).await?;
        let mut aggregate = A::default();
        let mut current_sequence = 0;
        for event in events {
            current_sequence = event.sequence;
            aggregate.apply(event.data)
        }
        Ok(AggregateContext {
            aggregate_id: aggregate_id.to_owned(),
            current_sequence,
            aggregate,
        })
    }

    async fn load_events<A: Aggregate>(
        &self,
        aggregate_id: &str,
    ) -> Result<Vec<Envelope<A>>, Error> {
        let (sql, params) = self.select_query(A::TYPE, Some(aggregate_id), None);
        let mut query = sqlx::query_as::<_, SerializedEvent>(&sql);
        for param in params {
            query = query.bind(param);
        }
        let serialized = query.fetch_all(&self.pool).await.map_err(Error::Database)?;
        serialized
            .into_iter()
            .map(Envelope::try_from)
            .collect::<Result<Vec<_>, _>>()
    }
}
