CREATE TABLE IF NOT EXISTS events (
    stream_id TEXT NOT NULL,
    aggregate_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    event_version TEXT NOT NULL,
    data TEXT NOT NULL,
    metadata TEXT NOT NULL,
    PRIMARY KEY (stream_id, aggregate_type, aggregate_id, sequence)
);
