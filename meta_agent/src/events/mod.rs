use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventMetadata {
    pub id: String,
    pub aggregate_id: String,
    pub timestamp: u64,
    pub causation_id: Option<String>,
    pub correlation_id: Option<String>,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedEvent<T> {
    pub meta: EventMetadata,
    pub payload: T,
}

pub trait EventStore<T> {
    fn append(&mut self, event: PersistedEvent<T>);
    fn load_all(&self) -> Vec<PersistedEvent<T>>;
}

#[derive(Default)]
pub struct InMemoryEventStore<T> {
    events: Vec<PersistedEvent<T>>,
}

impl<T: Clone> InMemoryEventStore<T> {
    pub fn new() -> Self { Self { events: Vec::new() } }
}

impl<T: Clone> EventStore<T> for InMemoryEventStore<T> {
    fn append(&mut self, event: PersistedEvent<T>) {
        self.events.push(event);
    }

    fn load_all(&self) -> Vec<PersistedEvent<T>> {
        self.events.clone()
    }
}
