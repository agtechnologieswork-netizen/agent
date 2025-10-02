CREATE TABLE IF NOT EXISTS events (
    stream_id TEXT NOT NULL,
    aggregate_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    sequence BIGINT NOT NULL,
    event_type TEXT NOT NULL,
    event_version TEXT NOT NULL,
    data JSONB NOT NULL,
    metadata JSONB NOT NULL,
    PRIMARY KEY (stream_id, aggregate_type, aggregate_id, sequence)
);
