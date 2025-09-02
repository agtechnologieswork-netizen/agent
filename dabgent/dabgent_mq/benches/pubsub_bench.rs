use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use dabgent_mq::db::{EventStore, Metadata, Query, sqlite::SqliteStore};
use dabgent_mq::models::Event;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Barrier;

const STREAM_ID: &str = "bench_stream";
const AGGREGATE_ID: &str = "bench_aggregate";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct BenchEvent {
    id: u64,
    payload: Vec<u8>,
}

impl Event for BenchEvent {
    const EVENT_VERSION: &'static str = "1.0";
    fn event_type() -> &'static str {
        "BenchEvent"
    }
}

fn create_payload(size: usize) -> Vec<u8> {
    vec![0u8; size]
}

fn create_event(payload_size: usize) -> BenchEvent {
    BenchEvent {
        id: 1,
        payload: create_payload(payload_size),
    }
}

async fn create_store() -> SqliteStore {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}

fn bench_pubsub(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("pubsub");
    group.sample_size(10);

    let payload_sizes = [
        ("1kb", 1024),
        ("4kb", 4 * 1024),
        ("256kb", 256 * 1024),
        ("512kb", 512 * 1024),
        ("1mb", 1024 * 1024),
    ];

    // name, num_producers, num_consumers, num_events
    let configurations = [
        ("1p_1c", 1, 1, 100),
        ("1p_2c", 1, 2, 100),
        ("2p_2c", 2, 2, 100),
        ("4p_4c", 4, 4, 100),
        ("1p_4c", 1, 4, 100),
        ("4p_1c", 4, 1, 100),
    ];

    for (size_name, payload_size) in &payload_sizes {
        for (config_name, num_producers, num_consumers, num_events) in &configurations {
            let bench_id = BenchmarkId::new(format!("{}_{}", size_name, config_name), payload_size);

            let total_messages = num_producers * num_events;
            group.throughput(criterion::Throughput::Elements(total_messages as u64));

            group.bench_function(bench_id, |b| {
                b.to_async(&rt).iter(|| async {
                    benchmark_scenario(*num_producers, *num_consumers, *num_events, *payload_size)
                        .await
                });
            });
        }
    }

    group.finish();
}

async fn benchmark_scenario(
    num_producers: usize,
    num_consumers: usize,
    num_events: usize,
    payload_size: usize,
) {
    let store = Arc::new(create_store().await);
    let barrier = Arc::new(Barrier::new(num_producers + num_consumers + 1));

    let event = create_event(payload_size);
    let metadata = Metadata::default();

    let mut producer_handles = Vec::new();
    for producer_id in 0..num_producers {
        let store = store.clone();
        let barrier = barrier.clone();
        let event = event.clone();
        let metadata = metadata.clone();

        let handle = tokio::spawn(async move {
            barrier.wait().await;

            for event_id in 0..num_events {
                let mut event = event.clone();
                event.id = (producer_id * num_events + event_id) as u64;

                store
                    .push_event(STREAM_ID, AGGREGATE_ID, &event, &metadata)
                    .await
                    .expect("Failed to push event");
            }
        });

        producer_handles.push(handle);
    }

    let mut consumer_handles = Vec::new();
    for _ in 0..num_consumers {
        let store = store.clone();
        let barrier = barrier.clone();

        let handle = tokio::spawn(async move {
            let query = Query {
                stream_id: STREAM_ID.to_string(),
                event_type: None,
                aggregate_id: Some(AGGREGATE_ID.to_string()),
            };

            let mut stream = store.subscribe::<BenchEvent>(&query).unwrap();

            barrier.wait().await;

            let expected_events = num_producers * num_events;
            let mut received = 0;

            while received < expected_events {
                if let Some(event) = stream.next().await {
                    let _ = black_box(event);
                    received += 1;
                } else {
                    break;
                }
            }
        });

        consumer_handles.push(handle);
    }

    barrier.wait().await;

    for handle in producer_handles {
        handle.await.expect("Producer task failed");
    }

    for handle in consumer_handles {
        handle.abort();
    }
}

fn bench_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("throughput");
    group.sample_size(10);

    // Test pure write throughput - 1000 messages
    group.throughput(criterion::Throughput::Elements(1000));
    group.bench_function("write_1kb_x1000", |b| {
        b.to_async(&rt).iter(|| async {
            let store = create_store().await;
            let event = create_event(1024);
            let metadata = Metadata::default();

            for i in 0..1000 {
                let mut event = event.clone();
                event.id = i;
                store
                    .push_event(STREAM_ID, AGGREGATE_ID, &event, &metadata)
                    .await
                    .expect("Failed to push event");
            }
        });
    });

    // Test pure read throughput - 1000 messages
    group.throughput(criterion::Throughput::Elements(1000));
    group.bench_function("read_1kb_x1000", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let mut total_duration = std::time::Duration::ZERO;

            for _ in 0..iters {
                let store = create_store().await;
                let event = create_event(1024);
                let metadata = Metadata::default();

                for i in 0..1000 {
                    let mut event = event.clone();
                    event.id = i;
                    store
                        .push_event(STREAM_ID, AGGREGATE_ID, &event, &metadata)
                        .await
                        .expect("Failed to push event");
                }

                // Benchmark: read all events
                let start = std::time::Instant::now();

                let query = Query {
                    stream_id: STREAM_ID.to_string(),
                    event_type: None,
                    aggregate_id: Some(AGGREGATE_ID.to_string()),
                };

                let mut stream = store.subscribe::<BenchEvent>(&query).unwrap();
                let mut count = 0;

                while let Some(event) = stream.next().await {
                    let _ = black_box(event);
                    count += 1;
                    if count >= 1000 {
                        break;
                    }
                }

                total_duration += start.elapsed();
            }

            total_duration
        });
    });

    group.finish();
}

fn bench_contention(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("contention");
    group.sample_size(10);

    // High contention scenario: many producers writing to same stream
    // 10 producers * 100 events = 1000 total messages
    group.throughput(criterion::Throughput::Elements(1000));
    group.bench_function("high_contention_10p_1kb", |b| {
        b.to_async(&rt).iter(|| async {
            let store = Arc::new(create_store().await);
            let num_producers = 10;
            let events_per_producer = 100;

            let mut handles = Vec::new();

            for producer_id in 0..num_producers {
                let store = store.clone();
                let handle = tokio::spawn(async move {
                    let event = create_event(1024);
                    let metadata = Metadata::default();

                    for event_id in 0..events_per_producer {
                        let mut event = event.clone();
                        event.id = (producer_id * events_per_producer + event_id) as u64;

                        store
                            .push_event(STREAM_ID, AGGREGATE_ID, &event, &metadata)
                            .await
                            .expect("Failed to push event");
                    }
                });

                handles.push(handle);
            }

            for handle in handles {
                handle.await.expect("Producer task failed");
            }
        });
    });

    group.finish();
}

criterion_group!(benches, bench_pubsub, bench_throughput, bench_contention);
criterion_main!(benches);
