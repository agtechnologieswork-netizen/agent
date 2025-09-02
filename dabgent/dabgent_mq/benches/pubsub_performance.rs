use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use dabgent_mq::db::{EventStore, Metadata, Query, sqlite::SqliteStore};
use dabgent_mq::models::Event;
use serde::{Deserialize, Serialize};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::Runtime;
use tokio::sync::{Barrier, mpsc};
use tokio::time::{Duration, timeout};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct SmallEvent {
    id: u64,
    value: i32,
}

impl Event for SmallEvent {
    const EVENT_VERSION: &'static str = "1.0";
    fn event_type() -> &'static str {
        "SmallEvent"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct MediumEvent {
    id: u64,
    name: String,
    data: Vec<u8>,
    metadata: std::collections::HashMap<String, String>,
}

impl Event for MediumEvent {
    const EVENT_VERSION: &'static str = "1.0";
    fn event_type() -> &'static str {
        "MediumEvent"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct LargeEvent {
    id: u64,
    payload: String,
    extra_data: Vec<u8>,
}

impl Event for LargeEvent {
    const EVENT_VERSION: &'static str = "1.0";
    fn event_type() -> &'static str {
        "LargeEvent"
    }
}

async fn setup_test_store() -> SqliteStore {
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

fn create_small_event(id: u64) -> SmallEvent {
    SmallEvent {
        id,
        value: id as i32,
    }
}

fn create_medium_event(id: u64) -> MediumEvent {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("key1".to_string(), format!("value_{}", id));
    metadata.insert("key2".to_string(), "fixed_value".to_string());

    MediumEvent {
        id,
        name: format!("event_{}", id),
        data: vec![id as u8; 100], // 100 bytes of data
        metadata,
    }
}

fn create_large_event(id: u64) -> LargeEvent {
    LargeEvent {
        id,
        payload: "x".repeat(1000),       // 1KB payload
        extra_data: vec![id as u8; 500], // 500 bytes extra
    }
}

fn bench_single_producer_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("single_producer_throughput");

    for event_count in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*event_count as u64));

        group.bench_with_input(
            BenchmarkId::new("small_events", event_count),
            event_count,
            |b, &event_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let store = setup_test_store().await;
                        let metadata = Metadata::default();

                        for i in 0..event_count {
                            let event = create_small_event(i as u64);
                            store
                                .push_event(
                                    "bench-stream",
                                    &format!("agg-{}", i),
                                    &event,
                                    &metadata,
                                )
                                .await
                                .unwrap();
                        }
                    })
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("medium_events", event_count),
            event_count,
            |b, &event_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let store = setup_test_store().await;
                        let metadata = Metadata::default();

                        for i in 0..event_count {
                            let event = create_medium_event(i as u64);
                            store
                                .push_event(
                                    "bench-stream",
                                    &format!("agg-{}", i),
                                    &event,
                                    &metadata,
                                )
                                .await
                                .unwrap();
                        }
                    })
                })
            },
        );
    }

    group.finish();
}

fn bench_concurrent_producers(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_producers");

    for producer_count in [2, 4, 8, 16].iter() {
        let events_per_producer = 250;
        let total_events = producer_count * events_per_producer;

        group.throughput(Throughput::Elements(total_events as u64));

        group.bench_with_input(
            BenchmarkId::new("concurrent_small_events", producer_count),
            producer_count,
            |b, &producer_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let store = Arc::new(setup_test_store().await);
                        let barrier = Arc::new(Barrier::new(producer_count + 1));

                        let mut handles = vec![];

                        for producer_id in 0..producer_count {
                            let store = store.clone();
                            let barrier = barrier.clone();

                            let handle = tokio::spawn(async move {
                                barrier.wait().await;

                                let metadata = Metadata::default();
                                for i in 0..events_per_producer {
                                    let event_id = (producer_id * events_per_producer + i) as u64;
                                    let event = create_small_event(event_id);
                                    store
                                        .push_event(
                                            "bench-stream",
                                            &format!("agg-{}-{}", producer_id, i),
                                            &event,
                                            &metadata,
                                        )
                                        .await
                                        .unwrap();
                                }
                            });
                            handles.push(handle);
                        }

                        barrier.wait().await; // Start all producers

                        for handle in handles {
                            handle.await.unwrap();
                        }
                    })
                })
            },
        );
    }

    group.finish();
}

fn bench_consumer_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("consumer_latency");

    group.bench_function("single_event_latency", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = setup_test_store().await;
                let query = Query {
                    stream_id: "latency-stream".to_string(),
                    event_type: None,
                    aggregate_id: None,
                };

                let mut receiver = store.subscribe::<SmallEvent>(&query).unwrap();
                let metadata = Metadata::default();

                let start = Instant::now();

                let event = create_small_event(1);
                store
                    .push_event("latency-stream", "agg-1", &event, &metadata)
                    .await
                    .unwrap();

                let received = timeout(Duration::from_secs(5), receiver.recv())
                    .await
                    .expect("Timeout waiting for event")
                    .expect("Failed to receive event");

                let latency = start.elapsed();
                assert_eq!(received, event);

                black_box(latency);
            })
        })
    });

    group.finish();
}

fn bench_multiple_consumers(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("multiple_consumers");

    for consumer_count in [2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_consumers", consumer_count),
            consumer_count,
            |b, &consumer_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let store = Arc::new(setup_test_store().await);
                        let query = Query {
                            stream_id: "multi-consumer-stream".to_string(),
                            event_type: None,
                            aggregate_id: None,
                        };

                        let mut handles = vec![];
                        let (tx, mut rx) = mpsc::channel(consumer_count);

                        // Start consumers
                        for consumer_id in 0..consumer_count {
                            let store = store.clone();
                            let query = query.clone();
                            let tx = tx.clone();

                            let handle = tokio::spawn(async move {
                                let mut receiver = store.subscribe::<SmallEvent>(&query).unwrap();

                                if let Ok(Some(event)) =
                                    timeout(Duration::from_secs(5), receiver.recv()).await
                                {
                                    tx.send((consumer_id, event)).await.ok();
                                }
                            });
                            handles.push(handle);
                        }

                        drop(tx); // Close sender

                        // Give consumers time to set up subscriptions
                        tokio::time::sleep(Duration::from_millis(100)).await;

                        // Send an event
                        let metadata = Metadata::default();
                        let event = create_small_event(42);
                        store
                            .push_event("multi-consumer-stream", "agg-42", &event, &metadata)
                            .await
                            .unwrap();

                        // Wait for consumers to receive
                        let mut received_count = 0;
                        while let Some(_) = timeout(Duration::from_secs(2), rx.recv())
                            .await
                            .ok()
                            .flatten()
                        {
                            received_count += 1;
                            if received_count >= consumer_count {
                                break;
                            }
                        }

                        for handle in handles {
                            let _ = handle.await;
                        }

                        assert_eq!(received_count, consumer_count);
                    })
                })
            },
        );
    }

    group.finish();
}

fn bench_producer_consumer_mixed(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("producer_consumer_mixed");

    group.bench_function("1_producer_2_consumers", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = Arc::new(setup_test_store().await);
                let query = Query {
                    stream_id: "mixed-stream".to_string(),
                    event_type: None,
                    aggregate_id: None,
                };

                let event_count = 100;
                let (tx, mut rx) = mpsc::channel(10);

                // Start consumers
                let mut consumer_handles = vec![];
                for consumer_id in 0..2 {
                    let store = store.clone();
                    let query = query.clone();
                    let tx = tx.clone();

                    let handle = tokio::spawn(async move {
                        let mut receiver = store.subscribe::<SmallEvent>(&query).unwrap();
                        let mut count = 0;

                        while let Ok(Some(_event)) =
                            timeout(Duration::from_secs(5), receiver.recv()).await
                        {
                            count += 1;
                            if count >= event_count {
                                break;
                            }
                        }

                        tx.send(count).await.ok();
                    });
                    consumer_handles.push(handle);
                }

                drop(tx);

                // Give consumers time to set up
                tokio::time::sleep(Duration::from_millis(100)).await;

                // Start producer
                let producer_handle = {
                    let store = store.clone();
                    tokio::spawn(async move {
                        let metadata = Metadata::default();
                        for i in 0..event_count {
                            let event = create_small_event(i as u64);
                            store
                                .push_event(
                                    "mixed-stream",
                                    &format!("agg-{}", i),
                                    &event,
                                    &metadata,
                                )
                                .await
                                .unwrap();

                            // Small delay to simulate real workload
                            if i % 10 == 0 {
                                tokio::time::sleep(Duration::from_micros(100)).await;
                            }
                        }
                    })
                };

                // Wait for completion
                producer_handle.await.unwrap();

                let mut total_received = 0;
                while let Some(count) = timeout(Duration::from_secs(3), rx.recv())
                    .await
                    .ok()
                    .flatten()
                {
                    total_received += count;
                    if total_received >= event_count * 2 {
                        break;
                    }
                }

                for handle in consumer_handles {
                    let _ = handle.await;
                }

                black_box(total_received);
            })
        })
    });

    group.finish();
}

fn bench_batch_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("batch_operations");

    for batch_size in [1, 10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("batch_push_events", batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    rt.block_on(async {
                        let store = setup_test_store().await;
                        let metadata = Metadata::default();

                        // Simulate batch by pushing events rapidly in sequence
                        for i in 0..batch_size {
                            let event = create_small_event(i as u64);
                            store
                                .push_event(
                                    "batch-stream",
                                    &format!("agg-{}", i),
                                    &event,
                                    &metadata,
                                )
                                .await
                                .unwrap();
                        }
                    })
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_single_producer_throughput,
    bench_concurrent_producers,
    bench_consumer_latency,
    bench_multiple_consumers,
    bench_producer_consumer_mixed,
    bench_batch_operations
);

criterion_main!(benches);
