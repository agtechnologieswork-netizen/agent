CREATE TABLE IF NOT EXISTS events (
    stream_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    event_version TEXT NOT NULL,
    data TEXT NOT NULL,
    metadata TEXT NOT NULL,
    created_at TEXT NOT NULL,  -- ISO 8601 timestamp format
    PRIMARY KEY (stream_id, event_type, aggregate_id, sequence)
);

CREATE INDEX idx_events_stream_sequence ON events (stream_id, sequence);

CREATE INDEX idx_events_type_sequence ON events (event_type, sequence);

CREATE INDEX idx_events_aggregate_sequence ON events (aggregate_id, sequence);

CREATE INDEX idx_events_created_at ON events (created_at);
