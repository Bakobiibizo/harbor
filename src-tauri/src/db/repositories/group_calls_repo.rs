use crate::db::Database;
use rusqlite::{params, OptionalExtension, Result as SqliteResult};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupCallRoom {
    pub room_id: String,
    pub creator_peer_id: String,
    pub topology: String,
    pub media_mode: String,
    pub roster_version: u64,
    pub participants: Vec<String>,
    pub state: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct GroupCallsRepository;

impl GroupCallsRepository {
    fn from_row(row: &rusqlite::Row<'_>) -> SqliteResult<GroupCallRoom> {
        let value: String = row.get(5)?;
        let participants = serde_json::from_str(&value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                value.len(),
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
        Ok(GroupCallRoom {
            room_id: row.get(0)?,
            creator_peer_id: row.get(1)?,
            topology: row.get(2)?,
            media_mode: row.get(3)?,
            roster_version: row.get::<_, i64>(4)? as u64,
            participants,
            state: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    }

    pub fn get(db: &Database, room_id: &str) -> SqliteResult<Option<GroupCallRoom>> {
        db.with_connection(|conn| {
            conn.query_row(
                "SELECT room_id, creator_peer_id, topology, media_mode, roster_version,
                        participants_json, state, created_at, updated_at
                 FROM group_call_rooms WHERE room_id = ?",
                [room_id],
                Self::from_row,
            )
            .optional()
        })
    }

    pub fn active(db: &Database) -> SqliteResult<Vec<GroupCallRoom>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT room_id, creator_peer_id, topology, media_mode, roster_version,
                        participants_json, state, created_at, updated_at
                 FROM group_call_rooms WHERE state IN ('invited', 'active')
                 ORDER BY updated_at DESC",
            )?;
            let rooms = stmt.query_map([], Self::from_row)?.collect();
            rooms
        })
    }

    pub fn upsert(db: &Database, room: &GroupCallRoom) -> SqliteResult<()> {
        let participants = serde_json::to_string(&room.participants)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO group_call_rooms (room_id, creator_peer_id, topology, media_mode,
                    roster_version, participants_json, state, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(room_id) DO UPDATE SET topology=excluded.topology,
                    media_mode=excluded.media_mode, roster_version=excluded.roster_version,
                    participants_json=excluded.participants_json, state=excluded.state,
                    updated_at=excluded.updated_at",
                params![
                    room.room_id,
                    room.creator_peer_id,
                    room.topology,
                    room.media_mode,
                    room.roster_version as i64,
                    participants,
                    room.state,
                    room.created_at,
                    room.updated_at
                ],
            )?;
            Ok(())
        })
    }

    pub fn record_nonce(
        db: &Database,
        room_id: &str,
        sender_peer_id: &str,
        nonce: &str,
        timestamp: i64,
    ) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            Ok(conn.execute(
                "INSERT OR IGNORE INTO group_call_nonces
             (room_id, sender_peer_id, nonce, received_at) VALUES (?, ?, ?, ?)",
                params![room_id, sender_peer_id, nonce, timestamp],
            )? == 1)
        })
    }
}
