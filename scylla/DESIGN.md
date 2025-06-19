# Scylla Time Travel Design

## Overview

The Scylla event store enables a **time travel system** for ships in SpaceTraders. Every ship action (movement, trading, mining, etc.) is recorded as an immutable event, allowing us to reconstruct any ship's state at any point in time.

## Time Travel Capabilities

### Ship State Reconstruction
```rust
// Reconstruct ship state at any point in time
let ship_state = scylla_client
    .get_events_by_entity("2024-01-15", "SHIP-123", Some(1000), None)
    .await?;

// Apply events in sequence to rebuild ship state
let reconstructed_ship = reconstruct_ship_from_events(ship_state);
```

### Temporal Queries
- **"Where was ship X at 2:30 PM yesterday?"**
- **"What cargo did ship Y have before the trade?"**
- **"Show me all ships in this system 3 hours ago"**

## Key Tables for Time Travel

### Events Table
```sql
CREATE TABLE events (
    event_log_id text,         -- '2024-01-15' (daily slices)
    seq_num bigint,            -- Global sequence number
    timestamp timestamp,       -- When event occurred
    entity_id text,            -- 'SHIP-123'
    event_type text,           -- 'ship_moved', 'cargo_updated'
    event_data text,           -- JSON event details
    PRIMARY KEY ((event_log_id), seq_num)
);
```

### Current State Table
```sql
CREATE TABLE current_state (
    event_log_id text,         -- '2024-01-15'
    entity_id text,            -- 'SHIP-123'
    state_data text,           -- Complete ship state JSON
    last_updated timestamp,
    seq_num bigint,            -- Version number
    entity_seq_num bigint,     -- Entity-specific sequence
    last_snapshot_entity_seq_num bigint,
    PRIMARY KEY ((event_log_id), entity_id)
);
```

### Snapshots Table
```sql
CREATE TABLE snapshots (
    event_log_id text,
    entity_id text,
    timestamp timestamp,
    state_data text,           -- Complete ship state JSON
    seq_num bigint,            -- Last event included
    PRIMARY KEY ((event_log_id), entity_id, seq_num)
);
```

### Events by Entity View
```sql
CREATE MATERIALIZED VIEW events_by_entity_id AS
SELECT event_log_id, seq_num, timestamp, entity_id, event_type, event_data
FROM events
WHERE event_log_id IS NOT NULL AND entity_id IS NOT NULL AND seq_num IS NOT NULL
PRIMARY KEY ((event_log_id), entity_id, seq_num);
```

## Time Travel Operations

### 1. Fast State Lookup
```rust
// Get ship state at specific time using snapshots
let snapshot = scylla_client
    .get_latest_snapshot("2024-01-15", "SHIP-123")
    .await?;

// Apply events after snapshot to reach target time
let events_after = scylla_client
    .get_events_by_entity("2024-01-15", "SHIP-123", Some(snapshot.seq_num), None)
    .await?;
```

### 2. Ship Movement History
```rust
// Get all movement events for a ship
let movements = scylla_client
    .get_events_by_entity("2024-01-15", "SHIP-123", None, None)
    .await?
    .into_iter()
    .filter(|e| e.event_type == "ship_moved")
    .collect();
```

### 3. Cargo Evolution
```rust
// Track cargo changes over time
let cargo_events = scylla_client
    .get_events_by_entity("2024-01-15", "SHIP-123", None, None)
    .await?
    .into_iter()
    .filter(|e| e.event_type == "cargo_updated")
    .collect();
```

## Future Additions

### Zone-Based Event Tracking

A planned enhancement will add **zone events** to track when ships enter or leave specific areas, enabling more efficient event filtering and monitoring.

#### Zone Event Types
- `zone_entered`: Ship enters a defined zone (system, waypoint, region)
- `zone_exited`: Ship leaves a defined zone
- `zone_boundary_crossed`: Ship crosses between different zone types

#### Zone Event Structure
```sql
-- Zone events will use the same events table with zone-specific event types
CREATE TABLE zone_events (
    event_log_id text,
    seq_num bigint,
    timestamp timestamp,
    entity_id text,            -- 'SHIP-123'
    event_type text,           -- 'zone_entered', 'zone_exited'
    event_data text,           -- JSON: {"zone_id": "X1-DF55", "zone_type": "system"}
    PRIMARY KEY ((event_log_id), seq_num)
);
```

#### Zone-Based Queries
```rust
// Get all ships currently in a specific system
let ships_in_system = scylla_client
    .get_zone_events("2024-01-15", "X1-DF55", "system", None, None)
    .await?;

// Track ship's zone history
let zone_history = scylla_client
    .get_events_by_entity("2024-01-15", "SHIP-123", None, None)
    .await?
    .into_iter()
    .filter(|e| e.event_type.starts_with("zone_"))
    .collect();

// Find ships that entered a zone within time window
let recent_entries = scylla_client
    .get_zone_entries("2024-01-15", "X1-DF55", start_time, end_time)
    .await?;
```

#### Benefits of Zone Tracking
1. **Efficient Filtering**: Only track events for ships in relevant zones
2. **Geographic Analytics**: Analyze ship movement patterns by region
3. **Resource Optimization**: Reduce event processing for ships outside areas of interest
4. **Boundary Monitoring**: Detect when ships cross important boundaries
5. **Fleet Management**: Track fleet dispersion across different zones

#### Zone Types
- **Systems**: Complete star systems (e.g., "X1-DF55")
- **Waypoints**: Specific locations within systems
- **Regions**: Larger geographic areas spanning multiple systems
- **Custom Zones**: User-defined areas of interest
