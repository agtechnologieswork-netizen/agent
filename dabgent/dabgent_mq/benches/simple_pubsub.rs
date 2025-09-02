use criterion::{Criterion, black_box, criterion_group, criterion_main};
use dabgent_mq::db::{EventStore, Metadata, Query, sqlite::SqliteStore};
use dabgent_mq::models::Event;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .build()
        .unwrap()
}

async fn store() -> SqliteStore {
    let opts = SqliteConnectOptions::from_str(":memory:")
        .unwrap()
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePool::connect_with(opts)
        .await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct BenchEvent {
    id: u64,
    payload: String,
}

impl Event for BenchEvent {
    const EVENT_VERSION: &'static str = "1.0";
    fn event_type() -> &'static str {
        "BenchEvent"
    }
}

fn small_event() -> BenchEvent {
    BenchEvent {
        id: 0,
        payload: "Small Event".to_string(),
    }
}

fn bench_sequential_throughput(c: &mut Criterion) {
    let rt = rt();

    c.bench_function("sequential_1000_events", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = store().await;
                let metadata = Metadata::default();

                for i in 0..1000 {
                    let event = BenchEvent {
                        id: i,
                        payload: format!("Event {}", i),
                    };
                    store
                        .push_event("stream", &format!("agg-{}", i), &event, &metadata)
                        .await
                        .unwrap();
                }
            })
        })
    });
}

fn bench_pub_sub_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("pub_sub_latency", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = store().await;
                let query = Query {
                    stream_id: "test-stream".to_string(),
                    event_type: None,
                    aggregate_id: None,
                };

                // Start subscriber
                let mut receiver = store.subscribe::<BenchEvent>(&query).unwrap();

                // Measure latency
                let start = Instant::now();

                // Publish event
                let event = BenchEvent {
                    id: 1,
                    payload: "Test".to_string(),
                };
                let metadata = Metadata::default();
                store
                    .push_event("test-stream", "agg-1", &event, &metadata)
                    .await
                    .unwrap();

                // Wait for event
                let received = tokio::time::timeout(Duration::from_secs(5), receiver.recv())
                    .await
                    .expect("Timeout")
                    .expect("Failed to receive");

                let latency = start.elapsed();
                assert_eq!(received, event);
                black_box(latency)
            })
        })
    });
}

fn bench_concurrent_subscribers(c: &mut Criterion) {
    let rt = rt();

    c.bench_function("5_concurrent_subscribers", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = store().await;
                let query = Query {
                    stream_id: "broadcast-stream".to_string(),
                    event_type: None,
                    aggregate_id: None,
                };

                // Create 5 subscribers
                let mut receivers = vec![];
                for _ in 0..5 {
                    receivers.push(store.subscribe::<BenchEvent>(&query).unwrap());
                }

                // Publish 100 events
                let metadata = Metadata::default();
                for i in 0..100 {
                    let event = BenchEvent {
                        id: i,
                        payload: format!("Event {}", i),
                    };
                    store
                        .push_event("broadcast-stream", &format!("agg-{}", i), &event, &metadata)
                        .await
                        .unwrap();
                }

                // Each subscriber should receive all 100 events
                for mut receiver in receivers {
                    let mut count = 0;
                    while let Ok(Some(_)) =
                        tokio::time::timeout(Duration::from_secs(1), receiver.recv()).await
                    {
                        count += 1;
                        if count >= 100 {
                            break;
                        }
                    }
                    black_box(count);
                }
            })
        })
    });
}

fn bench_load_events(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("load_1000_events", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = store().await;
                let metadata = Metadata::default();

                // First, push 1000 events
                for i in 0..1000 {
                    let event = BenchEvent {
                        id: i,
                        payload: format!("Event {}", i),
                    };
                    store
                        .push_event("load-stream", &format!("agg-{}", i), &event, &metadata)
                        .await
                        .unwrap();
                }

                // Now benchmark loading them
                let query = Query {
                    stream_id: "load-stream".to_string(),
                    event_type: None,
                    aggregate_id: None,
                };

                let start = Instant::now();
                let events: Vec<BenchEvent> = store.load_events(&query).await.unwrap();
                let duration = start.elapsed();

                assert_eq!(events.len(), 1000);
                black_box(duration)
            })
        })
    });
}

criterion_group!(
    benches,
    bench_sequential_throughput,
    bench_pub_sub_latency,
    bench_concurrent_subscribers,
    bench_load_events
);

criterion_main!(benches);
