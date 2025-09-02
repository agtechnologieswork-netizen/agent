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

let events: Vec<UserCreated> = store.load_events(&query).await?;

// Query for specific event type
let query = Query {
    stream_id: "user-stream".to_string(),
    event_type: Some("UserCreated".to_string()),
    aggregate_id: Some("user-123".to_string()),
};

let user_events: Vec<UserCreated> = store.load_events(&query).await?;
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
    while let Some(event) = subscription.recv().await {
        println!("Received event: {:?}", event);
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

// Load different event types
let events: Vec<MyEvent> = store.load_events(&query).await?;

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
    while let Some(event) = audit_sub.recv().await {
        log_to_audit_system(event).await;
    }
});

// Subscriber 2: Email notifications
let mut email_sub = store.subscribe::<UserCreated>(&query)?;
tokio::spawn(async move {
    while let Some(event) = email_sub.recv().await {
        send_welcome_email(event.email).await;
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
