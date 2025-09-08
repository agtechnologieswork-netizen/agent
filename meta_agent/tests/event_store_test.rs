use meta_agent::events::{EventMetadata, InMemoryEventStore, PersistedEvent, EventStore};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
enum Evt { X(u32), Y(&'static str) }

#[test]
fn event_store_roundtrip() {
    let mut store: InMemoryEventStore<Evt> = InMemoryEventStore::new();

    let e = PersistedEvent {
        meta: EventMetadata {
            id: "e1".into(),
            aggregate_id: "p1".into(),
            timestamp: 123,
            causation_id: None,
            correlation_id: None,
            version: 1,
        },
        payload: Evt::X(7),
    };

    store.append(e.clone());
    let all = store.load_all();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].payload, e.payload);
}


