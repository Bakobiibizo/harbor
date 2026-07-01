//! Call session repository for durable active-call state and history.
//!
//! This repository deliberately stores only lifecycle metadata. SDP payloads,
//! ICE candidates, and media bytes remain transient signaling/media data and
//! must not be persisted here.

use crate::db::Database;
use rusqlite::{params, OptionalExtension, Result as SqliteResult, Row};

/// Call direction relative to the local identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallDirection {
    Outgoing,
    Incoming,
}

impl CallDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            CallDirection::Outgoing => "outgoing",
            CallDirection::Incoming => "incoming",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "outgoing" => Some(CallDirection::Outgoing),
            "incoming" => Some(CallDirection::Incoming),
            _ => None,
        }
    }
}

/// Media kind requested for a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallMediaKind {
    Audio,
    Video,
}

impl CallMediaKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CallMediaKind::Audio => "audio",
            CallMediaKind::Video => "video",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "audio" => Some(CallMediaKind::Audio),
            "video" => Some(CallMediaKind::Video),
            _ => None,
        }
    }
}

/// Durable call lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallState {
    /// Outgoing call, waiting for answer.
    Ringing,
    /// Incoming call, not yet answered.
    Incoming,
    /// Call is connected.
    Connected,
    /// Call ended.
    Ended,
}

impl CallState {
    pub fn as_str(&self) -> &'static str {
        match self {
            CallState::Ringing => "ringing",
            CallState::Incoming => "incoming",
            CallState::Connected => "connected",
            CallState::Ended => "ended",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "ringing" => Some(CallState::Ringing),
            "incoming" => Some(CallState::Incoming),
            "connected" => Some(CallState::Connected),
            "ended" => Some(CallState::Ended),
            _ => None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, CallState::Ended)
    }
}

/// A stored call session/history row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSession {
    pub id: i64,
    pub call_id: String,
    /// Remote peer ID retained for compatibility with the original
    /// `call_history.peer_id` column.
    pub peer_id: String,
    pub caller_peer_id: Option<String>,
    pub callee_peer_id: Option<String>,
    pub direction: CallDirection,
    pub media_kind: CallMediaKind,
    pub state: CallState,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub duration_seconds: Option<i64>,
    pub terminal_reason: Option<String>,
    pub updated_at: i64,
}

/// Data needed to create a call session.
#[derive(Debug, Clone)]
pub struct NewCallSession {
    pub call_id: String,
    pub peer_id: String,
    pub caller_peer_id: String,
    pub callee_peer_id: String,
    pub direction: CallDirection,
    pub media_kind: CallMediaKind,
    pub state: CallState,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub duration_seconds: Option<i64>,
    pub terminal_reason: Option<String>,
}

/// Repository for call lifecycle state/history.
pub struct CallsRepository;

impl CallsRepository {
    /// Insert a new call session or terminal history row.
    pub fn insert_session(db: &Database, session: &NewCallSession) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO call_history (
                    call_id, peer_id, caller_peer_id, callee_peer_id, direction,
                    media_kind, status, started_at, ended_at, duration_seconds,
                    terminal_reason, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    session.call_id,
                    session.peer_id,
                    session.caller_peer_id,
                    session.callee_peer_id,
                    session.direction.as_str(),
                    session.media_kind.as_str(),
                    session.state.as_str(),
                    session.started_at,
                    session.ended_at,
                    session.duration_seconds,
                    session.terminal_reason,
                    session.ended_at.unwrap_or(session.started_at),
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Get a call session by stable call ID.
    pub fn get_by_call_id(db: &Database, call_id: &str) -> SqliteResult<Option<CallSession>> {
        db.with_connection(|conn| {
            conn.query_row(
                Self::select_sql_with_where("call_id = ?").as_str(),
                [call_id],
                Self::row_to_call_session,
            )
            .optional()
        })
    }

    /// Return active calls (all non-terminal rows), newest first.
    pub fn get_active_calls(db: &Database) -> SqliteResult<Vec<CallSession>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                Self::select_sql_with_where(
                    "status != 'ended' ORDER BY updated_at DESC, started_at DESC",
                )
                .as_str(),
            )?;
            let rows = stmt.query_map([], Self::row_to_call_session)?;
            rows.collect()
        })
    }

    /// Return recent call history, newest first. Active calls are included so
    /// callers can rebuild UI state from one persisted source after restart.
    pub fn get_call_history(db: &Database, limit: usize) -> SqliteResult<Vec<CallSession>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                format!(
                    "{} ORDER BY COALESCE(ended_at, updated_at, started_at) DESC, id DESC LIMIT ?",
                    Self::select_sql()
                )
                .as_str(),
            )?;
            let rows = stmt.query_map([limit as i64], Self::row_to_call_session)?;
            rows.collect()
        })
    }

    /// True if an active call exists with a peer.
    pub fn has_active_call_with_peer(db: &Database, peer_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM call_history WHERE peer_id = ? AND status != 'ended'",
                [peer_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Transition a call to connected.
    pub fn mark_connected(db: &Database, call_id: &str, timestamp: i64) -> SqliteResult<usize> {
        db.with_connection(|conn| {
            conn.execute(
                "UPDATE call_history
                 SET status = 'connected', updated_at = ?
                 WHERE call_id = ? AND status IN ('ringing', 'incoming')",
                params![timestamp, call_id],
            )
        })
    }

    /// Transition an active call to ended with a terminal reason.
    pub fn mark_ended(
        db: &Database,
        call_id: &str,
        terminal_reason: &str,
        timestamp: i64,
    ) -> SqliteResult<usize> {
        db.with_connection(|conn| {
            conn.execute(
                "UPDATE call_history
                 SET status = 'ended',
                     ended_at = ?,
                     duration_seconds = CASE
                         WHEN started_at IS NULL THEN 0
                         WHEN ? - started_at < 0 THEN 0
                         ELSE ? - started_at
                     END,
                     terminal_reason = ?,
                     updated_at = ?
                 WHERE call_id = ? AND status != 'ended'",
                params![
                    timestamp,
                    timestamp,
                    timestamp,
                    terminal_reason,
                    timestamp,
                    call_id,
                ],
            )
        })
    }

    fn select_sql() -> &'static str {
        "SELECT id, call_id, peer_id, caller_peer_id, callee_peer_id,
                direction, media_kind, status, started_at, ended_at,
                duration_seconds, terminal_reason, updated_at
         FROM call_history"
    }

    fn select_sql_with_where(where_clause: &str) -> String {
        format!("{} WHERE {}", Self::select_sql(), where_clause)
    }

    fn row_to_call_session(row: &Row<'_>) -> SqliteResult<CallSession> {
        let direction_text: String = row.get(5)?;
        let media_kind_text: String = row.get(6)?;
        let state_text: String = row.get(7)?;
        Ok(CallSession {
            id: row.get(0)?,
            call_id: row.get(1)?,
            peer_id: row.get(2)?,
            caller_peer_id: row.get(3)?,
            callee_peer_id: row.get(4)?,
            direction: CallDirection::from_str(&direction_text).unwrap_or(CallDirection::Incoming),
            media_kind: CallMediaKind::from_str(&media_kind_text).unwrap_or(CallMediaKind::Audio),
            state: CallState::from_str(&state_text).unwrap_or(CallState::Ended),
            started_at: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
            ended_at: row.get(9)?,
            duration_seconds: row.get(10)?,
            terminal_reason: row.get(11)?,
            updated_at: row.get::<_, Option<i64>>(12)?.unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outgoing_session(call_id: &str, started_at: i64) -> NewCallSession {
        NewCallSession {
            call_id: call_id.to_string(),
            peer_id: "peer-callee".to_string(),
            caller_peer_id: "peer-local".to_string(),
            callee_peer_id: "peer-callee".to_string(),
            direction: CallDirection::Outgoing,
            media_kind: CallMediaKind::Audio,
            state: CallState::Ringing,
            started_at,
            ended_at: None,
            duration_seconds: None,
            terminal_reason: None,
        }
    }

    #[test]
    fn records_valid_transitions_and_history() {
        let db = Database::in_memory().unwrap();
        CallsRepository::insert_session(&db, &outgoing_session("call-1", 10)).unwrap();

        assert_eq!(CallsRepository::get_active_calls(&db).unwrap().len(), 1);
        assert_eq!(
            CallsRepository::mark_connected(&db, "call-1", 20).unwrap(),
            1
        );
        let connected = CallsRepository::get_by_call_id(&db, "call-1")
            .unwrap()
            .unwrap();
        assert_eq!(connected.state, CallState::Connected);

        assert_eq!(
            CallsRepository::mark_ended(&db, "call-1", "normal", 45).unwrap(),
            1
        );
        let ended = CallsRepository::get_by_call_id(&db, "call-1")
            .unwrap()
            .unwrap();
        assert_eq!(ended.state, CallState::Ended);
        assert_eq!(ended.ended_at, Some(45));
        assert_eq!(ended.duration_seconds, Some(35));
        assert_eq!(ended.terminal_reason.as_deref(), Some("normal"));
        assert!(CallsRepository::get_active_calls(&db).unwrap().is_empty());
        assert_eq!(CallsRepository::get_call_history(&db, 10).unwrap().len(), 1);
    }

    #[test]
    fn state_updates_reject_unknown_or_ended_rows() {
        let db = Database::in_memory().unwrap();
        assert_eq!(
            CallsRepository::mark_connected(&db, "missing", 20).unwrap(),
            0
        );
        assert_eq!(
            CallsRepository::mark_ended(&db, "missing", "normal", 20).unwrap(),
            0
        );

        CallsRepository::insert_session(&db, &outgoing_session("call-1", 10)).unwrap();
        assert_eq!(
            CallsRepository::mark_ended(&db, "call-1", "normal", 20).unwrap(),
            1
        );
        assert_eq!(
            CallsRepository::mark_connected(&db, "call-1", 25).unwrap(),
            0
        );
        assert_eq!(
            CallsRepository::mark_ended(&db, "call-1", "normal", 30).unwrap(),
            0
        );
    }

    #[test]
    fn active_calls_survive_database_reopen() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("harbor.sqlite");
        {
            let db = Database::new(db_path.clone()).unwrap();
            CallsRepository::insert_session(&db, &outgoing_session("call-1", 10)).unwrap();
        }
        let reopened = Database::new(db_path).unwrap();
        let active = CallsRepository::get_active_calls(&reopened).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].call_id, "call-1");
        assert_eq!(active[0].state, CallState::Ringing);
    }
}
