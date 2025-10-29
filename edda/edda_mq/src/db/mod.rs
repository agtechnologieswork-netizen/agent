pub mod postgres;
pub mod sqlite;
use crate::{Aggregate, AggregateContext, Envelope, Event, Metadata};
use eyre::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SerializedEvent {
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub sequence: i64,
    pub event_type: String,
    pub event_version: String,
    pub data: JsonValue,
    pub metadata: JsonValue,
}

impl<A: Aggregate> TryFrom<&Envelope<A>> for SerializedEvent {
    type Error = Error;

    fn try_from(value: &Envelope<A>) -> Result<Self, Self::Error> {
        let aggregate_type = A::TYPE.to_string();
        let event_type = value.data.event_type();
        let event_version = value.data.event_version();
        let data = serde_json::to_value(&value.data)?;
        let metadata = serde_json::to_value(&value.metadata)?;
        Ok(Self {
            aggregate_id: value.aggregate_id.clone(),
            sequence: value.sequence,
            aggregate_type,
            event_type,
            event_version,
            data,
            metadata,
        })
    }
}

impl<A: Aggregate> TryFrom<SerializedEvent> for Envelope<A> {
    type Error = Error;

    fn try_from(value: SerializedEvent) -> Result<Self, Self::Error> {
        let data = serde_json::from_value(value.data)?;
        let metadata = serde_json::from_value(value.metadata)?;
        Ok(Self {
            aggregate_id: value.aggregate_id,
            sequence: value.sequence,
            data,
            metadata,
        })
    }
}

pub trait EventStore: Clone + Send + Sync + 'static {
    fn commit<A: Aggregate>(
        &self,
        events: Vec<A::Event>,
        metadata: Metadata,
        context: AggregateContext<A>,
    ) -> impl Future<Output = Result<Vec<Envelope<A>>, Error>> + Send;

    fn load_events<A: Aggregate>(
        &self,
        aggregate_id: &str,
    ) -> impl Future<Output = Result<Vec<Envelope<A>>, Error>> + Send;

    fn load_latest_events<A: Aggregate>(
        &self,
        aggregate_id: &str,
        sequence_from: i64,
    ) -> impl Future<Output = Result<Vec<Envelope<A>>, Error>> + Send;

    fn load_aggregate<A: Aggregate>(
        &self,
        aggregate_id: &str,
    ) -> impl Future<Output = Result<AggregateContext<A>, Error>> + Send;

    fn load_sequence_nums<A: Aggregate>(
        &self,
    ) -> impl Future<Output = Result<Vec<(String, i64)>, Error>> + Send;
}

pub fn wrap_events<A: Aggregate>(
    aggregate_id: &str,
    current_sequence: i64,
    events: Vec<A::Event>,
    metadata: Metadata,
) -> Vec<Envelope<A>> {
    let mut sequence = current_sequence;
    events
        .into_iter()
        .map(|data| {
            sequence += 1;
            Envelope {
                aggregate_id: aggregate_id.to_owned(),
                metadata: metadata.clone(),
                sequence,
                data,
            }
        })
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(sqlx::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
