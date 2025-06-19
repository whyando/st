-- Keyspace for the spacetraders application
CREATE KEYSPACE IF NOT EXISTS spacetraders 
WITH replication = {
    'class': 'SimpleStrategy',
    'replication_factor': 1
};

USE spacetraders;

-- 0. Global Event Log Table
CREATE TABLE IF NOT EXISTS event_logs (
    event_log_id text,          -- '2024-01-15' or 'log-1', 'log-2' etc.
    last_seq_num bigint,        -- last event sequence number in this event log
    last_updated timestamp,     -- when this was last updated
    PRIMARY KEY (event_log_id)
);

-- 1. Current State Table
-- Stores the current state of entities
CREATE TABLE IF NOT EXISTS current_state (
    event_log_id text,          -- '2024-01-15' or 'log-1', 'log-2' etc.
    entity_id text,             -- unique identifier for the entity
    state_data text,            -- JSON serialized state
    last_updated timestamp,     -- when this state was last updated
    seq_num bigint,             -- version number (matches event sequence)
    entity_seq_num bigint,      -- entity-specific sequence number
    last_snapshot_entity_seq_num bigint,  -- entity seq num when last snapshot was taken
    PRIMARY KEY ((event_log_id), entity_id)
);

-- 2. Events Table - Optimized for consecutive event retrieval across all entities
CREATE TABLE IF NOT EXISTS events (
    event_log_id text,          -- '2024-01-15' or 'log-1', 'log-2' etc.
    seq_num bigint,             -- PRIMARY ordering mechanism within event log
    timestamp timestamp,        -- when the event occurred
    entity_id text,             -- unique identifier for the entity
    event_type text,            -- type of event (e.g., 'ship_moved', 'trade_completed')
    event_data text,            -- JSON serialized event data
    PRIMARY KEY ((event_log_id), seq_num)
) WITH CLUSTERING ORDER BY (seq_num ASC);

-- 3. Snapshots Table
-- Stores periodic snapshots for efficient state reconstruction
CREATE TABLE IF NOT EXISTS snapshots (
    event_log_id text,          -- '2024-01-15' or 'log-1', 'log-2' etc.
    entity_id text,             -- unique identifier for the entity
    timestamp timestamp,        -- when the snapshot was taken
    state_data text,            -- JSON serialized state at this snapshot
    seq_num bigint,             -- event sequence number when this snapshot was taken
    PRIMARY KEY ((event_log_id), entity_id, seq_num)
) WITH CLUSTERING ORDER BY (entity_id ASC, seq_num ASC);

-- Materialized view for querying events for a specific entity id
CREATE MATERIALIZED VIEW IF NOT EXISTS events_by_entity_id AS
SELECT event_log_id, seq_num, timestamp, entity_id, event_type, event_data
FROM events
WHERE event_log_id IS NOT NULL AND entity_id IS NOT NULL AND seq_num IS NOT NULL
PRIMARY KEY ((event_log_id), entity_id, seq_num)
WITH CLUSTERING ORDER BY (entity_id ASC, seq_num ASC);

-- Secondary index for timestamp-based event queries
CREATE INDEX IF NOT EXISTS ON events (timestamp);
