//! Messages repository for storing and retrieving direct messages

use crate::db::Database;
use rusqlite::{params, Connection, Result as SqliteResult};

/// Message status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStatus {
    Pending,
    Sent,
    Delivered,
    Read,
    Failed,
}

impl MessageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageStatus::Pending => "pending",
            MessageStatus::Sent => "sent",
            MessageStatus::Delivered => "delivered",
            MessageStatus::Read => "read",
            MessageStatus::Failed => "failed",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(MessageStatus::Pending),
            "sent" => Some(MessageStatus::Sent),
            "delivered" => Some(MessageStatus::Delivered),
            "read" => Some(MessageStatus::Read),
            "failed" => Some(MessageStatus::Failed),
            _ => None,
        }
    }
}

/// A stored message
#[derive(Debug, Clone)]
pub struct Message {
    pub id: i64,
    pub message_id: String,
    pub conversation_id: String,
    pub sender_peer_id: String,
    pub recipient_peer_id: String,
    pub content_encrypted: Vec<u8>,
    pub content_type: String,
    pub reply_to_message_id: Option<String>,
    pub nonce_counter: u64,
    pub lamport_clock: i64,
    pub sent_at: i64,
    pub received_at: Option<i64>,
    pub delivered_at: Option<i64>,
    pub read_at: Option<i64>,
    pub status: String,
}

/// Data for inserting a new message
#[derive(Debug, Clone)]
pub struct MessageData {
    pub message_id: String,
    pub conversation_id: String,
    pub sender_peer_id: String,
    pub recipient_peer_id: String,
    pub content_encrypted: Vec<u8>,
    pub content_type: String,
    pub reply_to_message_id: Option<String>,
    pub nonce_counter: u64,
    pub lamport_clock: i64,
    pub sent_at: i64,
    pub received_at: Option<i64>,
    pub status: MessageStatus,
}

/// A conversation summary
#[derive(Debug, Clone)]
pub struct Conversation {
    pub conversation_id: String,
    pub peer_id: String,
    pub last_message_at: i64,
    pub last_message_preview: Option<String>,
    pub unread_count: i64,
}

/// Repository for message operations
pub struct MessagesRepository;

impl MessagesRepository {
    /// Insert a new message
    pub fn insert_message(db: &Database, msg: &MessageData) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO messages (
                    message_id, conversation_id, sender_peer_id, recipient_peer_id,
                    content_encrypted, content_type, reply_to_message_id, nonce_counter,
                    lamport_clock, sent_at, received_at, status
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    msg.message_id,
                    msg.conversation_id,
                    msg.sender_peer_id,
                    msg.recipient_peer_id,
                    msg.content_encrypted,
                    msg.content_type,
                    msg.reply_to_message_id,
                    msg.nonce_counter as i64,
                    msg.lamport_clock,
                    msg.sent_at,
                    msg.received_at,
                    msg.status.as_str(),
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Get a message by ID
    pub fn get_by_message_id(db: &Database, message_id: &str) -> SqliteResult<Option<Message>> {
        db.with_connection(|conn| Self::get_by_message_id_inner(conn, message_id))
    }

    fn get_by_message_id_inner(
        conn: &Connection,
        message_id: &str,
    ) -> SqliteResult<Option<Message>> {
        let mut stmt = conn.prepare(
            "SELECT id, message_id, conversation_id, sender_peer_id, recipient_peer_id,
                    content_encrypted, content_type, reply_to_message_id, nonce_counter,
                    lamport_clock, sent_at, received_at, delivered_at, read_at, status
             FROM messages WHERE message_id = ?",
        )?;

        let mut rows = stmt.query([message_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(Message {
                id: row.get(0)?,
                message_id: row.get(1)?,
                conversation_id: row.get(2)?,
                sender_peer_id: row.get(3)?,
                recipient_peer_id: row.get(4)?,
                content_encrypted: row.get(5)?,
                content_type: row.get(6)?,
                reply_to_message_id: row.get(7)?,
                nonce_counter: row.get::<_, i64>(8)? as u64,
                lamport_clock: row.get(9)?,
                sent_at: row.get(10)?,
                received_at: row.get(11)?,
                delivered_at: row.get(12)?,
                read_at: row.get(13)?,
                status: row.get(14)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get messages for a conversation
    pub fn get_conversation_messages(
        db: &Database,
        conversation_id: &str,
        limit: i64,
        before_timestamp: Option<i64>,
    ) -> SqliteResult<Vec<Message>> {
        db.with_connection(|conn| {
            // For pagination, we need to get the N most recent messages, then sort them ASC for display
            // When paginating (before_timestamp provided), get messages before that time
            let query = if before_timestamp.is_some() {
                "SELECT id, message_id, conversation_id, sender_peer_id, recipient_peer_id,
                        content_encrypted, content_type, reply_to_message_id, nonce_counter,
                        lamport_clock, sent_at, received_at, delivered_at, read_at, status
                 FROM (
                   SELECT * FROM messages
                   WHERE conversation_id = ? AND sent_at < ?
                   ORDER BY sent_at DESC
                   LIMIT ?
                 ) ORDER BY sent_at ASC"
            } else {
                "SELECT id, message_id, conversation_id, sender_peer_id, recipient_peer_id,
                        content_encrypted, content_type, reply_to_message_id, nonce_counter,
                        lamport_clock, sent_at, received_at, delivered_at, read_at, status
                 FROM (
                   SELECT * FROM messages
                   WHERE conversation_id = ?
                   ORDER BY sent_at DESC
                   LIMIT ?
                 ) ORDER BY sent_at ASC"
            };

            let mut stmt = conn.prepare(query)?;

            let rows = if let Some(before) = before_timestamp {
                stmt.query_map(
                    params![conversation_id, before, limit],
                    Self::row_to_message,
                )?
            } else {
                stmt.query_map(params![conversation_id, limit], Self::row_to_message)?
            };

            rows.collect()
        })
    }

    fn row_to_message(row: &rusqlite::Row) -> SqliteResult<Message> {
        Ok(Message {
            id: row.get(0)?,
            message_id: row.get(1)?,
            conversation_id: row.get(2)?,
            sender_peer_id: row.get(3)?,
            recipient_peer_id: row.get(4)?,
            content_encrypted: row.get(5)?,
            content_type: row.get(6)?,
            reply_to_message_id: row.get(7)?,
            nonce_counter: row.get::<_, i64>(8)? as u64,
            lamport_clock: row.get(9)?,
            sent_at: row.get(10)?,
            received_at: row.get(11)?,
            delivered_at: row.get(12)?,
            read_at: row.get(13)?,
            status: row.get(14)?,
        })
    }

    /// Update message status
    pub fn update_status(
        db: &Database,
        message_id: &str,
        status: MessageStatus,
    ) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let rows = conn.execute(
                "UPDATE messages SET status = ? WHERE message_id = ?",
                params![status.as_str(), message_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Mark message as delivered
    pub fn mark_delivered(db: &Database, message_id: &str, timestamp: i64) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let rows = conn.execute(
                "UPDATE messages SET status = 'delivered', delivered_at = ?
                 WHERE message_id = ? AND status IN ('pending', 'sent')",
                params![timestamp, message_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Mark message as read
    pub fn mark_read(db: &Database, message_id: &str, timestamp: i64) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let rows = conn.execute(
                "UPDATE messages SET status = 'read', read_at = ?
                 WHERE message_id = ? AND status IN ('pending', 'sent', 'delivered')",
                params![timestamp, message_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Mark all messages in a conversation as read
    pub fn mark_conversation_read(
        db: &Database,
        conversation_id: &str,
        our_peer_id: &str,
        timestamp: i64,
    ) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            let rows = conn.execute(
                "UPDATE messages SET status = 'read', read_at = ?
                 WHERE conversation_id = ? AND recipient_peer_id = ?
                   AND status IN ('delivered', 'sent')",
                params![timestamp, conversation_id, our_peer_id],
            )?;
            Ok(rows as i64)
        })
    }

    /// Get all conversations for a peer
    pub fn get_conversations(db: &Database, our_peer_id: &str) -> SqliteResult<Vec<Conversation>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    m.conversation_id,
                    CASE
                        WHEN m.sender_peer_id = ? THEN m.recipient_peer_id
                        ELSE m.sender_peer_id
                    END as peer_id,
                    MAX(m.sent_at) as last_message_at,
                    (SELECT COUNT(*) FROM messages m2
                     WHERE m2.conversation_id = m.conversation_id
                       AND m2.recipient_peer_id = ?
                       AND m2.status IN ('sent', 'delivered')) as unread_count
                 FROM messages m
                 WHERE m.sender_peer_id = ? OR m.recipient_peer_id = ?
                 GROUP BY m.conversation_id
                 ORDER BY last_message_at DESC",
            )?;

            let rows = stmt.query_map(
                params![our_peer_id, our_peer_id, our_peer_id, our_peer_id],
                |row| {
                    Ok(Conversation {
                        conversation_id: row.get(0)?,
                        peer_id: row.get(1)?,
                        last_message_at: row.get(2)?,
                        last_message_preview: None, // We don't store decrypted content
                        unread_count: row.get(3)?,
                    })
                },
            )?;

            rows.collect()
        })
    }

    /// Get unread count for a conversation
    pub fn get_unread_count(
        db: &Database,
        conversation_id: &str,
        our_peer_id: &str,
    ) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM messages
                 WHERE conversation_id = ? AND recipient_peer_id = ?
                   AND status IN ('sent', 'delivered')",
                params![conversation_id, our_peer_id],
                |row| row.get(0),
            )
        })
    }

    /// Get pending messages for a peer (for retry/sync)
    pub fn get_pending_messages(
        db: &Database,
        recipient_peer_id: &str,
    ) -> SqliteResult<Vec<Message>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, message_id, conversation_id, sender_peer_id, recipient_peer_id,
                        content_encrypted, content_type, reply_to_message_id, nonce_counter,
                        lamport_clock, sent_at, received_at, delivered_at, read_at, status
                 FROM messages
                 WHERE recipient_peer_id = ? AND status = 'pending'
                 ORDER BY sent_at ASC",
            )?;

            let rows = stmt.query_map([recipient_peer_id], Self::row_to_message)?;
            rows.collect()
        })
    }

    /// Check if a message exists
    pub fn message_exists(db: &Database, message_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE message_id = ?",
                [message_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Record a message event (for event sourcing)
    #[allow(clippy::too_many_arguments)]
    pub fn record_message_event(
        db: &Database,
        event_id: &str,
        event_type: &str,
        message_id: &str,
        conversation_id: &str,
        sender_peer_id: &str,
        recipient_peer_id: &str,
        lamport_clock: i64,
        timestamp: i64,
        payload_cbor: &[u8],
        signature: &[u8],
    ) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            let received_at = chrono::Utc::now().timestamp();
            conn.execute(
                "INSERT INTO message_events (
                    event_id, event_type, message_id, conversation_id,
                    sender_peer_id, recipient_peer_id, lamport_clock,
                    timestamp, payload_cbor, signature, received_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    event_id,
                    event_type,
                    message_id,
                    conversation_id,
                    sender_peer_id,
                    recipient_peer_id,
                    lamport_clock,
                    timestamp,
                    payload_cbor,
                    signature,
                    received_at,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Check if a message event exists (for deduplication)
    pub fn event_exists(db: &Database, event_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM message_events WHERE event_id = ?",
                [event_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Database {
        Database::in_memory().unwrap()
    }

    #[test]
    fn test_insert_and_get_message() {
        let db = create_test_db();

        let msg = MessageData {
            message_id: "msg-123".to_string(),
            conversation_id: "conv-456".to_string(),
            sender_peer_id: "peer-a".to_string(),
            recipient_peer_id: "peer-b".to_string(),
            content_encrypted: vec![1, 2, 3, 4],
            content_type: "text".to_string(),
            reply_to_message_id: None,
            nonce_counter: 1,
            lamport_clock: 1,
            sent_at: 1234567890,
            received_at: None,
            status: MessageStatus::Pending,
        };

        let id = MessagesRepository::insert_message(&db, &msg).unwrap();
        assert!(id > 0);

        let stored = MessagesRepository::get_by_message_id(&db, "msg-123")
            .unwrap()
            .unwrap();
        assert_eq!(stored.message_id, "msg-123");
        assert_eq!(stored.content_encrypted, vec![1, 2, 3, 4]);
        assert_eq!(stored.status, "pending");
    }

    #[test]
    fn test_mark_delivered_and_read() {
        let db = create_test_db();

        let msg = MessageData {
            message_id: "msg-456".to_string(),
            conversation_id: "conv-789".to_string(),
            sender_peer_id: "peer-a".to_string(),
            recipient_peer_id: "peer-b".to_string(),
            content_encrypted: vec![5, 6, 7, 8],
            content_type: "text".to_string(),
            reply_to_message_id: None,
            nonce_counter: 1,
            lamport_clock: 1,
            sent_at: 1234567890,
            received_at: None,
            status: MessageStatus::Sent,
        };

        MessagesRepository::insert_message(&db, &msg).unwrap();

        // Mark delivered
        let delivered = MessagesRepository::mark_delivered(&db, "msg-456", 1234567900).unwrap();
        assert!(delivered);

        let stored = MessagesRepository::get_by_message_id(&db, "msg-456")
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, "delivered");
        assert_eq!(stored.delivered_at, Some(1234567900));

        // Mark read
        let read = MessagesRepository::mark_read(&db, "msg-456", 1234567910).unwrap();
        assert!(read);

        let stored = MessagesRepository::get_by_message_id(&db, "msg-456")
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, "read");
        assert_eq!(stored.read_at, Some(1234567910));
    }

    #[test]
    fn test_get_conversations() {
        let db = create_test_db();

        // Insert messages in two conversations
        let msg1 = MessageData {
            message_id: "msg-1".to_string(),
            conversation_id: "conv-1".to_string(),
            sender_peer_id: "peer-a".to_string(),
            recipient_peer_id: "peer-b".to_string(),
            content_encrypted: vec![1],
            content_type: "text".to_string(),
            reply_to_message_id: None,
            nonce_counter: 1,
            lamport_clock: 1,
            sent_at: 1000,
            received_at: None,
            status: MessageStatus::Sent,
        };

        let msg2 = MessageData {
            message_id: "msg-2".to_string(),
            conversation_id: "conv-2".to_string(),
            sender_peer_id: "peer-c".to_string(),
            recipient_peer_id: "peer-a".to_string(),
            content_encrypted: vec![2],
            content_type: "text".to_string(),
            reply_to_message_id: None,
            nonce_counter: 1,
            lamport_clock: 1,
            sent_at: 2000,
            received_at: Some(2000),
            status: MessageStatus::Delivered,
        };

        MessagesRepository::insert_message(&db, &msg1).unwrap();
        MessagesRepository::insert_message(&db, &msg2).unwrap();

        let conversations = MessagesRepository::get_conversations(&db, "peer-a").unwrap();
        assert_eq!(conversations.len(), 2);

        // Should be ordered by last_message_at DESC
        assert_eq!(conversations[0].conversation_id, "conv-2");
        assert_eq!(conversations[0].peer_id, "peer-c");
        assert_eq!(conversations[0].unread_count, 1); // Unread from peer-c

        assert_eq!(conversations[1].conversation_id, "conv-1");
        assert_eq!(conversations[1].peer_id, "peer-b");
    }
}
