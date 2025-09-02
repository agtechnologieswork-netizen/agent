# DabGent MQ

A lean event sourcing library for Rust, supporting both PostgreSQL and SQLite backends with real-time event streaming capabilities.

## Features

- **Dual Database Support**: PostgreSQL and SQLite implementations
- **Event Sourcing**: Store and replay events with full audit trails
- **Real-time Subscriptions**: Subscribe to event streams with automatic polling
- **Type Safety**: Strongly typed events with compile-time guarantees
- **Metadata Support**: Rich event metadata with correlation and causation tracking
- **Concurrent Safe**: Built with Tokio for async/await and thread safety

## Quick Start

### 1. Define Your Events

```rust
use dabgent_mq::models::Event;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UserCreated {
    pub user_id: String,
    pub email: String,
    pub name: String,
}

impl Event for UserCreated {
    const EVENT_VERSION: &'static str = "1.0";

    fn event_type() -> &'static str {
        "UserCreated"
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserUpdated {
    pub user_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

impl Event for UserUpdated {
    const EVENT_VERSION: &'static str = "1.0";

    fn event_type() -> &'static str {
        "UserUpdated"
    }
}
```

### 2. Setup Database Connection

#### PostgreSQL

```rust
use dabgent_mq::db::{postgres::PostgresStore, EventStore};
use sqlx::PgPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPool::connect("postgresql://user:pass@localhost/events").await?;
    let store = PostgresStore::new(pool);

    // Run migrations
    store.migrate().await;

    // Use the store...
    Ok(())
}
```

#### SQLite

```rust
use dabgent_mq::db::{sqlite::SqliteStore, EventStore};
use sqlx::SqlitePool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = SqlitePool::connect("sqlite:events.db").await?;
    let store = SqliteStore::new(pool);

    // Run migrations
    store.migrate().await;

    // Use the store...
    Ok(())
}
```

### 3. Store Events

```rust
use dabgent_mq::db::Metadata;
use uuid::Uuid;

// Create metadata
let metadata = Metadata::default()
    .with_correlation_id(Uuid::new_v4())
    .with_causation_id(Uuid::new_v4());

// Create and store an event
let event = UserCreated {
    user_id: "user-123".to_string(),
    email: "user@example.com".to_string(),
    name: "John Doe".to_string(),
};

store.push_event(
    "user-stream",      // stream_id
    "user-123",         // aggregate_id
    &event,
    &metadata,
).await?;
```

### 4. Load Events

```rust
use dabgent_mq::db::Query;

// Query for all events in a stream
let query = Query {
    stream_id: "user-stream".to_string(),
    event_type: None,
    aggregate_id: None,
};

let events: Vec<UserCreated> = store.load_events(&query, None).await?;

// Query for specific event type
let query = Query {
    stream_id: "user-stream".to_string(),
    event_type: Some("UserCreated".to_string()),
    aggregate_id: Some("user-123".to_string()),
};

let user_events: Vec<UserCreated> = store.load_events(&query, None).await?;
```

### 5. Real-time Subscriptions

```rust
// Subscribe to events in real-time
let query = Query {
    stream_id: "user-stream".to_string(),
    event_type: Some("UserCreated".to_string()),
    aggregate_id: None,
};

let mut subscription = store.subscribe::<UserCreated>(&query)?;

// Process events as they arrive
tokio::spawn(async move {
    while let Some(event) = subscription.next().await {
        match event {
            Ok(e) => println!("Received event: {:?}", e),
            Err(err) => eprintln!("Error receiving event: {:?}", err),
        }
    }
});
```

## Advanced Usage

### Event Replay and Projections

```rust
// Load all events for an aggregate and build a projection
let query = Query {
    stream_id: "my-stream".to_string(),
    event_type: None,
    aggregate_id: Some("entity-123".to_string()),
};

// Load events for projection
let created_query = Query {
    stream_id: "user-stream".to_string(),
    event_type: Some("UserCreated".to_string()),
    aggregate_id: Some("entity-123".to_string()),
};

let updated_query = Query {
    stream_id: "user-stream".to_string(),
    event_type: Some("UserUpdated".to_string()),
    aggregate_id: Some("entity-123".to_string()),
};

let created_events: Vec<UserCreated> = store.load_events(&created_query, None).await?;
let updated_events: Vec<UserUpdated> = store.load_events(&updated_query, None).await?;

// Build user projection
let mut user_projection = UserProjection::new();
for event in created_events {
    user_projection.apply_created(event);
}
for event in updated_events {
    user_projection.apply_updated(event);
}
```

### Multiple Subscribers

```rust
// Multiple processes can subscribe to the same stream
let query = Query {
    stream_id: "user-stream".to_string(),
    event_type: None,
    aggregate_id: None,
};

// Subscriber 1: Audit log
let mut audit_sub = store.subscribe::<UserCreated>(&query)?;
tokio::spawn(async move {
    while let Some(event) = audit_sub.next().await {
        if let Ok(e) = event {
            log_to_audit_system(e).await;
        }
    }
});

// Subscriber 2: Email notifications
let mut email_sub = store.subscribe::<UserCreated>(&query)?;
tokio::spawn(async move {
    while let Some(event) = email_sub.next().await {
        if let Ok(e) = event {
            send_welcome_email(e.email).await;
        }
    }
});
```

### Custom Metadata

```rust
use serde_json::json;

let metadata = Metadata::new(
    Some(Uuid::new_v4()),  // correlation_id
    Some(Uuid::new_v4()),  // causation_id
    Some(json!({           // custom metadata
        "user_agent": "MyApp/1.0",
        "ip_address": "192.168.1.1",
        "trace_id": "abc123"
    }))
);

store.push_event("user-stream", "user-123", &event, &metadata).await?;
```

## Database Schema

The library automatically manages database migrations. The events table structure:

```sql
CREATE TABLE events (
    stream_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    sequence BIGINT NOT NULL,
    event_version TEXT NOT NULL,
    data JSONB NOT NULL,        -- JSON in SQLite
    metadata JSONB NOT NULL,    -- JSON in SQLite
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (stream_id, event_type, aggregate_id, sequence)
);
```

## Performance Considerations

- Events are automatically assigned sequential numbers within each stream
- Indexes are created for efficient querying by stream, event type, aggregate, and timestamp
- Real-time subscriptions use polling with intervals

## Benchmarks

The library includes comprehensive benchmarks for measuring throughput under various configurations.

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench
```

### Benchmark Scenarios

The benchmarks test various producer/consumer configurations with different payload sizes:

**Payload Sizes:**
- 1KB - Small messages
- 4KB - Typical messages
- 256KB - Large messages
- 512KB - Very large messages
- 1MB - Maximum size messages

**Configurations:**
- `1p_1c` - 1 producer, 1 consumer (baseline)
- `1p_2c` - 1 producer, 2 consumers (fan-out)
- `2p_2c` - 2 producers, 2 consumers (balanced)
- `4p_4c` - 4 producers, 4 consumers (high concurrency)
- `1p_4c` - 1 producer, 4 consumers (heavy fan-out)
- `4p_1c` - 4 producers, 1 consumer (many-to-one)

**Throughput Tests:**
- Write throughput - Sequential writes of 1000 messages
- Read throughput - Sequential reads of 1000 messages
- High contention - 10 concurrent producers writing to the same stream

### Interpreting Results

Benchmark output shows:
- **Time**: Duration per iteration
- **Throughput**: Messages per second (elem/s)

Example output:
```
pubsub/1kb_1p_1c/1024   time:   [109.49 ms 111.15 ms 111.57 ms]
                        thrpt:  [896.33 elem/s 899.68 elem/s 913.34 elem/s]
```

This indicates ~900 messages per second for 1KB payloads with 1 producer and 1 consumer.
