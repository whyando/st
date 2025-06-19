use chrono::{DateTime, Utc};
use scylla::deserialize::row::ColumnIterator;
use scylla::deserialize::row::DeserializeRow;
use scylla::{
    client::{session::Session, session_builder::SessionBuilder},
    response::query_result::QueryRowsResult,
    statement::Statement,
    value::{CqlTimestamp, CqlValue, Row},
    DeserializeRow, SerializeRow,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::OnceCell;

#[derive(Debug, DeserializeRow)]
pub struct CurrentState {
    pub event_log_id: String,
    pub entity_id: String,
    pub state_data: String,
    pub last_updated: DateTime<Utc>,
    pub seq_num: i64,
    pub entity_seq_num: i64,
    pub last_snapshot_entity_seq_num: i64,
}

#[derive(Debug, DeserializeRow)]
pub struct Event {
    pub event_log_id: String,
    pub seq_num: i64,             // Primary ordering mechanism within event log
    pub timestamp: DateTime<Utc>, // When the event occurred
    pub entity_id: String,
    pub event_type: String,
    pub event_data: String,
}

#[derive(Debug, DeserializeRow)]
pub struct Snapshot {
    pub event_log_id: String,
    pub entity_id: String,
    pub timestamp: DateTime<Utc>, // When snapshot was taken
    pub state_data: String,
    pub seq_num: i64, // Event sequence number when this snapshot was taken
}

#[derive(Debug, DeserializeRow, SerializeRow)]
pub struct EventLog {
    pub event_log_id: String,
    pub last_seq_num: i64,
    pub last_updated: DateTime<Utc>,
}

pub struct ScyllaClient {
    session: Arc<Session>,
}

impl ScyllaClient {
    pub async fn new() -> Self {
        let session = SessionBuilder::new()
            .known_node(std::env::var("SCYLLA_URI").expect("SCYLLA_URI env var not set"))
            .build()
            .await
            .expect("Failed to connect to Scylla");

        ScyllaClient {
            session: Arc::new(session),
        }
    }

    /// Get the current global event sequence number for a log
    pub async fn get_next_seq_num(&self, log_id: &str) -> i64 {
        // First try to get the current sequence for this log
        let query = Statement::new("SELECT event_log_id, last_seq_num, last_updated FROM spacetraders.event_logs WHERE event_log_id = ? LIMIT 1");
        let result = self.session.query_unpaged(query, &(log_id,)).await.unwrap();
        let result = result.into_rows_result().unwrap();

        let current_sequence = if let Some(row) = result.rows::<EventLog>().unwrap().next() {
            let event_log = row.unwrap();
            event_log.last_seq_num + 1
        } else {
            1 // Start from 1 if no sequence exists for this log
        };

        // Update the sequence
        let update_query = Statement::new(
            "INSERT INTO spacetraders.event_logs (event_log_id, last_seq_num, last_updated) VALUES (?, ?, ?)",
        );
        let upsert = EventLog {
            event_log_id: log_id.to_string(),
            last_seq_num: current_sequence,
            last_updated: Utc::now(),
        };
        self.session
            .query_unpaged(update_query, &upsert)
            .await
            .unwrap();

        current_sequence
    }

    // Current State Operations
    pub async fn get_current_state(
        &self,
        event_log_id: &str,
        entity_id: &str,
    ) -> Option<CurrentState> {
        let query = Statement::new("SELECT * FROM spacetraders.current_state WHERE event_log_id = ? AND entity_id = ? LIMIT 1");
        let result = self
            .session
            .query_unpaged(query, &(event_log_id.to_string(), entity_id.to_string()))
            .await
            .unwrap();
        let result = result.into_rows_result().unwrap();
        result
            .rows::<CurrentState>()
            .unwrap()
            .next()
            .map(|row| row.unwrap())
    }

    pub async fn upsert_current_state(&self, current_state: CurrentState) {
        let query = Statement::new("INSERT INTO spacetraders.current_state (event_log_id, entity_id, state_data, last_updated, seq_num, entity_seq_num, last_snapshot_entity_seq_num) VALUES (?, ?, ?, ?, ?, ?, ?)");
        self.session
            .query_unpaged(
                query,
                &(
                    current_state.event_log_id.to_string(),
                    current_state.entity_id.to_string(),
                    current_state.state_data.to_string(),
                    current_state.last_updated,
                    current_state.seq_num,
                    current_state.entity_seq_num,
                    current_state.last_snapshot_entity_seq_num,
                ),
            )
            .await
            .unwrap();
    }

    // Event Operations - Main table for consecutive event retrieval
    pub async fn insert_event(
        &self,
        event_log_id: &str,
        entity_id: &str,
        event_type: &str,
        event_data: &str,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let seq_num = self.get_next_seq_num(event_log_id).await;
        let timestamp = CqlValue::Timestamp(chrono::Utc::now());

        // Insert into main events table
        let query = Statement::new(
            "INSERT INTO spacetraders.events (event_log_id, seq_num, timestamp, entity_id, event_type, event_data) VALUES (?, ?, ?, ?, ?, ?)",
        );
        self.session
            .query_unpaged(
                query,
                (
                    event_log_id.to_string(),
                    seq_num,
                    timestamp.clone(),
                    entity_id.to_string(),
                    event_type.to_string(),
                    event_data.to_string(),
                ),
            )
            .await?;

        Ok(seq_num)
    }

    /// Get consecutive events across all entities for a specific event log
    pub async fn get_events(
        &self,
        event_log_id: &str,
        from_seq_num: Option<i64>,
        limit: Option<i32>,
    ) -> Result<Vec<Event>, Box<dyn std::error::Error>> {
        let mut query_str = "SELECT * FROM spacetraders.events WHERE event_log_id = ?".to_string();

        if let Some(_from_seq) = from_seq_num {
            query_str.push_str(" AND seq_num > ?");
        }

        query_str.push_str(" ORDER BY seq_num ASC");

        if let Some(_limit_val) = limit {
            query_str.push_str(" LIMIT ?");
        }

        let query = Statement::new(query_str);

        // Use different query patterns based on parameters
        let result = if let Some(from_seq) = from_seq_num {
            if let Some(limit_val) = limit {
                self.session
                    .query_unpaged(query, (event_log_id.to_string(), from_seq, limit_val))
                    .await?
            } else {
                self.session
                    .query_unpaged(query, (event_log_id.to_string(), from_seq))
                    .await?
            }
        } else {
            if let Some(limit_val) = limit {
                self.session
                    .query_unpaged(query, (event_log_id.to_string(), limit_val))
                    .await?
            } else {
                self.session
                    .query_unpaged(query, (event_log_id.to_string(),))
                    .await?
            }
        };

        let rows = result.into_rows_result()?;
        Ok(rows
            .rows::<(String, i64, CqlValue, String, String, String)>()?
            .map(|row| {
                let (event_log_id, seq_num, timestamp, entity_id, event_type, event_data) =
                    row.unwrap();
                Event {
                    event_log_id,
                    seq_num,
                    timestamp,
                    entity_id,
                    event_type,
                    event_data,
                }
            })
            .collect())
    }

    /// Get events for a specific entity within an event log using the materialized view
    pub async fn get_events_by_entity(
        &self,
        event_log_id: &str,
        entity_id: &str,
        from_sequence: Option<i64>,
        limit: Option<i32>,
    ) -> Result<Vec<EventByEntity>, Box<dyn std::error::Error>> {
        let mut query_str = "SELECT * FROM spacetraders.events_by_entity_id WHERE event_log_id = ? AND entity_id = ?".to_string();

        if let Some(_from_seq) = from_sequence {
            query_str.push_str(" AND seq_num > ?");
        }

        query_str.push_str(" ORDER BY entity_id ASC, seq_num ASC");

        if let Some(_limit_val) = limit {
            query_str.push_str(" LIMIT ?");
        }

        let query = Statement::new(query_str);

        // Use different query patterns based on parameters
        let result = if let Some(from_seq) = from_sequence {
            if let Some(limit_val) = limit {
                self.session
                    .query_unpaged(
                        query,
                        (
                            event_log_id.to_string(),
                            entity_id.to_string(),
                            from_seq,
                            limit_val,
                        ),
                    )
                    .await?
            } else {
                self.session
                    .query_unpaged(
                        query,
                        (event_log_id.to_string(), entity_id.to_string(), from_seq),
                    )
                    .await?
            }
        } else {
            if let Some(limit_val) = limit {
                self.session
                    .query_unpaged(
                        query,
                        (event_log_id.to_string(), entity_id.to_string(), limit_val),
                    )
                    .await?
            } else {
                self.session
                    .query_unpaged(query, (event_log_id.to_string(), entity_id.to_string()))
                    .await?
            }
        };

        let rows = result.into_rows_result()?;
        Ok(rows
            .rows::<(String, String, i64, CqlValue, String, String)>()?
            .map(|row| {
                let (event_log_id, entity_id, seq_num, timestamp, event_type, event_data) =
                    row.unwrap();
                EventByEntity {
                    event_log_id,
                    entity_id,
                    seq_num,
                    timestamp,
                    event_type,
                    event_data,
                }
            })
            .collect())
    }

    // Snapshot Operations
    pub async fn insert_snapshot(
        &self,
        event_log_id: &str,
        entity_id: &str,
        state_data: &str,
        seq_num: i64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let query = Statement::new(
            "INSERT INTO spacetraders.snapshots (event_log_id, entity_id, timestamp, state_data, seq_num) VALUES (?, ?, ?, ?, ?)",
        );
        self.session
            .query_unpaged(
                query,
                (
                    event_log_id.to_string(),
                    entity_id.to_string(),
                    CqlValue::Timestamp(chrono::Utc::now()),
                    state_data.to_string(),
                    seq_num,
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn get_latest_snapshot(
        &self,
        event_log_id: &str,
        entity_id: &str,
    ) -> Result<Option<Snapshot>, Box<dyn std::error::Error>> {
        let query = Statement::new(
            "SELECT * FROM spacetraders.snapshots WHERE event_log_id = ? AND entity_id = ? ORDER BY seq_num DESC LIMIT 1",
        );
        let result = self
            .session
            .query_unpaged(query, (event_log_id.to_string(), entity_id.to_string()))
            .await?;

        let rows = result.into_rows_result()?;
        Ok(rows
            .rows::<(String, String, CqlValue, String, i64)>()?
            .next()
            .map(|row| {
                let (event_log_id, entity_id, timestamp, state_data, seq_num) = row.unwrap();
                Snapshot {
                    event_log_id,
                    entity_id,
                    timestamp,
                    state_data,
                    seq_num,
                }
            }))
    }

    pub async fn get_snapshots(
        &self,
        event_log_id: &str,
        entity_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<Snapshot>, Box<dyn std::error::Error>> {
        let mut query_str = "SELECT * FROM spacetraders.snapshots WHERE event_log_id = ? AND entity_id = ? ORDER BY seq_num DESC".to_string();

        if let Some(_limit_val) = limit {
            query_str.push_str(" LIMIT ?");
        }

        let query = Statement::new(query_str);

        let result = if let Some(limit_val) = limit {
            self.session
                .query_unpaged(
                    query,
                    (event_log_id.to_string(), entity_id.to_string(), limit_val),
                )
                .await?
        } else {
            self.session
                .query_unpaged(query, (event_log_id.to_string(), entity_id.to_string()))
                .await?
        };

        let rows = result.into_rows_result()?;
        Ok(rows
            .rows::<(String, String, CqlValue, String, i64)>()?
            .map(|row| {
                let (event_log_id, entity_id, timestamp, state_data, seq_num) = row.unwrap();
                Snapshot {
                    event_log_id,
                    entity_id,
                    timestamp,
                    state_data,
                    seq_num,
                }
            })
            .collect())
    }
}
