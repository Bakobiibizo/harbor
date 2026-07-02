//! Signed wall social event repository.

use crate::db::Database;
use rusqlite::{params, OptionalExtension, Result as SqliteResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallSocialEventType {
    CommentCreate,
    CommentDelete,
    ReactionAdd,
    ReactionRemove,
    LegacyCommentCreate,
    LegacyReactionAdd,
}

impl WallSocialEventType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommentCreate => "comment_create",
            Self::CommentDelete => "comment_delete",
            Self::ReactionAdd => "reaction_add",
            Self::ReactionRemove => "reaction_remove",
            Self::LegacyCommentCreate => "legacy_comment_create",
            Self::LegacyReactionAdd => "legacy_reaction_add",
        }
    }

    pub fn parse_event_type(value: &str) -> Option<Self> {
        match value {
            "comment_create" => Some(Self::CommentCreate),
            "comment_delete" => Some(Self::CommentDelete),
            "reaction_add" => Some(Self::ReactionAdd),
            "reaction_remove" => Some(Self::ReactionRemove),
            "legacy_comment_create" => Some(Self::LegacyCommentCreate),
            "legacy_reaction_add" => Some(Self::LegacyReactionAdd),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallSocialEvent {
    pub id: i64,
    pub event_id: String,
    pub event_type: WallSocialEventType,
    pub post_id: String,
    pub actor_peer_id: String,
    pub author_name: Option<String>,
    pub comment_id: Option<String>,
    pub content: Option<String>,
    pub reaction_type: Option<String>,
    pub timestamp: i64,
    pub payload_cbor: Vec<u8>,
    pub signature: Vec<u8>,
    pub received_at: i64,
}

pub struct WallSocialEventData<'a> {
    pub event_id: &'a str,
    pub event_type: WallSocialEventType,
    pub post_id: &'a str,
    pub actor_peer_id: &'a str,
    pub author_name: Option<&'a str>,
    pub comment_id: Option<&'a str>,
    pub content: Option<&'a str>,
    pub reaction_type: Option<&'a str>,
    pub timestamp: i64,
    pub payload_cbor: &'a [u8],
    pub signature: &'a [u8],
}

pub struct WallSocialEventsRepository;

impl WallSocialEventsRepository {
    pub fn record_event(db: &Database, event: &WallSocialEventData<'_>) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let inserted = conn.execute(
                "INSERT OR IGNORE INTO wall_social_events (
                    event_id, event_type, post_id, actor_peer_id, author_name, comment_id,
                    content, reaction_type, timestamp, payload_cbor, signature, received_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    event.event_id,
                    event.event_type.as_str(),
                    event.post_id,
                    event.actor_peer_id,
                    event.author_name,
                    event.comment_id,
                    event.content,
                    event.reaction_type,
                    event.timestamp,
                    event.payload_cbor,
                    event.signature,
                    chrono::Utc::now().timestamp(),
                ],
            )?;
            Ok(inserted > 0)
        })
    }

    pub fn get_by_event_id(db: &Database, event_id: &str) -> SqliteResult<Option<WallSocialEvent>> {
        db.with_connection(|conn| {
            conn.query_row(
                "SELECT id, event_id, event_type, post_id, actor_peer_id, author_name,
                        comment_id, content, reaction_type, timestamp, payload_cbor,
                        signature, received_at
                 FROM wall_social_events WHERE event_id = ?",
                params![event_id],
                Self::row_to_event,
            )
            .optional()
        })
    }

    pub fn list_for_post(db: &Database, post_id: &str) -> SqliteResult<Vec<WallSocialEvent>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, event_id, event_type, post_id, actor_peer_id, author_name,
                        comment_id, content, reaction_type, timestamp, payload_cbor,
                        signature, received_at
                 FROM wall_social_events WHERE post_id = ? ORDER BY timestamp ASC, id ASC",
            )?;
            let rows = stmt.query_map(params![post_id], Self::row_to_event)?;
            rows.collect()
        })
    }

    pub fn list_by_actor_since(
        db: &Database,
        actor_peer_id: &str,
        after_timestamp: i64,
        limit: i64,
    ) -> SqliteResult<Vec<WallSocialEvent>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, event_id, event_type, post_id, actor_peer_id, author_name,
                        comment_id, content, reaction_type, timestamp, payload_cbor,
                        signature, received_at
                 FROM wall_social_events
                 WHERE actor_peer_id = ? AND timestamp > ? AND payload_cbor <> X'' AND signature <> X''
                 ORDER BY timestamp ASC, id ASC LIMIT ?",
            )?;
            let rows = stmt.query_map(params![actor_peer_id, after_timestamp, limit], Self::row_to_event)?;
            rows.collect()
        })
    }

    pub fn list_since(
        db: &Database,
        post_ids: &[String],
        after_timestamp: i64,
        limit: i64,
    ) -> SqliteResult<Vec<WallSocialEvent>> {
        if post_ids.is_empty() {
            return Ok(Vec::new());
        }
        db.with_connection(|conn| {
            let placeholders = crate::db::sql_utils::build_in_clause_placeholders(post_ids.len());
            let query = format!(
                "SELECT id, event_id, event_type, post_id, actor_peer_id, author_name,
                        comment_id, content, reaction_type, timestamp, payload_cbor,
                        signature, received_at
                 FROM wall_social_events
                 WHERE post_id IN ({}) AND timestamp > ?
                 ORDER BY timestamp ASC, id ASC LIMIT ?",
                placeholders
            );
            let mut params_vec: Vec<&dyn rusqlite::ToSql> =
                post_ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            params_vec.push(&after_timestamp);
            params_vec.push(&limit);
            let mut stmt = conn.prepare(&query)?;
            let rows =
                stmt.query_map(rusqlite::params_from_iter(params_vec), Self::row_to_event)?;
            rows.collect()
        })
    }

    fn row_to_event(row: &rusqlite::Row<'_>) -> SqliteResult<WallSocialEvent> {
        let event_type_str: String = row.get(2)?;
        Ok(WallSocialEvent {
            id: row.get(0)?,
            event_id: row.get(1)?,
            event_type: WallSocialEventType::parse_event_type(&event_type_str)
                .unwrap_or(WallSocialEventType::LegacyCommentCreate),
            post_id: row.get(3)?,
            actor_peer_id: row.get(4)?,
            author_name: row.get(5)?,
            comment_id: row.get(6)?,
            content: row.get(7)?,
            reaction_type: row.get(8)?,
            timestamp: row.get(9)?,
            payload_cbor: row.get(10)?,
            signature: row.get(11)?,
            received_at: row.get(12)?,
        })
    }
}
