# DabGent MQ - Efficient Message Queue for Rust

A high-performance, database-backed message queue implementation for Rust that seamlessly supports both SQLite and PostgreSQL. Designed for use as an in-process message bus with support for topics, streams, and JSON payloads up to 1MB in size.

## Features

- **Dual Database Support**: Seamlessly switch between SQLite and PostgreSQL
- **Topic-Based Routing**: Publish and subscribe to specific topics with wildcard support
- **Stream Processing**: Efficient message streaming with offset tracking
- **Priority Queues**: Support for message priorities
- **At-Least-Once Delivery**: Reliable message delivery with acknowledgment
- **Consumer Groups**: Multiple consumers with automatic load balancing
- **Large Payloads**: Support for JSON payloads up to 1MB
- **Async/Await**: Full async support with Tokio runtime
- **Performance Optimized**: 
  - Connection pooling
  - Prepared statements
  - Batch operations
  - Database-specific optimizations

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
dabgent_mq = "0.1.0"
```

## Quick Start

```rust
use dabgent_mq::{DatabaseType, MessageQueue};
use serde_json::json;

#[tokio::main]
async fn main() -> dabgent_mq::Result<()> {
    // Initialize with SQLite
    let queue = MessageQueue::new(
        DatabaseType::Sqlite,
        "sqlite://messages.db"
    ).await?;
    
    // Or with PostgreSQL
    // let queue = MessageQueue::new(
    //     DatabaseType::Postgres,
    //     "postgres://user:pass@localhost/mydb"
    // ).await?;
    
    // Create a stream
    queue.create_stream("orders").await?;
    
    // Publish a message
    let msg_id = queue.publish(
        "orders",
        "order.created",
        json!({
            "order_id": "12345",
            "amount": 99.99
        })
    ).await?;
    
    // Subscribe and consume
    let mut subscriber = queue.subscribe(
        "orders", 
        vec!["order.*".to_string()]
    ).await?;
    
    let messages = subscriber.poll().await?;
    for msg in messages {
        println!("Received: {:?}", msg);
        subscriber.ack(msg.id).await?;
    }
    
    Ok(())
}
```

## Architecture

### Database Schema

The system uses three main tables:

1. **messages**: Core message storage with indexing for efficient queries
   - Composite indexes on (stream_id, created_at, consumed_at)
   - Partial indexes for unconsumed messages
   - Priority-based ordering support

2. **streams**: Stream metadata and configuration
   - Tracks last offset for replay
   - Stores stream-specific configuration

3. **consumers**: Active consumer tracking
   - Heartbeat mechanism for liveness
   - Offset tracking per consumer

### Database-Specific Optimizations

**PostgreSQL:**
- `SKIP LOCKED` for concurrent consumer support
- `NOTIFY/LISTEN` for real-time updates
- JSONB for efficient JSON queries
- Native batch insert with `UNNEST`

**SQLite:**
- WAL mode for concurrent reads
- Row-level locking strategy
- Optimized transaction batching
- TEXT storage with JSON parsing

## Usage Examples

### Publishing with Priority

```rust
// High-priority message
queue.publish_with_priority(
    "notifications",
    "alert.critical",
    json!({"message": "System down!"}),
    10  // Higher priority
).await?;
```

### Batch Publishing

```rust
use dabgent_mq::Message;

let messages = vec![
    Message::new("events", "user.login", json!({"user_id": 1})),
    Message::new("events", "user.logout", json!({"user_id": 2})),
];

let ids = queue.publish_batch(messages).await?;
```

### Stream-Based Subscription

```rust
use futures::StreamExt;

let subscriber = queue.subscribe("events", vec!["user.*".to_string()]).await?;
let mut stream = subscriber.into_stream();

while let Some(message) = stream.next().await {
    process_message(message).await;
}
```

### Message Replay

```rust
// Replay from beginning
let mut replay = queue.stream("events")
    .from_offset(0)
    .batch_size(100)
    .build();

while let Some(msg) = replay.next().await {
    println!("Replaying: {:?}", msg);
}
```

### Topic Patterns

The queue supports flexible topic matching:

- `*` - Matches all topics
- `order.*` - Matches `order.created`, `order.updated`, etc.
- `*.created` - Matches `order.created`, `user.created`, etc.
- Exact matches: `order.created`

### Consumer Management

```rust
// Register custom consumer
use dabgent_mq::{Consumer, SubscribeOptions};

let options = SubscribeOptions::new("orders")
    .consumer_id("worker-1")
    .topics(vec!["order.created".to_string()])
    .batch_size(50)
    .from_offset(1000);

let subscriber = queue.subscribe_with_options(options).await?;

// Heartbeat to maintain consumer registration
subscriber.heartbeat().await?;

// Clean up expired consumers (> 60 seconds inactive)
let removed = queue.cleanup_expired_consumers(60).await?;
```

## Performance Considerations

### Message Size
- Maximum payload size: 1MB
- Large messages are validated before insertion
- Consider external storage for larger payloads

### Batching
- Use batch operations for bulk inserts
- Configure appropriate batch sizes (default: 100)
- Batch acknowledgments when possible

### Polling Strategy
- Long polling with configurable timeout (default: 30s)
- Automatic heartbeat during streaming
- Exponential backoff on errors

### Connection Pooling
- Default: 20 connections
- Configurable via pool options
- Automatic connection recycling

## Database Setup

### SQLite
```sql
-- Automatic setup on first run
-- Uses WAL mode for concurrency
-- Creates indexes automatically
```

### PostgreSQL
```sql
-- Automatic setup on first run
-- Requires CREATE TABLE permissions
-- Optional: Enable pg_notify for real-time updates
```

## Error Handling

```rust
use dabgent_mq::MqError;

match queue.publish("stream", "topic", payload).await {
    Ok(id) => println!("Published: {}", id),
    Err(MqError::MessageTooLarge(size, max)) => {
        println!("Message too large: {} > {}", size, max);
    }
    Err(MqError::Database(e)) => {
        println!("Database error: {}", e);
    }
    Err(e) => println!("Other error: {}", e),
}
```

## Testing

Run tests with:
```bash
cargo test

# For integration tests with real databases:
DATABASE_URL=postgres://user:pass@localhost/test cargo test
DATABASE_URL=sqlite://test.db cargo test
```

## License

MIT

## Contributing

Contributions are welcome! Please ensure:
- Tests pass for both SQLite and PostgreSQL
- Code follows Rust best practices
- Documentation is updated for new features