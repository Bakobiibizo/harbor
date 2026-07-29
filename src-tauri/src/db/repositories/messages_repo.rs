//! Messages repository for storing and retrieving direct messages

use crate::db::Database;
use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult, Transaction};
use thiserror::Error;

const MESSAGE_CRYPTO_V2: u16 = 2;
const MESSAGE_NONCE_ID_LEN: usize = 16;

/// Message status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStatus {
    Queued,
    Pending,
    Sent,
    Delivered,
    Read,
    Failed,
}

impl MessageStatus {
    #[allow(clippy::should_implement_trait)]
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageStatus::Queued => "queued",
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
            "queued" => Some(MessageStatus::Queued),
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
    pub protocol_version: u16,
    pub event_id: String,
    pub message_id: String,
    pub conversation_id: String,
    pub sender_peer_id: String,
    pub recipient_peer_id: String,
    pub nonce_id: Vec<u8>,
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
    pub protocol_version: u16,
    pub event_id: String,
    pub message_id: String,
    pub conversation_id: String,
    pub sender_peer_id: String,
    pub recipient_peer_id: String,
    pub nonce_id: Vec<u8>,
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
/// Parameters for recording a message event
pub struct RecordMessageEventParams<'a> {
    pub event_id: &'a str,
    pub event_type: &'a str,
    pub message_id: &'a str,
    pub conversation_id: &'a str,
    pub sender_peer_id: &'a str,
    pub recipient_peer_id: &'a str,
    pub lamport_clock: i64,
    pub timestamp: i64,
    pub payload_cbor: &'a [u8],
    pub signature: &'a [u8],
}

/// A verified protocol-v2 create event ready for one durable inbound commit.
///
/// Signature verification and authenticated decryption happen before this
/// boundary. Persistence still compares every immutable wire field so an
/// exact retransmission is idempotent while identity or replay collisions are
/// rejected.
pub struct IncomingMessageCommit<'a> {
    pub message: &'a MessageData,
    pub payload_cbor: &'a [u8],
    pub signature: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncomingMessageCommitOutcome {
    Inserted,
    Duplicate,
}

#[derive(Debug, Error)]
pub enum IncomingMessagePersistenceError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("incoming message is invalid: {0}")]
    InvalidMessage(&'static str),
    #[error("incoming message identity already identifies different signed content: {0}")]
    IdentityCollision(String),
    #[error("incoming message reuses a directional nonce")]
    NonceReplay,
    #[error("incoming message reuses a directional counter")]
    CounterReplay,
    #[error("integer value is too large for durable storage: {0}")]
    IntegerOverflow(&'static str),
}

#[derive(Debug)]
struct StoredCreateEvent {
    event_id: String,
    event_type: String,
    message_id: String,
    conversation_id: String,
    sender_peer_id: String,
    recipient_peer_id: String,
    lamport_clock: i64,
    timestamp: i64,
    payload_cbor: Vec<u8>,
    signature: Vec<u8>,
}

/// A signed, encrypted, immutable direct-message edit event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageEditEvent {
    pub id: i64,
    pub event_id: String,
    pub protocol_version: u16,
    pub message_id: String,
    pub conversation_id: String,
    pub author_peer_id: String,
    pub recipient_peer_id: String,
    pub revision: u64,
    pub nonce_id: Vec<u8>,
    pub nonce_counter: u64,
    pub lamport_clock: u64,
    pub encrypted_content: Vec<u8>,
    pub signature: Vec<u8>,
    pub timestamp: i64,
    pub received_at: i64,
}

/// Verified edit-event data ready for durable application.
///
/// Authentication and decryption happen before this boundary. The repository
/// still binds every field to the immutable original message and refuses to
/// move the current revision backwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageEditEventData {
    pub event_id: String,
    pub protocol_version: u16,
    pub message_id: String,
    pub conversation_id: String,
    pub author_peer_id: String,
    pub recipient_peer_id: String,
    pub revision: u64,
    pub nonce_id: Vec<u8>,
    pub nonce_counter: u64,
    pub lamport_clock: u64,
    pub encrypted_content: Vec<u8>,
    pub signature: Vec<u8>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditEventApplyOutcome {
    Inserted,
    Duplicate,
}

#[derive(Debug, Error)]
pub enum MessageEditPersistenceError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("message not found: {0}")]
    MessageNotFound(String),
    #[error("edit event is invalid: {0}")]
    InvalidEvent(&'static str),
    #[error("edit event parties or conversation do not match the original message")]
    MessageBindingMismatch,
    #[error("edit event id already identifies different signed content: {0}")]
    EventIdCollision(String),
    #[error("edit revision {attempted} is not newer than durable revision {current}")]
    StaleRevision { attempted: u64, current: u64 },
    #[error("edit revision {revision} is already occupied by a different event")]
    RevisionConflict { revision: u64 },
    #[error("integer value is too large for durable storage: {0}")]
    IntegerOverflow(&'static str),
}

pub struct MessagesRepository;

impl MessagesRepository {
    /// Insert a new message
    pub fn insert_message(db: &Database, msg: &MessageData) -> SqliteResult<i64> {
        db.with_connection_mut(|conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO message_crypto_nonces (
                    author_peer_id, recipient_peer_id, nonce_id, nonce_counter,
                    event_id, event_kind, recorded_at
                 ) VALUES (?, ?, ?, ?, ?, 'create', ?)",
                params![
                    msg.sender_peer_id,
                    msg.recipient_peer_id,
                    msg.nonce_id,
                    msg.nonce_counter as i64,
                    msg.event_id,
                    chrono::Utc::now().timestamp(),
                ],
            )?;
            tx.execute(
                "INSERT INTO messages (
                    protocol_version, event_id, message_id, conversation_id,
                    sender_peer_id, recipient_peer_id, nonce_id, content_encrypted,
                    content_type, reply_to_message_id, nonce_counter,
                    lamport_clock, sent_at, received_at, status
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    msg.protocol_version,
                    msg.event_id,
                    msg.message_id,
                    msg.conversation_id,
                    msg.sender_peer_id,
                    msg.recipient_peer_id,
                    msg.nonce_id,
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
            let id = tx.last_insert_rowid();
            tx.commit()?;
            Ok(id)
        })
    }

    /// Atomically persist a verified incoming create event.
    ///
    /// The directional nonce/counter claim, materialized message, immutable
    /// event, and Lamport-clock advance are one commit. An exact retransmission
    /// returns [`IncomingMessageCommitOutcome::Duplicate`] without writing.
    pub fn commit_incoming_message(
        db: &Database,
        incoming: &IncomingMessageCommit<'_>,
    ) -> Result<IncomingMessageCommitOutcome, IncomingMessagePersistenceError> {
        Self::validate_incoming_message(incoming)?;
        let msg = incoming.message;
        let nonce_counter = Self::incoming_to_i64(msg.nonce_counter, "nonce_counter")?;

        db.with_connection_mut_result(|conn| {
            let tx = conn.transaction()?;

            let by_message_id = Self::get_by_message_id_inner(&tx, &msg.message_id)?;
            let by_event_id = Self::get_message_by_event_id_inner(&tx, &msg.event_id)?;
            let stored_event = Self::get_create_event_inner(&tx, &msg.event_id)?;

            if let Some(existing) = by_message_id.as_ref() {
                let same_materialized = Self::same_incoming_message(existing, msg);
                let same_event = stored_event
                    .as_ref()
                    .is_some_and(|event| Self::same_incoming_create_event(event, incoming));
                let event_points_to_same_row = by_event_id
                    .as_ref()
                    .is_some_and(|event_message| event_message.message_id == existing.message_id);
                if same_materialized && same_event && event_points_to_same_row {
                    // Deliberately commit no writes. This remains a valid exact
                    // duplicate after restart or after local read-state changes.
                    tx.commit()?;
                    return Ok(IncomingMessageCommitOutcome::Duplicate);
                }
                return Err(IncomingMessagePersistenceError::IdentityCollision(
                    msg.message_id.clone(),
                ));
            }

            if by_event_id.is_some() || stored_event.is_some() {
                return Err(IncomingMessagePersistenceError::IdentityCollision(
                    msg.event_id.clone(),
                ));
            }

            let nonce_owner: Option<String> = tx
                .query_row(
                    "SELECT event_id FROM message_crypto_nonces
                     WHERE author_peer_id = ? AND recipient_peer_id = ? AND nonce_id = ?",
                    params![msg.sender_peer_id, msg.recipient_peer_id, msg.nonce_id],
                    |row| row.get(0),
                )
                .optional()?;
            if nonce_owner.is_some() {
                return Err(IncomingMessagePersistenceError::NonceReplay);
            }

            let counter_owner: Option<String> = tx
                .query_row(
                    "SELECT event_id FROM message_crypto_nonces
                     WHERE author_peer_id = ? AND recipient_peer_id = ? AND nonce_counter = ?",
                    params![msg.sender_peer_id, msg.recipient_peer_id, nonce_counter],
                    |row| row.get(0),
                )
                .optional()?;
            if counter_owner.is_some() {
                return Err(IncomingMessagePersistenceError::CounterReplay);
            }

            let received_at =
                msg.received_at
                    .ok_or(IncomingMessagePersistenceError::InvalidMessage(
                        "received_at must be set",
                    ))?;
            tx.execute(
                "INSERT INTO message_crypto_nonces (
                    author_peer_id, recipient_peer_id, nonce_id, nonce_counter,
                    event_id, event_kind, recorded_at
                 ) VALUES (?, ?, ?, ?, ?, 'create', ?)",
                params![
                    msg.sender_peer_id,
                    msg.recipient_peer_id,
                    msg.nonce_id,
                    nonce_counter,
                    msg.event_id,
                    received_at,
                ],
            )?;
            tx.execute(
                "INSERT INTO messages (
                    protocol_version, event_id, message_id, conversation_id,
                    sender_peer_id, recipient_peer_id, nonce_id, content_encrypted,
                    content_type, reply_to_message_id, nonce_counter,
                    lamport_clock, sent_at, received_at, status
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    msg.protocol_version,
                    msg.event_id,
                    msg.message_id,
                    msg.conversation_id,
                    msg.sender_peer_id,
                    msg.recipient_peer_id,
                    msg.nonce_id,
                    msg.content_encrypted,
                    msg.content_type,
                    msg.reply_to_message_id,
                    nonce_counter,
                    msg.lamport_clock,
                    msg.sent_at,
                    received_at,
                    msg.status.as_str(),
                ],
            )?;
            tx.execute(
                "INSERT INTO message_events (
                    event_id, event_type, message_id, conversation_id,
                    sender_peer_id, recipient_peer_id, lamport_clock,
                    timestamp, payload_cbor, signature, received_at
                 ) VALUES (?, 'received', ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    msg.event_id,
                    msg.message_id,
                    msg.conversation_id,
                    msg.sender_peer_id,
                    msg.recipient_peer_id,
                    msg.lamport_clock,
                    msg.sent_at,
                    incoming.payload_cbor,
                    incoming.signature,
                    received_at,
                ],
            )?;
            tx.execute(
                "INSERT INTO lamport_clocks (author_peer_id, current_value) VALUES (?, ?)
                 ON CONFLICT(author_peer_id) DO UPDATE SET
                    current_value = MAX(current_value, excluded.current_value)",
                params![msg.sender_peer_id, msg.lamport_clock],
            )?;

            tx.commit()?;
            Ok(IncomingMessageCommitOutcome::Inserted)
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
            "SELECT id, protocol_version, event_id, message_id, conversation_id,
                    sender_peer_id, recipient_peer_id, nonce_id, content_encrypted,
                    content_type, reply_to_message_id, nonce_counter,
                    lamport_clock, sent_at, received_at, delivered_at, read_at, status
             FROM messages WHERE message_id = ?",
        )?;

        let mut rows = stmt.query([message_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(Message {
                id: row.get(0)?,
                protocol_version: row.get(1)?,
                event_id: row.get(2)?,
                message_id: row.get(3)?,
                conversation_id: row.get(4)?,
                sender_peer_id: row.get(5)?,
                recipient_peer_id: row.get(6)?,
                nonce_id: row.get(7)?,
                content_encrypted: row.get(8)?,
                content_type: row.get(9)?,
                reply_to_message_id: row.get(10)?,
                nonce_counter: row.get::<_, i64>(11)? as u64,
                lamport_clock: row.get(12)?,
                sent_at: row.get(13)?,
                received_at: row.get(14)?,
                delivered_at: row.get(15)?,
                read_at: row.get(16)?,
                status: row.get(17)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn get_message_by_event_id_inner(
        conn: &Connection,
        event_id: &str,
    ) -> SqliteResult<Option<Message>> {
        conn.query_row(
            "SELECT id, protocol_version, event_id, message_id, conversation_id,
                    sender_peer_id, recipient_peer_id, nonce_id, content_encrypted,
                    content_type, reply_to_message_id, nonce_counter,
                    lamport_clock, sent_at, received_at, delivered_at, read_at, status
             FROM messages WHERE event_id = ?",
            [event_id],
            Self::row_to_message,
        )
        .optional()
    }

    fn get_create_event_inner(
        conn: &Connection,
        event_id: &str,
    ) -> SqliteResult<Option<StoredCreateEvent>> {
        conn.query_row(
            "SELECT event_id, event_type, message_id, conversation_id,
                    sender_peer_id, recipient_peer_id, lamport_clock,
                    timestamp, payload_cbor, signature
             FROM message_events WHERE event_id = ?",
            [event_id],
            |row| {
                Ok(StoredCreateEvent {
                    event_id: row.get(0)?,
                    event_type: row.get(1)?,
                    message_id: row.get(2)?,
                    conversation_id: row.get(3)?,
                    sender_peer_id: row.get(4)?,
                    recipient_peer_id: row.get(5)?,
                    lamport_clock: row.get(6)?,
                    timestamp: row.get(7)?,
                    payload_cbor: row.get(8)?,
                    signature: row.get(9)?,
                })
            },
        )
        .optional()
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
                "SELECT id, protocol_version, event_id, message_id, conversation_id,
                        sender_peer_id, recipient_peer_id, nonce_id, content_encrypted,
                        content_type, reply_to_message_id, nonce_counter,
                        lamport_clock, sent_at, received_at, delivered_at, read_at, status
                 FROM (
                   SELECT * FROM messages
                   WHERE conversation_id = ? AND sent_at < ?
                   ORDER BY sent_at DESC
                   LIMIT ?
                 ) ORDER BY sent_at ASC"
            } else {
                "SELECT id, protocol_version, event_id, message_id, conversation_id,
                        sender_peer_id, recipient_peer_id, nonce_id, content_encrypted,
                        content_type, reply_to_message_id, nonce_counter,
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
            protocol_version: row.get(1)?,
            event_id: row.get(2)?,
            message_id: row.get(3)?,
            conversation_id: row.get(4)?,
            sender_peer_id: row.get(5)?,
            recipient_peer_id: row.get(6)?,
            nonce_id: row.get(7)?,
            content_encrypted: row.get(8)?,
            content_type: row.get(9)?,
            reply_to_message_id: row.get(10)?,
            nonce_counter: row.get::<_, i64>(11)? as u64,
            lamport_clock: row.get(12)?,
            sent_at: row.get(13)?,
            received_at: row.get(14)?,
            delivered_at: row.get(15)?,
            read_at: row.get(16)?,
            status: row.get(17)?,
        })
    }

    fn validate_incoming_message(
        incoming: &IncomingMessageCommit<'_>,
    ) -> Result<(), IncomingMessagePersistenceError> {
        let msg = incoming.message;
        if msg.protocol_version != MESSAGE_CRYPTO_V2 {
            return Err(IncomingMessagePersistenceError::InvalidMessage(
                "protocol_version must be 2",
            ));
        }
        for (value, field) in [
            (&msg.event_id, "event_id"),
            (&msg.message_id, "message_id"),
            (&msg.conversation_id, "conversation_id"),
            (&msg.sender_peer_id, "sender_peer_id"),
            (&msg.recipient_peer_id, "recipient_peer_id"),
            (&msg.content_type, "content_type"),
        ] {
            if value.trim().is_empty() {
                return Err(IncomingMessagePersistenceError::InvalidMessage(field));
            }
        }
        if msg.event_id != msg.message_id {
            return Err(IncomingMessagePersistenceError::InvalidMessage(
                "event_id must equal message_id for create events",
            ));
        }
        if msg.sender_peer_id == msg.recipient_peer_id {
            return Err(IncomingMessagePersistenceError::InvalidMessage(
                "sender and recipient must differ",
            ));
        }
        if msg.nonce_id.len() != MESSAGE_NONCE_ID_LEN {
            return Err(IncomingMessagePersistenceError::InvalidMessage(
                "nonce_id must be exactly 16 bytes",
            ));
        }
        if msg.nonce_counter == 0 {
            return Err(IncomingMessagePersistenceError::InvalidMessage(
                "nonce_counter must be non-zero",
            ));
        }
        if msg.lamport_clock <= 0 {
            return Err(IncomingMessagePersistenceError::InvalidMessage(
                "lamport_clock must be positive",
            ));
        }
        if msg.content_encrypted.is_empty() {
            return Err(IncomingMessagePersistenceError::InvalidMessage(
                "content_encrypted must not be empty",
            ));
        }
        if msg.received_at.is_none() {
            return Err(IncomingMessagePersistenceError::InvalidMessage(
                "received_at must be set",
            ));
        }
        if msg.status != MessageStatus::Delivered {
            return Err(IncomingMessagePersistenceError::InvalidMessage(
                "incoming message status must be delivered",
            ));
        }
        if incoming.payload_cbor.is_empty() {
            return Err(IncomingMessagePersistenceError::InvalidMessage(
                "payload_cbor must not be empty",
            ));
        }
        if incoming.signature.is_empty() {
            return Err(IncomingMessagePersistenceError::InvalidMessage(
                "signature must not be empty",
            ));
        }
        Self::incoming_to_i64(msg.nonce_counter, "nonce_counter")?;
        Ok(())
    }

    fn same_incoming_message(stored: &Message, incoming: &MessageData) -> bool {
        stored.protocol_version == incoming.protocol_version
            && stored.event_id == incoming.event_id
            && stored.message_id == incoming.message_id
            && stored.conversation_id == incoming.conversation_id
            && stored.sender_peer_id == incoming.sender_peer_id
            && stored.recipient_peer_id == incoming.recipient_peer_id
            && stored.nonce_id == incoming.nonce_id
            && stored.content_encrypted == incoming.content_encrypted
            && stored.content_type == incoming.content_type
            && stored.reply_to_message_id == incoming.reply_to_message_id
            && stored.nonce_counter == incoming.nonce_counter
            && stored.lamport_clock == incoming.lamport_clock
            && stored.sent_at == incoming.sent_at
    }

    fn same_incoming_create_event(
        stored: &StoredCreateEvent,
        incoming: &IncomingMessageCommit<'_>,
    ) -> bool {
        let msg = incoming.message;
        stored.event_id == msg.event_id
            && stored.event_type == "received"
            && stored.message_id == msg.message_id
            && stored.conversation_id == msg.conversation_id
            && stored.sender_peer_id == msg.sender_peer_id
            && stored.recipient_peer_id == msg.recipient_peer_id
            && stored.lamport_clock == msg.lamport_clock
            && stored.timestamp == msg.sent_at
            && stored.payload_cbor == incoming.payload_cbor
            && stored.signature == incoming.signature
    }

    fn incoming_to_i64(
        value: u64,
        field: &'static str,
    ) -> Result<i64, IncomingMessagePersistenceError> {
        i64::try_from(value).map_err(|_| IncomingMessagePersistenceError::IntegerOverflow(field))
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
                "SELECT id, protocol_version, event_id, message_id, conversation_id,
                        sender_peer_id, recipient_peer_id, nonce_id, content_encrypted,
                        content_type, reply_to_message_id, nonce_counter,
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
    pub fn record_message_event(
        db: &Database,
        params: &RecordMessageEventParams<'_>,
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
                    params.event_id,
                    params.event_type,
                    params.message_id,
                    params.conversation_id,
                    params.sender_peer_id,
                    params.recipient_peer_id,
                    params.lamport_clock,
                    params.timestamp,
                    params.payload_cbor,
                    params.signature,
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

    /// Reserve the next edit revision in its own transaction.
    ///
    /// Callers must reserve before encrypting. The reservation deliberately is
    /// not rolled back if encryption, signing, transport, or event persistence
    /// later fails, preventing a revision from being reused after a retry.
    pub fn reserve_edit_revision(
        db: &Database,
        message_id: &str,
        author_peer_id: &str,
        recipient_peer_id: &str,
    ) -> Result<u64, MessageEditPersistenceError> {
        Self::validate_identifier(message_id, "message_id")?;
        Self::validate_identifier(author_peer_id, "author_peer_id")?;
        Self::validate_identifier(recipient_peer_id, "recipient_peer_id")?;

        db.with_connection_mut_result(|conn| {
            let tx = conn.transaction()?;
            Self::verify_original_binding(
                &tx,
                message_id,
                None,
                author_peer_id,
                recipient_peer_id,
            )?;

            let durable_event_revision: Option<u64> = tx.query_row(
                "SELECT MAX(revision) FROM message_edit_events WHERE message_id = ?",
                [message_id],
                |row| row.get(0),
            )?;
            let counter: Option<(String, String, u64)> = tx
                .query_row(
                    "SELECT author_peer_id, recipient_peer_id, last_reserved_revision
                     FROM message_edit_revision_counters WHERE message_id = ?",
                    [message_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;

            if let Some((stored_author, stored_recipient, _)) = &counter {
                if stored_author != author_peer_id || stored_recipient != recipient_peer_id {
                    return Err(MessageEditPersistenceError::MessageBindingMismatch);
                }
            }

            let last_reserved = counter.map(|(_, _, revision)| revision).unwrap_or(0);
            let next = last_reserved
                .max(durable_event_revision.unwrap_or(0))
                .checked_add(1)
                .ok_or(MessageEditPersistenceError::IntegerOverflow("revision"))?;
            let next_i64 = Self::to_i64(next, "revision")?;

            tx.execute(
                "INSERT INTO message_edit_revision_counters (
                    message_id, author_peer_id, recipient_peer_id,
                    last_reserved_revision, updated_at
                 ) VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(message_id) DO UPDATE SET
                    last_reserved_revision = excluded.last_reserved_revision,
                    updated_at = excluded.updated_at",
                params![
                    message_id,
                    author_peer_id,
                    recipient_peer_id,
                    next_i64,
                    chrono::Utc::now().timestamp(),
                ],
            )?;
            tx.commit()?;
            Ok(next)
        })
    }

    /// Atomically append a verified edit and advance its materialized head.
    ///
    /// Equal event IDs are idempotent only when every signed/encrypted field is
    /// identical. Any older revision, same-revision substitution, party swap,
    /// or nonce reuse fails without changing either the event ledger or head.
    pub fn record_verified_edit_event(
        db: &Database,
        event: &MessageEditEventData,
    ) -> Result<EditEventApplyOutcome, MessageEditPersistenceError> {
        db.with_connection_mut_result(|conn| {
            let tx = conn.transaction()?;
            let outcome = Self::record_verified_edit_event_tx(&tx, event)?;
            tx.commit()?;
            Ok(outcome)
        })
    }

    /// Apply a verified edit inside a caller-owned transaction. This is used
    /// by the durable outbox so the immutable edit and its exact wire bytes
    /// cannot be committed independently.
    pub(crate) fn record_verified_edit_event_tx(
        tx: &Transaction<'_>,
        event: &MessageEditEventData,
    ) -> Result<EditEventApplyOutcome, MessageEditPersistenceError> {
        Self::validate_edit_event(event)?;
        let revision = Self::to_i64(event.revision, "revision")?;
        let nonce_counter = Self::to_i64(event.nonce_counter, "nonce_counter")?;
        let lamport_clock = Self::to_i64(event.lamport_clock, "lamport_clock")?;

        Self::verify_original_binding(
            tx,
            &event.message_id,
            Some(&event.conversation_id),
            &event.author_peer_id,
            &event.recipient_peer_id,
        )?;
        let counter_parties: Option<(String, String)> = tx
            .query_row(
                "SELECT author_peer_id, recipient_peer_id
                     FROM message_edit_revision_counters WHERE message_id = ?",
                [&event.message_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if counter_parties.is_some_and(|(author, recipient)| {
            author != event.author_peer_id || recipient != event.recipient_peer_id
        }) {
            return Err(MessageEditPersistenceError::MessageBindingMismatch);
        }

        if let Some(existing) = Self::get_edit_event_by_id_inner(tx, &event.event_id)? {
            if Self::same_edit_event(&existing, event) {
                return Ok(EditEventApplyOutcome::Duplicate);
            }
            return Err(MessageEditPersistenceError::EventIdCollision(
                event.event_id.clone(),
            ));
        }

        let current_revision: Option<u64> = tx
            .query_row(
                "SELECT revision FROM message_edit_heads WHERE message_id = ?",
                [&event.message_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(current) = current_revision {
            if event.revision <= current {
                return Err(MessageEditPersistenceError::StaleRevision {
                    attempted: event.revision,
                    current,
                });
            }
        }

        let occupied: Option<String> = tx
            .query_row(
                "SELECT event_id FROM message_edit_events
                     WHERE message_id = ? AND revision = ?",
                params![event.message_id, revision],
                |row| row.get(0),
            )
            .optional()?;
        if occupied.is_some() {
            return Err(MessageEditPersistenceError::RevisionConflict {
                revision: event.revision,
            });
        }

        let received_at = chrono::Utc::now().timestamp();
        tx.execute(
            "INSERT INTO message_crypto_nonces (
                    author_peer_id, recipient_peer_id, nonce_id, nonce_counter,
                    event_id, event_kind, recorded_at
                 ) VALUES (?, ?, ?, ?, ?, 'edit', ?)",
            params![
                event.author_peer_id,
                event.recipient_peer_id,
                event.nonce_id,
                nonce_counter,
                event.event_id,
                received_at,
            ],
        )?;
        tx.execute(
            "INSERT INTO message_edit_events (
                    event_id, protocol_version, message_id, conversation_id,
                    author_peer_id, recipient_peer_id, revision, nonce_id,
                    nonce_counter, lamport_clock, encrypted_content, signature,
                    timestamp, received_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                event.event_id,
                event.protocol_version,
                event.message_id,
                event.conversation_id,
                event.author_peer_id,
                event.recipient_peer_id,
                revision,
                event.nonce_id,
                nonce_counter,
                lamport_clock,
                event.encrypted_content,
                event.signature,
                event.timestamp,
                received_at,
            ],
        )?;

        tx.execute(
            "INSERT INTO message_edit_heads (
                    message_id, event_id, author_peer_id, recipient_peer_id,
                    revision, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT(message_id) DO UPDATE SET
                    event_id = excluded.event_id,
                    author_peer_id = excluded.author_peer_id,
                    recipient_peer_id = excluded.recipient_peer_id,
                    revision = excluded.revision,
                    updated_at = excluded.updated_at
                 WHERE excluded.revision > message_edit_heads.revision",
            params![
                event.message_id,
                event.event_id,
                event.author_peer_id,
                event.recipient_peer_id,
                revision,
                received_at,
            ],
        )?;

        // Receiving an event can reveal a revision newer than our restored
        // local counter. Raise the durable floor, never lower it.
        tx.execute(
            "INSERT INTO message_edit_revision_counters (
                    message_id, author_peer_id, recipient_peer_id,
                    last_reserved_revision, updated_at
                 ) VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(message_id) DO UPDATE SET
                    last_reserved_revision = MAX(
                        message_edit_revision_counters.last_reserved_revision,
                        excluded.last_reserved_revision
                    ),
                    updated_at = excluded.updated_at",
            params![
                event.message_id,
                event.author_peer_id,
                event.recipient_peer_id,
                revision,
                received_at,
            ],
        )?;

        Ok(EditEventApplyOutcome::Inserted)
    }

    pub fn get_current_edit_event(
        db: &Database,
        message_id: &str,
    ) -> Result<Option<MessageEditEvent>, MessageEditPersistenceError> {
        Ok(db.with_connection(|conn| {
            conn.query_row(
                "SELECT e.id, e.event_id, e.protocol_version, e.message_id,
                        e.conversation_id, e.author_peer_id, e.recipient_peer_id,
                        e.revision, e.nonce_id, e.nonce_counter, e.lamport_clock,
                        e.encrypted_content, e.signature, e.timestamp, e.received_at
                 FROM message_edit_heads h
                 JOIN message_edit_events e ON e.event_id = h.event_id
                 WHERE h.message_id = ?",
                [message_id],
                Self::row_to_edit_event,
            )
            .optional()
        })?)
    }

    /// Return the immutable signature associated with a stored create event.
    ///
    /// The materialized `messages` row deliberately does not duplicate the
    /// signature. Readers use this narrow lookup to authenticate that row
    /// before displaying its decrypted contents.
    pub fn get_create_event_signature(
        db: &Database,
        event_id: &str,
    ) -> SqliteResult<Option<Vec<u8>>> {
        db.with_connection(|conn| {
            conn.query_row(
                "SELECT signature FROM message_events WHERE event_id = ?",
                [event_id],
                |row| row.get(0),
            )
            .optional()
        })
    }

    pub fn list_edit_events(
        db: &Database,
        message_id: &str,
    ) -> Result<Vec<MessageEditEvent>, MessageEditPersistenceError> {
        Ok(db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, event_id, protocol_version, message_id,
                        conversation_id, author_peer_id, recipient_peer_id,
                        revision, nonce_id, nonce_counter, lamport_clock,
                        encrypted_content, signature, timestamp, received_at
                 FROM message_edit_events WHERE message_id = ?
                 ORDER BY revision ASC",
            )?;
            let rows = stmt.query_map([message_id], Self::row_to_edit_event)?;
            rows.collect()
        })?)
    }

    fn validate_edit_event(
        event: &MessageEditEventData,
    ) -> Result<(), MessageEditPersistenceError> {
        if event.protocol_version != MESSAGE_CRYPTO_V2 {
            return Err(MessageEditPersistenceError::InvalidEvent(
                "protocol_version must be 2",
            ));
        }
        Self::validate_identifier(&event.event_id, "event_id")?;
        Self::validate_identifier(&event.message_id, "message_id")?;
        Self::validate_identifier(&event.conversation_id, "conversation_id")?;
        Self::validate_identifier(&event.author_peer_id, "author_peer_id")?;
        Self::validate_identifier(&event.recipient_peer_id, "recipient_peer_id")?;
        if event.author_peer_id == event.recipient_peer_id {
            return Err(MessageEditPersistenceError::InvalidEvent(
                "author and recipient must differ",
            ));
        }
        if event.revision == 0 {
            return Err(MessageEditPersistenceError::InvalidEvent(
                "revision must be non-zero",
            ));
        }
        if event.nonce_id.len() != MESSAGE_NONCE_ID_LEN {
            return Err(MessageEditPersistenceError::InvalidEvent(
                "nonce_id must be exactly 16 bytes",
            ));
        }
        if event.nonce_counter == 0 {
            return Err(MessageEditPersistenceError::InvalidEvent(
                "nonce_counter must be non-zero",
            ));
        }
        if event.lamport_clock == 0 {
            return Err(MessageEditPersistenceError::InvalidEvent(
                "lamport_clock must be non-zero",
            ));
        }
        if event.encrypted_content.is_empty() {
            return Err(MessageEditPersistenceError::InvalidEvent(
                "encrypted_content must not be empty",
            ));
        }
        if event.signature.is_empty() {
            return Err(MessageEditPersistenceError::InvalidEvent(
                "signature must not be empty",
            ));
        }
        Self::to_i64(event.revision, "revision")?;
        Self::to_i64(event.nonce_counter, "nonce_counter")?;
        Self::to_i64(event.lamport_clock, "lamport_clock")?;
        Ok(())
    }

    fn validate_identifier(
        value: &str,
        field: &'static str,
    ) -> Result<(), MessageEditPersistenceError> {
        if value.trim().is_empty() {
            return Err(MessageEditPersistenceError::InvalidEvent(field));
        }
        Ok(())
    }

    fn verify_original_binding(
        tx: &Transaction<'_>,
        message_id: &str,
        conversation_id: Option<&str>,
        author_peer_id: &str,
        recipient_peer_id: &str,
    ) -> Result<(), MessageEditPersistenceError> {
        let original: Option<(String, String, String)> = tx
            .query_row(
                "SELECT conversation_id, sender_peer_id, recipient_peer_id
                 FROM messages WHERE message_id = ?",
                [message_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((stored_conversation, stored_author, stored_recipient)) = original else {
            return Err(MessageEditPersistenceError::MessageNotFound(
                message_id.to_string(),
            ));
        };
        if conversation_id.is_some_and(|value| value != stored_conversation)
            || stored_author != author_peer_id
            || stored_recipient != recipient_peer_id
        {
            return Err(MessageEditPersistenceError::MessageBindingMismatch);
        }
        Ok(())
    }

    fn get_edit_event_by_id_inner(
        conn: &Connection,
        event_id: &str,
    ) -> SqliteResult<Option<MessageEditEvent>> {
        conn.query_row(
            "SELECT id, event_id, protocol_version, message_id,
                    conversation_id, author_peer_id, recipient_peer_id,
                    revision, nonce_id, nonce_counter, lamport_clock,
                    encrypted_content, signature, timestamp, received_at
             FROM message_edit_events WHERE event_id = ?",
            [event_id],
            Self::row_to_edit_event,
        )
        .optional()
    }

    fn row_to_edit_event(row: &rusqlite::Row<'_>) -> SqliteResult<MessageEditEvent> {
        Ok(MessageEditEvent {
            id: row.get(0)?,
            event_id: row.get(1)?,
            protocol_version: row.get(2)?,
            message_id: row.get(3)?,
            conversation_id: row.get(4)?,
            author_peer_id: row.get(5)?,
            recipient_peer_id: row.get(6)?,
            revision: row.get(7)?,
            nonce_id: row.get(8)?,
            nonce_counter: row.get(9)?,
            lamport_clock: row.get(10)?,
            encrypted_content: row.get(11)?,
            signature: row.get(12)?,
            timestamp: row.get(13)?,
            received_at: row.get(14)?,
        })
    }

    fn same_edit_event(stored: &MessageEditEvent, event: &MessageEditEventData) -> bool {
        stored.event_id == event.event_id
            && stored.protocol_version == event.protocol_version
            && stored.message_id == event.message_id
            && stored.conversation_id == event.conversation_id
            && stored.author_peer_id == event.author_peer_id
            && stored.recipient_peer_id == event.recipient_peer_id
            && stored.revision == event.revision
            && stored.nonce_id == event.nonce_id
            && stored.nonce_counter == event.nonce_counter
            && stored.lamport_clock == event.lamport_clock
            && stored.encrypted_content == event.encrypted_content
            && stored.signature == event.signature
            && stored.timestamp == event.timestamp
    }

    fn to_i64(value: u64, field: &'static str) -> Result<i64, MessageEditPersistenceError> {
        i64::try_from(value).map_err(|_| MessageEditPersistenceError::IntegerOverflow(field))
    }

    /// Delete all messages in a conversation (clear history)
    pub fn clear_conversation_messages(db: &Database, conversation_id: &str) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            let rows = conn.execute(
                "DELETE FROM messages WHERE conversation_id = ?",
                params![conversation_id],
            )?;
            Ok(rows as i64)
        })
    }

    /// Delete a conversation and all its messages
    pub fn delete_conversation(db: &Database, conversation_id: &str) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            // Delete message events first (referential integrity)
            conn.execute(
                "DELETE FROM message_events WHERE conversation_id = ?",
                params![conversation_id],
            )?;
            // Delete all messages in the conversation
            let rows = conn.execute(
                "DELETE FROM messages WHERE conversation_id = ?",
                params![conversation_id],
            )?;
            Ok(rows as i64)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Database {
        Database::in_memory().unwrap()
    }

    fn incoming_message(
        id: &str,
        sender: &str,
        recipient: &str,
        nonce_byte: u8,
        nonce_counter: u64,
    ) -> MessageData {
        MessageData {
            protocol_version: 2,
            event_id: id.to_string(),
            message_id: id.to_string(),
            conversation_id: format!("conv:{sender}:{recipient}"),
            sender_peer_id: sender.to_string(),
            recipient_peer_id: recipient.to_string(),
            nonce_id: vec![nonce_byte; 16],
            content_encrypted: vec![nonce_byte, 42, 99],
            content_type: "text".to_string(),
            reply_to_message_id: None,
            nonce_counter,
            lamport_clock: nonce_counter as i64 + 10,
            sent_at: 1_700_000_000 + nonce_counter as i64,
            received_at: Some(1_700_000_100 + nonce_counter as i64),
            status: MessageStatus::Delivered,
        }
    }

    fn commit_incoming(
        db: &Database,
        message: &MessageData,
        payload: &[u8],
        signature: &[u8],
    ) -> Result<IncomingMessageCommitOutcome, IncomingMessagePersistenceError> {
        MessagesRepository::commit_incoming_message(
            db,
            &IncomingMessageCommit {
                message,
                payload_cbor: payload,
                signature,
            },
        )
    }

    fn ingress_row_counts(db: &Database) -> (i64, i64, i64) {
        db.with_connection(|conn| {
            Ok((
                conn.query_row("SELECT COUNT(*) FROM message_crypto_nonces", [], |row| {
                    row.get(0)
                })?,
                conn.query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))?,
                conn.query_row("SELECT COUNT(*) FROM message_events", [], |row| row.get(0))?,
            ))
        })
        .unwrap()
    }

    #[test]
    fn incoming_commit_atomically_materializes_replay_event_and_lamport_state() {
        let db = create_test_db();
        let message = incoming_message("msg-incoming", "peer-a", "peer-b", 7, 4);

        assert_eq!(
            commit_incoming(&db, &message, b"signed-payload", b"signature").unwrap(),
            IncomingMessageCommitOutcome::Inserted
        );
        assert_eq!(ingress_row_counts(&db), (1, 1, 1));
        assert_eq!(db.get_lamport_clock("peer-a").unwrap(), 14);

        let stored = MessagesRepository::get_by_message_id(&db, "msg-incoming")
            .unwrap()
            .unwrap();
        assert_eq!(stored.nonce_id, vec![7; 16]);
        assert_eq!(stored.nonce_counter, 4);
    }

    #[test]
    fn exact_incoming_duplicate_survives_restart_and_does_not_mutate_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("incoming.sqlite");
        let message = incoming_message("msg-restart", "peer-a", "peer-b", 8, 5);
        {
            let db = Database::new(path.clone()).unwrap();
            assert_eq!(
                commit_incoming(&db, &message, b"payload", b"signature").unwrap(),
                IncomingMessageCommitOutcome::Inserted
            );
            MessagesRepository::mark_read(&db, "msg-restart", 1_700_000_999).unwrap();
        }

        let reopened = Database::new(path).unwrap();
        let before = ingress_row_counts(&reopened);
        let lamport_before = reopened.get_lamport_clock("peer-a").unwrap();
        assert_eq!(
            commit_incoming(&reopened, &message, b"payload", b"signature").unwrap(),
            IncomingMessageCommitOutcome::Duplicate
        );
        assert_eq!(ingress_row_counts(&reopened), before);
        assert_eq!(
            reopened.get_lamport_clock("peer-a").unwrap(),
            lamport_before
        );
        assert_eq!(
            MessagesRepository::get_by_message_id(&reopened, "msg-restart")
                .unwrap()
                .unwrap()
                .status,
            "read"
        );
    }

    #[test]
    fn incoming_collisions_and_directional_replays_leave_state_unchanged() {
        let db = create_test_db();
        let original = incoming_message("msg-original", "peer-a", "peer-b", 9, 6);
        commit_incoming(&db, &original, b"payload", b"signature").unwrap();

        let mut collision = original.clone();
        collision.content_encrypted.push(1);
        assert!(matches!(
            commit_incoming(&db, &collision, b"payload", b"signature"),
            Err(IncomingMessagePersistenceError::IdentityCollision(_))
        ));
        assert!(matches!(
            commit_incoming(&db, &original, b"altered-payload", b"signature"),
            Err(IncomingMessagePersistenceError::IdentityCollision(_))
        ));
        assert!(matches!(
            commit_incoming(&db, &original, b"payload", b"altered-signature"),
            Err(IncomingMessagePersistenceError::IdentityCollision(_))
        ));

        let nonce_replay = incoming_message("msg-nonce", "peer-a", "peer-b", 9, 7);
        assert!(matches!(
            commit_incoming(&db, &nonce_replay, b"payload-2", b"signature-2"),
            Err(IncomingMessagePersistenceError::NonceReplay)
        ));

        let counter_replay = incoming_message("msg-counter", "peer-a", "peer-b", 10, 6);
        assert!(matches!(
            commit_incoming(&db, &counter_replay, b"payload-3", b"signature-3"),
            Err(IncomingMessagePersistenceError::CounterReplay)
        ));
        assert_eq!(ingress_row_counts(&db), (1, 1, 1));

        // Replay domains are directional: the reverse sender/recipient flow is
        // allowed to use the same nonce material and counter.
        let reverse = incoming_message("msg-reverse", "peer-b", "peer-a", 9, 6);
        assert_eq!(
            commit_incoming(&db, &reverse, b"payload-4", b"signature-4").unwrap(),
            IncomingMessageCommitOutcome::Inserted
        );
        assert_eq!(ingress_row_counts(&db), (2, 2, 2));
    }

    #[test]
    fn incoming_transaction_failure_rolls_back_every_ingress_table() {
        let db = create_test_db();
        db.with_connection(|conn| {
            conn.execute_batch(
                "CREATE TRIGGER fail_incoming_event
                 BEFORE INSERT ON message_events
                 BEGIN SELECT RAISE(ABORT, 'injected ingress failure'); END;",
            )
        })
        .unwrap();
        let message = incoming_message("msg-failure", "peer-a", "peer-b", 11, 8);

        assert!(matches!(
            commit_incoming(&db, &message, b"payload", b"signature"),
            Err(IncomingMessagePersistenceError::Database(_))
        ));
        assert_eq!(ingress_row_counts(&db), (0, 0, 0));
        assert_eq!(db.get_lamport_clock("peer-a").unwrap(), 0);

        db.with_connection(|conn| conn.execute_batch("DROP TRIGGER fail_incoming_event;"))
            .unwrap();
        assert_eq!(
            commit_incoming(&db, &message, b"payload", b"signature").unwrap(),
            IncomingMessageCommitOutcome::Inserted
        );
    }

    #[test]
    fn test_insert_and_get_message() {
        let db = create_test_db();

        let msg = MessageData {
            protocol_version: 2,
            event_id: "create:msg-123".to_string(),
            message_id: "msg-123".to_string(),
            conversation_id: "conv-456".to_string(),
            sender_peer_id: "peer-a".to_string(),
            recipient_peer_id: "peer-b".to_string(),
            nonce_id: vec![1; 16],
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
        assert_eq!(stored.protocol_version, 2);
        assert_eq!(stored.event_id, "create:msg-123");
        assert_eq!(stored.nonce_id, vec![1; 16]);
        assert_eq!(stored.content_encrypted, vec![1, 2, 3, 4]);
        assert_eq!(stored.status, "pending");
    }

    #[test]
    fn test_mark_delivered_and_read() {
        let db = create_test_db();

        let msg = MessageData {
            protocol_version: 2,
            event_id: "create:msg-456".to_string(),
            message_id: "msg-456".to_string(),
            conversation_id: "conv-789".to_string(),
            sender_peer_id: "peer-a".to_string(),
            recipient_peer_id: "peer-b".to_string(),
            nonce_id: vec![2; 16],
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
            protocol_version: 2,
            event_id: "create:msg-1".to_string(),
            message_id: "msg-1".to_string(),
            conversation_id: "conv-1".to_string(),
            sender_peer_id: "peer-a".to_string(),
            recipient_peer_id: "peer-b".to_string(),
            nonce_id: vec![3; 16],
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
            protocol_version: 2,
            event_id: "create:msg-2".to_string(),
            message_id: "msg-2".to_string(),
            conversation_id: "conv-2".to_string(),
            sender_peer_id: "peer-c".to_string(),
            recipient_peer_id: "peer-a".to_string(),
            nonce_id: vec![4; 16],
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

    fn insert_editable_message(db: &Database, message_id: &str) {
        MessagesRepository::insert_message(
            db,
            &MessageData {
                protocol_version: 2,
                event_id: format!("create:{message_id}"),
                message_id: message_id.to_string(),
                conversation_id: "conv-edit".to_string(),
                sender_peer_id: "peer-author".to_string(),
                recipient_peer_id: "peer-recipient".to_string(),
                nonce_id: vec![9; 16],
                content_encrypted: vec![1, 2, 3],
                content_type: "text".to_string(),
                reply_to_message_id: None,
                nonce_counter: 1,
                lamport_clock: 1,
                sent_at: 100,
                received_at: None,
                status: MessageStatus::Sent,
            },
        )
        .unwrap();
    }

    fn edit_event(event_id: &str, revision: u64, nonce_byte: u8) -> MessageEditEventData {
        MessageEditEventData {
            event_id: event_id.to_string(),
            protocol_version: 2,
            message_id: "msg-edit".to_string(),
            conversation_id: "conv-edit".to_string(),
            author_peer_id: "peer-author".to_string(),
            recipient_peer_id: "peer-recipient".to_string(),
            revision,
            nonce_id: vec![nonce_byte; 16],
            nonce_counter: revision + 10,
            lamport_clock: revision + 20,
            encrypted_content: vec![nonce_byte, 42],
            signature: vec![nonce_byte, 99],
            timestamp: 1_000 + revision as i64,
        }
    }

    #[test]
    fn edit_events_are_immutable_monotonic_and_exactly_idempotent() {
        let db = create_test_db();
        insert_editable_message(&db, "msg-edit");

        let revision = MessagesRepository::reserve_edit_revision(
            &db,
            "msg-edit",
            "peer-author",
            "peer-recipient",
        )
        .unwrap();
        let event = edit_event("edit:1", revision, 1);
        assert_eq!(
            MessagesRepository::record_verified_edit_event(&db, &event).unwrap(),
            EditEventApplyOutcome::Inserted
        );
        assert_eq!(
            MessagesRepository::record_verified_edit_event(&db, &event).unwrap(),
            EditEventApplyOutcome::Duplicate
        );

        let mut collision = event.clone();
        collision.encrypted_content.push(7);
        assert!(matches!(
            MessagesRepository::record_verified_edit_event(&db, &collision),
            Err(MessageEditPersistenceError::EventIdCollision(_))
        ));

        let newer = edit_event("edit:3", 3, 3);
        MessagesRepository::record_verified_edit_event(&db, &newer).unwrap();
        let stale = edit_event("edit:2", 2, 2);
        assert!(matches!(
            MessagesRepository::record_verified_edit_event(&db, &stale),
            Err(MessageEditPersistenceError::StaleRevision {
                attempted: 2,
                current: 3
            })
        ));

        let head = MessagesRepository::get_current_edit_event(&db, "msg-edit")
            .unwrap()
            .unwrap();
        assert_eq!(head.event_id, "edit:3");
        assert_eq!(head.revision, 3);
        assert_eq!(
            MessagesRepository::list_edit_events(&db, "msg-edit")
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn edit_event_must_match_original_author_recipient_and_conversation() {
        let db = create_test_db();
        insert_editable_message(&db, "msg-edit");

        for mutation in 0..3 {
            let mut event = edit_event(&format!("bad:{mutation}"), 1, mutation + 1);
            match mutation {
                0 => event.author_peer_id = "peer-attacker".to_string(),
                1 => event.recipient_peer_id = "peer-other".to_string(),
                _ => event.conversation_id = "conv-other".to_string(),
            }
            assert!(matches!(
                MessagesRepository::record_verified_edit_event(&db, &event),
                Err(MessageEditPersistenceError::MessageBindingMismatch)
            ));
        }
        assert!(MessagesRepository::list_edit_events(&db, "msg-edit")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn failed_event_transaction_never_reuses_reserved_revision() {
        let db = create_test_db();
        insert_editable_message(&db, "msg-edit");
        assert_eq!(
            MessagesRepository::reserve_edit_revision(
                &db,
                "msg-edit",
                "peer-author",
                "peer-recipient"
            )
            .unwrap(),
            1
        );
        MessagesRepository::record_verified_edit_event(&db, &edit_event("edit:1", 1, 1)).unwrap();

        assert_eq!(
            MessagesRepository::reserve_edit_revision(
                &db,
                "msg-edit",
                "peer-author",
                "peer-recipient"
            )
            .unwrap(),
            2
        );
        let duplicate_nonce = edit_event("edit:2", 2, 1);
        assert!(matches!(
            MessagesRepository::record_verified_edit_event(&db, &duplicate_nonce),
            Err(MessageEditPersistenceError::Database(_))
        ));
        assert_eq!(
            MessagesRepository::get_current_edit_event(&db, "msg-edit")
                .unwrap()
                .unwrap()
                .revision,
            1
        );
        assert_eq!(
            MessagesRepository::list_edit_events(&db, "msg-edit")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            MessagesRepository::reserve_edit_revision(
                &db,
                "msg-edit",
                "peer-author",
                "peer-recipient"
            )
            .unwrap(),
            3
        );
    }

    #[test]
    fn edit_revision_reservation_survives_database_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("messages.sqlite");
        {
            let db = Database::new(path.clone()).unwrap();
            insert_editable_message(&db, "msg-edit");
            assert_eq!(
                MessagesRepository::reserve_edit_revision(
                    &db,
                    "msg-edit",
                    "peer-author",
                    "peer-recipient"
                )
                .unwrap(),
                1
            );
        }

        let reopened = Database::new(path).unwrap();
        assert_eq!(
            MessagesRepository::reserve_edit_revision(
                &reopened,
                "msg-edit",
                "peer-author",
                "peer-recipient"
            )
            .unwrap(),
            2
        );
    }
}
