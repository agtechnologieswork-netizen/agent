pub mod postgres;
pub mod sqlite;
use crate::models::{self};
use chrono::{DateTime, Utc};
use eyre::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tokio::sync::{broadcast, mpsc};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Event {
    pub stream_id: String,
    pub event_type: String,
    pub aggregate_id: String,
    pub sequence: i64,
    pub event_version: String,
    pub data: JsonValue,
    pub metadata: JsonValue,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub correlation_id: Option<uuid::Uuid>,
    pub causation_id: Option<uuid::Uuid>,
    pub extra: Option<JsonValue>,
}

impl Metadata {
    pub fn new(
        correlation_id: Option<uuid::Uuid>,
        causation_id: Option<uuid::Uuid>,
        extra: Option<JsonValue>,
    ) -> Self {
        Metadata {
            correlation_id,
            causation_id,
            extra,
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: uuid::Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    pub fn with_causation_id(mut self, causation_id: uuid::Uuid) -> Self {
        self.causation_id = Some(causation_id);
        self
    }

    pub fn with_extra(mut self, extra: JsonValue) -> Self {
        self.extra = Some(extra);
        self
    }
}

impl Default for Metadata {
    fn default() -> Self {
        Metadata {
            correlation_id: None,
            causation_id: None,
            extra: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Query {
    pub stream_id: String,
    pub event_type: Option<String>,
    pub aggregate_id: Option<String>,
}

pub trait EventStore {
    fn push_event<T: models::Event>(
        &self,
        stream_id: &str,
        aggregate_id: &str,
        event: &T,
        metadata: &Metadata,
    ) -> impl Future<Output = Result<(), Error>> + Send;
    fn load_events<T: models::Event>(
        &self,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<T>, Error>> + Send;
    fn get_or_create_watcher(&self, query: &Query) -> broadcast::Receiver<Event>;
    fn subscribe<T: models::Event + 'static>(
        &self,
        query: &Query,
    ) -> Result<mpsc::Receiver<T>, Error> {
        let mut event_rx = self.get_or_create_watcher(query);
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            tracing::info!("subscription forwarding started");
            while let Ok(event) = event_rx.recv().await {
                tracing::info!(?event.sequence, "subscribe_typed");
                match serde_json::from_value::<T>(event.data) {
                    Ok(event) => {
                        //let _ = tx.send(event).await;
                        tracing::info!("forwarding wait");
                        if let Err(err) = tx.send(event).await {
                            tracing::error!("Failed to forward event: {}", err);
                        }
                        tracing::info!("forwarding done");
                    }
                    Err(err) => {
                        tracing::error!("Failed to deserialize event: {}", err);
                    }
                }
            }
            tracing::info!("subscription forwarding stopped");
        });
        Ok(rx)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(sqlx::Error),
    #[error("Serialization error: {0}")]
    Serialization(serde_json::Error),
}
