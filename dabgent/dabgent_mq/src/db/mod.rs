pub mod postgres;
pub mod sqlite;
use crate::models::{self};
use chrono::{DateTime, Utc};
use eyre::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

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

pub trait EventStore: Clone + Send + Sync + 'static {
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

    fn poll_new_events(
        &self,
        query: &Query,
        last_sequence: i64,
    ) -> impl Future<Output = Result<Vec<Event>, Error>> + Send;

    fn get_watchers(&self) -> &Arc<Mutex<HashMap<Query, Vec<mpsc::UnboundedSender<Event>>>>>;

    fn subscribe<T: models::Event + 'static>(
        &self,
        query: &Query,
    ) -> Result<EventStream<T>, Error> {
        let mut watchers = self.get_watchers().lock().unwrap();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let senders = watchers.entry(query.clone()).or_insert_with(|| {
            let store = self.clone();
            let query_clone = query.clone();
            let watchers_arc = self.get_watchers().clone();

            tokio::spawn(async move {
                tracing::info!(?query_clone, "watcher started");
                let mut last_seen = 0i64;
                const POLL_INTERVAL: Duration = Duration::from_millis(500);

                'main: loop {
                    match store.poll_new_events(&query_clone, last_seen).await {
                        Ok(events) => {
                            for event in events {
                                let sequence = event.sequence;

                                let mut watchers = watchers_arc.lock().unwrap();
                                if let Some(senders) = watchers.get_mut(&query_clone) {
                                    senders.retain(|sender| sender.send(event.clone()).is_ok());

                                    if senders.is_empty() {
                                        watchers.remove(&query_clone);
                                        break 'main;
                                    }
                                } else {
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
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
                tracing::info!(?query_clone, "watcher stopped");
            });

            Vec::new()
        });

        senders.push(event_tx);

        Ok(EventStream::new(event_rx))
    }
}

pub struct EventStream<T: models::Event> {
    rx: mpsc::UnboundedReceiver<Event>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: models::Event> EventStream<T> {
    pub fn new(rx: mpsc::UnboundedReceiver<Event>) -> Self {
        Self {
            rx,
            _marker: std::marker::PhantomData,
        }
    }

    pub async fn next(&mut self) -> Option<Result<T, Error>> {
        match self.rx.recv().await {
            Some(event) => match serde_json::from_value::<T>(event.data) {
                Ok(typed_event) => Some(Ok(typed_event)),
                Err(err) => Some(Err(Error::Serialization(err))),
            },
            None => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(sqlx::Error),
    #[error("Serialization error: {0}")]
    Serialization(serde_json::Error),
}
