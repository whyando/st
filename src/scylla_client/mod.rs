use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::OnceCell;
use chrono::{DateTime, Utc};
use scylla::{
    client::session::Session, 
    client::session_builder::SessionBuilder, 
    statement::Statement,
    value::CqlValue,
    value::Row,
    value::CqlTimestamp,
};

#[derive(Debug, Clone)]
pub struct CurrentState {
    pub slice_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub state_data: String,
    pub last_updated: CqlValue,
    pub seq_num: i64,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub slice_id: String,
    pub seq_num: i64,            // Primary ordering mechanism within slice
    pub timestamp: CqlValue,     // When the event occurred (for human readability/debugging)
    pub entity_type: String,
    pub entity_id: String,
    pub event_type: String,
    pub event_data: String,
}

#[derive(Debug, Clone)]
pub struct EventByEntity {
    pub slice_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub event_sequence: i64,      // Primary ordering mechanism
    pub timestamp: CqlValue,      // When the event occurred
    pub event_type: String,
    pub event_data: String,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub slice_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub snapshot_sequence: i64,   // Ordering for snapshots
    pub timestamp: CqlValue,      // When snapshot was taken
    pub state_data: String,
    pub seq_num: i64,            // Last event sequence included in this snapshot
}

#[derive(Debug, Clone)]
pub struct EventLog {
    pub event_log_id: String,
    pub last_seq_num: i64,
    pub last_updated: CqlValue,
}

// // Helper functions to convert rows to structs
// impl CurrentState {
//     fn from_row(row: Row) -> Result<Self, Box<dyn std::error::Error>> {
//         Ok(CurrentState {
//             slice_id: row.columns[0].as_text().unwrap_or_default().to_string(),
//             entity_type: row.columns[1].as_text().unwrap_or_default().to_string(),
//             entity_id: row.columns[2].as_text().unwrap_or_default().to_string(),
//             state_data: row.columns[3].as_text().unwrap_or_default().to_string(),
//             last_updated: row.columns[4].clone(),
//             seq_num: row.columns[5].as_bigint().unwrap_or_default(),
//         })
//     }
// }

// impl Event {
//     fn from_row(row: Row) -> Result<Self, Box<dyn std::error::Error>> {
//         Ok(Event {
//             slice_id: row.columns[0].as_text().unwrap_or_default().to_string(),
//             seq_num: row.columns[1].as_bigint().unwrap_or_default(),
//             timestamp: row.columns[2].clone(),
//             entity_type: row.columns[3].as_text().unwrap_or_default().to_string(),
//             entity_id: row.columns[4].as_text().unwrap_or_default().to_string(),
//             event_type: row.columns[5].as_text().unwrap_or_default().to_string(),
//             event_data: row.columns[6].as_text().unwrap_or_default().to_string(),
//         })
//     }
// }

// impl EventByEntity {
//     fn from_row(row: Row) -> Result<Self, Box<dyn std::error::Error>> {
//         Ok(EventByEntity {
//             slice_id: row.columns[0].as_text().unwrap_or_default().to_string(),
//             entity_type: row.columns[1].as_text().unwrap_or_default().to_string(),
//             entity_id: row.columns[2].as_text().unwrap_or_default().to_string(),
//             event_sequence: row.columns[3].as_bigint().unwrap_or_default(),
//             timestamp: row.columns[4].clone(),
//             event_type: row.columns[5].as_text().unwrap_or_default().to_string(),
//             event_data: row.columns[6].as_text().unwrap_or_default().to_string(),
//         })
//     }
// }

// impl Snapshot {
//     fn from_row(row: Row) -> Result<Self, Box<dyn std::error::Error>> {
//         Ok(Snapshot {
//             slice_id: row.columns[0].as_text().unwrap_or_default().to_string(),
//             entity_type: row.columns[1].as_text().unwrap_or_default().to_string(),
//             entity_id: row.columns[2].as_text().unwrap_or_default().to_string(),
//             snapshot_sequence: row.columns[3].as_bigint().unwrap_or_default(),
//             timestamp: row.columns[4].clone(),
//             state_data: row.columns[5].as_text().unwrap_or_default().to_string(),
//             seq_num: row.columns[6].as_bigint().unwrap_or_default(),
//         })
//     }
// }

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
        let query = Statement::new("SELECT last_seq_num FROM spacetraders.event_logs WHERE event_log_id = ? LIMIT 1");
        let result = self.session.query_unpaged(query, (log_id,)).await.unwrap();
        let result = result.into_rows_result().unwrap();
        
        let current_sequence = if let Some(row) = result.rows::<(i64,)>().unwrap().next() {
            let (x,) = row.unwrap();
            x + 1
        } else {
            1 // Start from 1 if no sequence exists for this log
        };

        // Update the sequence
        let update_query = Statement::new(
            "INSERT INTO spacetraders.event_logs (event_log_id, last_seq_num, last_updated) VALUES (?, ?, ?)",
        );
        self.session
            .query_unpaged(update_query, (log_id, current_sequence, Utc::now()))
            .await.unwrap();

        current_sequence
    }

    // // Current State Operations
    // pub async fn get_current_state(
    //     &self,
    //     slice_id: &str,
    //     entity_type: &str,
    //     entity_id: &str,
    // ) -> Result<Option<CurrentState>, QueryError> {
    //     let query = Statement::new("SELECT * FROM spacetraders.current_state WHERE slice_id = ? AND entity_type = ? AND entity_id = ?");
    //     let result = self
    //         .session
    //         .execute(&query, (slice_id.to_string(), entity_type.to_string(), entity_id.to_string()))
    //         .await?;

    //     Ok(result.rows.into_iter().next().map(|row| CurrentState::from_row(row).unwrap()))
    // }

    // pub async fn update_current_state(
    //     &self,
    //     slice_id: &str,
    //     entity_type: &str,
    //     entity_id: &str,
    //     state_data: &str,
    //     seq_num: i64,
    // ) -> Result<(), QueryError> {
    //     let query = Statement::new(
    //         "INSERT INTO spacetraders.current_state (slice_id, entity_type, entity_id, state_data, last_updated, seq_num) VALUES (?, ?, ?, ?, ?, ?)",
    //     );
    //     self.session
    //         .execute(
    //             &query,
    //             (
    //                 slice_id.to_string(),
    //                 entity_type.to_string(),
    //                 entity_id.to_string(),
    //                 state_data.to_string(),
    //                 CqlValue::Timestamp(chrono::Utc::now()),
    //                 seq_num,
    //             ),
    //         )
    //         .await?;
    //     Ok(())
    // }

    // // Event Operations - Main table for consecutive event retrieval
    // pub async fn insert_event(
    //     &self,
    //     slice_id: &str,
    //     entity_type: &str,
    //     entity_id: &str,
    //     event_type: &str,
    //     event_data: &str,
    // ) -> Result<i64, QueryError> {
    //     let seq_num = self.get_next_seq_num(slice_id).await?;
    //     let timestamp = CqlValue::Timestamp(chrono::Utc::now());

    //     // Insert into main events table
    //     let query = Statement::new(
    //         "INSERT INTO spacetraders.events (slice_id, seq_num, timestamp, entity_type, entity_id, event_type, event_data) VALUES (?, ?, ?, ?, ?, ?, ?)",
    //     );
    //     self.session
    //         .execute(
    //             &query,
    //             (
    //                 slice_id.to_string(),
    //                 seq_num,
    //                 timestamp.clone(),
    //                 entity_type.to_string(),
    //                 entity_id.to_string(),
    //                 event_type.to_string(),
    //                 event_data.to_string(),
    //             ),
    //         )
    //         .await?;

    //     // Also insert into entity-specific table for entity queries
    //     let query_entity = Statement::new(
    //         "INSERT INTO spacetraders.events_by_entity (slice_id, entity_type, entity_id, event_sequence, timestamp, event_type, event_data) VALUES (?, ?, ?, ?, ?, ?, ?)",
    //     );
    //     self.session
    //         .execute(
    //             &query_entity,
    //             (
    //                 slice_id.to_string(),
    //                 entity_type.to_string(),
    //                 entity_id.to_string(),
    //                 seq_num,
    //                 timestamp,
    //                 event_type.to_string(),
    //                 event_data.to_string(),
    //             ),
    //         )
    //         .await?;

    //     Ok(seq_num)
    // }

    // /// Get consecutive events across all entities for a specific slice
    // pub async fn get_events(
    //     &self,
    //     slice_id: &str,
    //     from_seq_num: Option<i64>,
    //     limit: Option<i32>,
    // ) -> Result<Vec<Event>, QueryError> {
    //     let mut query_str = "SELECT * FROM spacetraders.events WHERE slice_id = ?".to_string();

    //     if let Some(_from_seq) = from_seq_num {
    //         query_str.push_str(" AND seq_num > ?");
    //     }

    //     query_str.push_str(" ORDER BY seq_num ASC");

    //     if let Some(_limit_val) = limit {
    //         query_str.push_str(" LIMIT ?");
    //     }

    //     let query = Statement::new(query_str);
        
    //     // Use different query patterns based on parameters
    //     let result = if let Some(from_seq) = from_seq_num {
    //         if let Some(limit_val) = limit {
    //             self.session.execute(&query, (slice_id.to_string(), from_seq, limit_val)).await?
    //         } else {
    //             self.session.execute(&query, (slice_id.to_string(), from_seq)).await?
    //         }
    //     } else {
    //         if let Some(limit_val) = limit {
    //             self.session.execute(&query, (slice_id.to_string(), limit_val)).await?
    //         } else {
    //             self.session.execute(&query, (slice_id.to_string(),)).await?
    //         }
    //     };

    //     Ok(result.rows.into_iter().map(|row| Event::from_row(row).unwrap()).collect())
    // }

    // /// Get events for a specific entity within a slice
    // pub async fn get_events_by_entity(
    //     &self,
    //     slice_id: &str,
    //     entity_type: &str,
    //     entity_id: &str,
    //     from_sequence: Option<i64>,
    //     limit: Option<i32>,
    // ) -> Result<Vec<EventByEntity>, QueryError> {
    //     let mut query_str = "SELECT * FROM spacetraders.events_by_entity WHERE slice_id = ? AND entity_type = ? AND entity_id = ?".to_string();

    //     if let Some(_from_seq) = from_sequence {
    //         query_str.push_str(" AND event_sequence > ?");
    //     }

    //     query_str.push_str(" ORDER BY event_sequence ASC");

    //     if let Some(_limit_val) = limit {
    //         query_str.push_str(" LIMIT ?");
    //     }

    //     let query = Statement::new(query_str);
        
    //     // Use different query patterns based on parameters
    //     let result = if let Some(from_seq) = from_sequence {
    //         if let Some(limit_val) = limit {
    //             self.session.execute(&query, (slice_id.to_string(), entity_type.to_string(), entity_id.to_string(), from_seq, limit_val)).await?
    //         } else {
    //             self.session.execute(&query, (slice_id.to_string(), entity_type.to_string(), entity_id.to_string(), from_seq)).await?
    //         }
    //     } else {
    //         if let Some(limit_val) = limit {
    //             self.session.execute(&query, (slice_id.to_string(), entity_type.to_string(), entity_id.to_string(), limit_val)).await?
    //         } else {
    //             self.session.execute(&query, (slice_id.to_string(), entity_type.to_string(), entity_id.to_string())).await?
    //         }
    //     };

    //     Ok(result.rows.into_iter().map(|row| EventByEntity::from_row(row).unwrap()).collect())
    // }

    // /// Get events by entity type for a slice
    // pub async fn get_events_by_entity_type(
    //     &self,
    //     slice_id: &str,
    //     entity_type: &str,
    //     from_seq_num: Option<i64>,
    //     limit: Option<i32>,
    // ) -> Result<Vec<Event>, QueryError> {
    //     let mut query_str = "SELECT * FROM spacetraders.events_by_entity_type WHERE slice_id = ? AND entity_type = ?".to_string();

    //     if let Some(_from_seq) = from_seq_num {
    //         query_str.push_str(" AND seq_num > ?");
    //     }

    //     query_str.push_str(" ORDER BY seq_num ASC");

    //     if let Some(_limit_val) = limit {
    //         query_str.push_str(" LIMIT ?");
    //     }

    //     let query = Statement::new(query_str);
        
    //     // Use different query patterns based on parameters
    //     let result = if let Some(from_seq) = from_seq_num {
    //         if let Some(limit_val) = limit {
    //             self.session.execute(&query, (slice_id.to_string(), entity_type.to_string(), from_seq, limit_val)).await?
    //         } else {
    //             self.session.execute(&query, (slice_id.to_string(), entity_type.to_string(), from_seq)).await?
    //         }
    //     } else {
    //         if let Some(limit_val) = limit {
    //             self.session.execute(&query, (slice_id.to_string(), entity_type.to_string(), limit_val)).await?
    //         } else {
    //             self.session.execute(&query, (slice_id.to_string(), entity_type.to_string())).await?
    //         }
    //     };

    //     Ok(result.rows.into_iter().map(|row| Event::from_row(row).unwrap()).collect())
    // }

    // // Snapshot Operations
    // pub async fn insert_snapshot(
    //     &self,
    //     slice_id: &str,
    //     entity_type: &str,
    //     entity_id: &str,
    //     snapshot_sequence: i64,
    //     state_data: &str,
    //     seq_num: i64,
    // ) -> Result<(), QueryError> {
    //     let query = Statement::new(
    //         "INSERT INTO spacetraders.snapshots (slice_id, entity_type, entity_id, snapshot_sequence, timestamp, state_data, seq_num) VALUES (?, ?, ?, ?, ?, ?, ?)",
    //     );
    //     self.session
    //         .execute(
    //             &query,
    //             (
    //                 slice_id.to_string(),
    //                 entity_type.to_string(),
    //                 entity_id.to_string(),
    //                 snapshot_sequence,
    //                 CqlValue::Timestamp(chrono::Utc::now()),
    //                 state_data.to_string(),
    //                 seq_num,
    //             ),
    //         )
    //         .await?;
    //     Ok(())
    // }

    // pub async fn get_latest_snapshot(
    //     &self,
    //     slice_id: &str,
    //     entity_type: &str,
    //     entity_id: &str,
    // ) -> Result<Option<Snapshot>, QueryError> {
    //     let query = Statement::new(
    //         "SELECT * FROM spacetraders.snapshots WHERE slice_id = ? AND entity_type = ? AND entity_id = ? ORDER BY snapshot_sequence DESC LIMIT 1",
    //     );
    //     let result = self
    //         .session
    //         .execute(&query, (slice_id.to_string(), entity_type.to_string(), entity_id.to_string()))
    //         .await?;

    //     Ok(result.rows.into_iter().next().map(|row| Snapshot::from_row(row).unwrap()))
    // }

    // pub async fn get_snapshots(
    //     &self,
    //     slice_id: &str,
    //     entity_type: &str,
    //     entity_id: &str,
    //     limit: Option<i32>,
    // ) -> Result<Vec<Snapshot>, QueryError> {
    //     let mut query_str = "SELECT * FROM spacetraders.snapshots WHERE slice_id = ? AND entity_type = ? AND entity_id = ? ORDER BY snapshot_sequence DESC".to_string();

    //     if let Some(_limit_val) = limit {
    //         query_str.push_str(" LIMIT ?");
    //     }

    //     let query = Statement::new(query_str);
        
    //     let result = if let Some(limit_val) = limit {
    //         self.session.execute(&query, (slice_id.to_string(), entity_type.to_string(), entity_id.to_string(), limit_val)).await?
    //     } else {
    //         self.session.execute(&query, (slice_id.to_string(), entity_type.to_string(), entity_id.to_string())).await?
    //     };

    //     Ok(result.rows.into_iter().map(|row| Snapshot::from_row(row).unwrap()).collect())
    // }
}
