use serde::{Deserialize, Serialize};

/// Envelope metadata for persisted events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub id: String,
    pub aggregate_id: String,
    pub timestamp: u64,
    pub causation_id: Option<String>,
    pub correlation_id: Option<String>,
    pub version: u8,
}

/// Persisted event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedEvent<T> {
    pub meta: EventMetadata,
    pub payload: T,
}

/// Simple event store trait
pub trait EventStore<T: Clone + Send + Sync + 'static> {
    fn append(&mut self, event: PersistedEvent<T>);
    fn load_all(&self) -> Vec<PersistedEvent<T>>;
}

/// In-memory event store for testing and development
#[derive(Default)]
pub struct InMemoryEventStore<T: Clone> {
    events: Vec<PersistedEvent<T>>,
}

impl<T: Clone + Send + Sync + 'static> InMemoryEventStore<T> {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }
}

impl<T: Clone + Send + Sync + 'static> EventStore<T> for InMemoryEventStore<T> {
    fn append(&mut self, event: PersistedEvent<T>) {
        self.events.push(event);
    }

    fn load_all(&self) -> Vec<PersistedEvent<T>> {
        self.events.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    enum TestEvent { A(u32), B(String) }

    #[test]
    fn in_memory_event_store_roundtrip() {
        let mut store: InMemoryEventStore<TestEvent> = InMemoryEventStore::new();

        let e1 = PersistedEvent {
            meta: EventMetadata {
                id: "1".into(),
                aggregate_id: "agg".into(),
                timestamp: 1,
                causation_id: None,
                correlation_id: None,
                version: 1,
            },
            payload: TestEvent::A(42),
        };

        store.append(e1.clone());
        let all = store.load_all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].payload, e1.payload);
    }
}


