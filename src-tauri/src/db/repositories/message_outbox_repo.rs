//! Durable, bounded delivery queue for encrypted direct-message wire events.

use crate::db::repositories::messages_repo::{
    MessageData, MessageEditEventData, MessagesRepository, RecordMessageEventParams,
};
use crate::db::Database;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use thiserror::Error;

const DEFAULT_MAX_ROWS: u32 = 10_000;
const DEFAULT_MAX_ATTEMPTS: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxState {
    Queued,
    InFlight,
    Sent,
    Delivered,
    Read,
    Failed,
    Canceled,
}

impl OutboxState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::InFlight => "in_flight",
            Self::Sent => "sent",
            Self::Delivered => "delivered",
            Self::Read => "read",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }

    fn from_str(value: &str) -> Result<Self, OutboxError> {
        match value {
            "queued" => Ok(Self::Queued),
            "in_flight" => Ok(Self::InFlight),
            "sent" => Ok(Self::Sent),
            "delivered" => Ok(Self::Delivered),
            "read" => Ok(Self::Read),
            "failed" => Ok(Self::Failed),
            "canceled" => Ok(Self::Canceled),
            other => Err(OutboxError::InvalidStoredState(other.to_owned())),
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Delivered | Self::Read | Self::Failed | Self::Canceled
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutboxLimits {
    pub max_rows: u32,
    pub default_max_attempts: u32,
}

impl Default for OutboxLimits {
    fn default() -> Self {
        Self {
            max_rows: DEFAULT_MAX_ROWS,
            default_max_attempts: DEFAULT_MAX_ATTEMPTS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxEntry {
    pub event_id: String,
    pub message_id: String,
    pub peer_id: String,
    pub payload: Vec<u8>,
    pub state: OutboxState,
    pub attempt_count: u32,
    pub max_attempts: u32,
    pub next_attempt_at: i64,
    pub attempt_deadline_at: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub terminal_at: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
pub struct EnqueueOutboxMessage<'a> {
    pub event_id: &'a str,
    pub message_id: &'a str,
    pub peer_id: &'a str,
    pub payload: &'a [u8],
    pub max_attempts: Option<u32>,
    pub next_attempt_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnqueueOutcome {
    Inserted,
    Existing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionOutcome {
    Applied,
    AlreadyInState,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecoveryOutcome {
    pub requeued: usize,
    pub failed: usize,
}

#[derive(Debug, Error)]
pub enum OutboxError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("invalid outbox input: {0}")]
    InvalidInput(&'static str),
    #[error("outbox event not found: {0}")]
    NotFound(String),
    #[error("outbox event id already identifies different delivery bytes: {0}")]
    EventCollision(String),
    #[error("outbox is at its durable row limit ({0})")]
    CapacityExceeded(u32),
    #[error("invalid outbox transition for {event_id}: {from:?} -> {to:?}")]
    InvalidTransition {
        event_id: String,
        from: OutboxState,
        to: OutboxState,
    },
    #[error("invalid outbox state stored in database: {0}")]
    InvalidStoredState(String),
    #[error("integer value is too large for durable storage: {0}")]
    IntegerOverflow(&'static str),
    #[error("outgoing message, immutable event, and outbox identities do not agree: {0}")]
    BindingMismatch(&'static str),
    #[error("message edit persistence failed: {0}")]
    EditPersistence(String),
}

pub struct MessageOutboxRepository<'a> {
    db: &'a Database,
    limits: OutboxLimits,
}

impl<'a> MessageOutboxRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self::with_limits(db, OutboxLimits::default())
    }

    pub fn with_limits(db: &'a Database, limits: OutboxLimits) -> Self {
        Self { db, limits }
    }

    /// Insert a wire event once. An exact repeated enqueue is idempotent; an
    /// event-id reuse with different bytes or routing metadata is rejected.
    pub fn enqueue(&self, input: &EnqueueOutboxMessage<'_>) -> Result<EnqueueOutcome, OutboxError> {
        self.validate_limits()?;
        let max_attempts = self.validate_enqueue(input)?;
        self.db.with_connection_mut_result(|connection| {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let outcome = self.enqueue_tx(&tx, input, max_attempts)?;
            tx.commit()?;
            Ok(outcome)
        })
    }

    /// Atomically claim due events and increment their durable attempt count.
    /// Expired in-flight or sent claims are recovered in the same transaction.
    pub fn claim_due(
        &self,
        now: i64,
        lease_seconds: u32,
        limit: u32,
    ) -> Result<Vec<OutboxEntry>, OutboxError> {
        if lease_seconds == 0 {
            return Err(OutboxError::InvalidInput("lease_seconds must be positive"));
        }
        if limit == 0 {
            return Ok(Vec::new());
        }
        let deadline = now
            .checked_add(i64::from(lease_seconds))
            .ok_or(OutboxError::IntegerOverflow("attempt_deadline_at"))?;
        self.db.with_connection_mut_result(|connection| {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            Self::recover_expired_tx(&tx, now)?;
            let event_ids = {
                let mut statement = tx.prepare(
                    "SELECT event_id FROM direct_message_outbox
                     WHERE state='queued' AND next_attempt_at<=? AND attempt_count<max_attempts
                     ORDER BY next_attempt_at, created_at, event_id LIMIT ?",
                )?;
                let rows = statement
                    .query_map(params![now, i64::from(limit)], |row| {
                        row.get::<_, String>(0)
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows
            };
            let mut claimed = Vec::with_capacity(event_ids.len());
            for event_id in event_ids {
                let changed = tx.execute(
                    "UPDATE direct_message_outbox
                     SET state='in_flight', attempt_count=attempt_count+1,
                         attempt_deadline_at=?, updated_at=?, last_error=NULL
                     WHERE event_id=? AND state='queued' AND next_attempt_at<=?
                       AND attempt_count<max_attempts",
                    params![deadline, now, event_id, now],
                )?;
                if changed == 1 {
                    claimed.push(
                        Self::get_tx(&tx, &event_id)?
                            .ok_or_else(|| OutboxError::NotFound(event_id.clone()))?,
                    );
                }
            }
            tx.commit()?;
            Ok(claimed)
        })
    }

    /// Mark a transport attempt successful while retaining an acknowledgement
    /// deadline. If delivery is not acknowledged, recovery makes it retryable.
    pub fn mark_sent(
        &self,
        event_id: &str,
        acknowledgement_deadline_at: i64,
        now: i64,
    ) -> Result<TransitionOutcome, OutboxError> {
        if acknowledgement_deadline_at <= now {
            return Err(OutboxError::InvalidInput(
                "acknowledgement deadline must be in the future",
            ));
        }
        self.transition(event_id, OutboxState::Sent, now, |tx| {
            let changed = tx.execute(
                "UPDATE direct_message_outbox
                 SET state='sent', next_attempt_at=?, attempt_deadline_at=?,
                     updated_at=?, last_error=NULL
                 WHERE event_id=? AND state='in_flight'",
                params![
                    acknowledgement_deadline_at,
                    acknowledgement_deadline_at,
                    now,
                    event_id
                ],
            )?;
            if changed == 1 {
                Self::mirror_message_state_tx(tx, event_id, OutboxState::Sent, now)?;
            }
            Ok(changed)
        })
    }

    pub fn mark_delivered(
        &self,
        event_id: &str,
        now: i64,
    ) -> Result<TransitionOutcome, OutboxError> {
        self.transition(event_id, OutboxState::Delivered, now, |tx| {
            let changed = tx.execute(
                "UPDATE direct_message_outbox
                 SET state='delivered', attempt_deadline_at=NULL, updated_at=?, terminal_at=?
                 WHERE event_id=? AND state='sent'",
                params![now, now, event_id],
            )?;
            if changed == 1 {
                Self::mirror_message_state_tx(tx, event_id, OutboxState::Delivered, now)?;
            }
            Ok(changed)
        })
    }

    pub fn mark_read(&self, event_id: &str, now: i64) -> Result<TransitionOutcome, OutboxError> {
        self.transition(event_id, OutboxState::Read, now, |tx| {
            let changed = tx.execute(
                "UPDATE direct_message_outbox SET state='read', updated_at=?
                 WHERE event_id=? AND state='delivered'",
                params![now, event_id],
            )?;
            if changed == 1 {
                Self::mirror_message_state_tx(tx, event_id, OutboxState::Read, now)?;
            }
            Ok(changed)
        })
    }

    pub fn cancel(&self, event_id: &str, now: i64) -> Result<TransitionOutcome, OutboxError> {
        self.transition(event_id, OutboxState::Canceled, now, |tx| {
            let changed = tx.execute(
                "UPDATE direct_message_outbox
                 SET state='canceled', attempt_deadline_at=NULL, updated_at=?, terminal_at=?
                 WHERE event_id=? AND state IN ('queued','in_flight','sent')",
                params![now, now, event_id],
            )?;
            if changed == 1 {
                Self::mirror_message_state_tx(tx, event_id, OutboxState::Failed, now)?;
            }
            Ok(changed)
        })
    }

    /// Permanently fail an active event without scheduling another attempt.
    /// Use this for protocol rejection or an invalid response, where retrying
    /// the same immutable bytes cannot succeed.
    pub fn fail_terminal(
        &self,
        event_id: &str,
        error: &str,
        now: i64,
    ) -> Result<TransitionOutcome, OutboxError> {
        if error.trim().is_empty() {
            return Err(OutboxError::InvalidInput("error must not be empty"));
        }
        self.transition(event_id, OutboxState::Failed, now, |tx| {
            let changed = tx.execute(
                "UPDATE direct_message_outbox
                 SET state='failed',attempt_deadline_at=NULL,last_error=?,updated_at=?,terminal_at=?
                 WHERE event_id=? AND state IN ('queued','in_flight','sent')",
                params![error, now, now, event_id],
            )?;
            if changed == 1 {
                Self::mirror_message_state_tx(tx, event_id, OutboxState::Failed, now)?;
            }
            Ok(changed)
        })
    }

    /// Record a failed claimed attempt. The event is requeued unless this was
    /// its final permitted attempt, in which case failure becomes terminal.
    pub fn record_attempt_failure(
        &self,
        event_id: &str,
        error: &str,
        next_attempt_at: i64,
        now: i64,
    ) -> Result<OutboxState, OutboxError> {
        if error.trim().is_empty() {
            return Err(OutboxError::InvalidInput("error must not be empty"));
        }
        self.db.with_connection_mut_result(|connection| {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let entry = Self::get_tx(&tx, event_id)?
                .ok_or_else(|| OutboxError::NotFound(event_id.to_owned()))?;
            if !matches!(entry.state, OutboxState::InFlight | OutboxState::Sent) {
                return Err(OutboxError::InvalidTransition {
                    event_id: event_id.to_owned(),
                    from: entry.state,
                    to: OutboxState::Queued,
                });
            }
            let state = if entry.attempt_count >= entry.max_attempts {
                tx.execute(
                    "UPDATE direct_message_outbox
                     SET state='failed', attempt_deadline_at=NULL, last_error=?,
                         updated_at=?, terminal_at=?
                     WHERE event_id=? AND state IN ('in_flight','sent')",
                    params![error, now, now, event_id],
                )?;
                Self::mirror_message_state_tx(&tx, event_id, OutboxState::Failed, now)?;
                OutboxState::Failed
            } else {
                tx.execute(
                    "UPDATE direct_message_outbox
                     SET state='queued', next_attempt_at=?, attempt_deadline_at=NULL,
                         last_error=?, updated_at=?
                     WHERE event_id=? AND state IN ('in_flight','sent')",
                    params![next_attempt_at, error, now, event_id],
                )?;
                Self::mirror_message_state_tx(&tx, event_id, OutboxState::Queued, now)?;
                OutboxState::Queued
            };
            tx.commit()?;
            Ok(state)
        })
    }

    pub fn recover_expired(&self, now: i64) -> Result<RecoveryOutcome, OutboxError> {
        self.db.with_connection_mut_result(|connection| {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let outcome = Self::recover_expired_tx(&tx, now)?;
            tx.commit()?;
            Ok(outcome)
        })
    }

    /// Remove retained terminal rows only. Active and acknowledgement-waiting
    /// events are never deleted by cleanup.
    pub fn cleanup_terminal(&self, older_than: i64, limit: u32) -> Result<usize, OutboxError> {
        if limit == 0 {
            return Ok(0);
        }
        self.db.with_connection_mut_result(|connection| {
            connection
                .execute(
                    "DELETE FROM direct_message_outbox WHERE event_id IN (
                       SELECT event_id FROM direct_message_outbox
                       WHERE state IN ('delivered','read','failed','canceled')
                         AND terminal_at<=?
                       ORDER BY terminal_at, created_at LIMIT ?
                     )",
                    params![older_than, i64::from(limit)],
                )
                .map_err(OutboxError::from)
        })
    }

    pub fn get(&self, event_id: &str) -> Result<Option<OutboxEntry>, OutboxError> {
        self.db
            .with_connection_mut_result(|connection| Self::get_connection(connection, event_id))
    }

    /// A message may have a create event and later edit events. ACK callers
    /// that only have a message id must inspect these rows and transition the
    /// specific wire event; this method never guesses which edit an ACK means.
    pub fn get_by_message_id(&self, message_id: &str) -> Result<Vec<OutboxEntry>, OutboxError> {
        self.db.with_connection_mut_result(|connection| {
            let mut statement = connection.prepare(
                "SELECT event_id,message_id,peer_id,payload,state,attempt_count,max_attempts,
                        next_attempt_at,attempt_deadline_at,last_error,created_at,updated_at,terminal_at
                 FROM direct_message_outbox WHERE message_id=? ORDER BY created_at,event_id",
            )?;
            let rows = statement
                .query_map([message_id], Self::map_entry)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    pub fn count(&self) -> Result<usize, OutboxError> {
        self.db.with_connection_mut_result(|connection| {
            let count: i64 =
                connection.query_row("SELECT COUNT(*) FROM direct_message_outbox", [], |row| {
                    row.get(0)
                })?;
            usize::try_from(count).map_err(|_| OutboxError::IntegerOverflow("row count"))
        })
    }

    /// Commit all durable state for a new outgoing create event. The nonce
    /// claim, queued message row, immutable event row, and exact outbox bytes
    /// either all commit or all roll back. `event.payload_cbor` is contractually
    /// the complete encoded wire envelope, not merely its inner message, and
    /// therefore must exactly equal `outbox.payload`.
    pub fn commit_outgoing_create(
        &self,
        message: &MessageData,
        event: &RecordMessageEventParams<'_>,
        outbox: &EnqueueOutboxMessage<'_>,
    ) -> Result<EnqueueOutcome, OutboxError> {
        self.validate_limits()?;
        let max_attempts = self.validate_enqueue(outbox)?;
        Self::validate_outgoing_bindings(message, event, outbox)?;
        let nonce_counter = i64::try_from(message.nonce_counter)
            .map_err(|_| OutboxError::IntegerOverflow("nonce_counter"))?;
        self.db.with_connection_mut_result(|connection| {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if Self::get_tx(&tx, outbox.event_id)?.is_some()
                || Self::identity_exists_tx(&tx, &message.message_id, event.event_id)?
            {
                if Self::same_committed_create(&tx, message, event, outbox)? {
                    tx.commit()?;
                    return Ok(EnqueueOutcome::Existing);
                }
                return Err(OutboxError::EventCollision(outbox.event_id.to_owned()));
            }
            Self::ensure_capacity_tx(&tx, self.limits.max_rows)?;
            tx.execute(
                "INSERT INTO message_crypto_nonces(
                   author_peer_id,recipient_peer_id,nonce_id,nonce_counter,event_id,event_kind,recorded_at
                 ) VALUES(?,?,?,?,?,'create',?)",
                params![
                    message.sender_peer_id,
                    message.recipient_peer_id,
                    message.nonce_id,
                    nonce_counter,
                    message.event_id,
                    outbox.created_at
                ],
            )?;
            tx.execute(
                "INSERT INTO messages(
                   protocol_version,event_id,message_id,conversation_id,sender_peer_id,
                   recipient_peer_id,nonce_id,content_encrypted,content_type,reply_to_message_id,
                   nonce_counter,lamport_clock,sent_at,received_at,status
                 ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                params![
                    message.protocol_version,
                    message.event_id,
                    message.message_id,
                    message.conversation_id,
                    message.sender_peer_id,
                    message.recipient_peer_id,
                    message.nonce_id,
                    message.content_encrypted,
                    message.content_type,
                    message.reply_to_message_id,
                    nonce_counter,
                    message.lamport_clock,
                    message.sent_at,
                    message.received_at,
                    message.status.as_str()
                ],
            )?;
            tx.execute(
                "INSERT INTO message_events(
                   event_id,event_type,message_id,conversation_id,sender_peer_id,recipient_peer_id,
                   lamport_clock,timestamp,payload_cbor,signature,received_at
                 ) VALUES(?,?,?,?,?,?,?,?,?,?,?)",
                params![
                    event.event_id,
                    event.event_type,
                    event.message_id,
                    event.conversation_id,
                    event.sender_peer_id,
                    event.recipient_peer_id,
                    event.lamport_clock,
                    event.timestamp,
                    event.payload_cbor,
                    event.signature,
                    outbox.created_at
                ],
            )?;
            Self::insert_outbox_tx(&tx, outbox, max_attempts)?;
            tx.commit()?;
            Ok(EnqueueOutcome::Inserted)
        })
    }

    /// Atomically append a verified outgoing edit and queue its exact encoded
    /// wire envelope. A queue failure rolls back the nonce and edit ledger too.
    pub fn commit_outgoing_edit(
        &self,
        event: &MessageEditEventData,
        outbox: &EnqueueOutboxMessage<'_>,
    ) -> Result<EnqueueOutcome, OutboxError> {
        self.validate_limits()?;
        let max_attempts = self.validate_enqueue(outbox)?;
        if event.event_id != outbox.event_id {
            return Err(OutboxError::BindingMismatch("edit event_id"));
        }
        if event.message_id != outbox.message_id {
            return Err(OutboxError::BindingMismatch("edit message_id"));
        }
        if event.recipient_peer_id != outbox.peer_id {
            return Err(OutboxError::BindingMismatch("edit recipient_peer_id"));
        }

        self.db.with_connection_mut_result(|connection| {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let existing = Self::get_tx(&tx, outbox.event_id)?;
            if let Some(existing) = &existing {
                if existing.message_id != outbox.message_id
                    || existing.peer_id != outbox.peer_id
                    || existing.payload != outbox.payload
                    || existing.max_attempts != max_attempts
                {
                    return Err(OutboxError::EventCollision(outbox.event_id.to_owned()));
                }
            } else {
                Self::ensure_capacity_tx(&tx, self.limits.max_rows)?;
            }

            MessagesRepository::record_verified_edit_event_tx(&tx, event)
                .map_err(|error| OutboxError::EditPersistence(error.to_string()))?;
            if existing.is_some() {
                tx.commit()?;
                return Ok(EnqueueOutcome::Existing);
            }
            Self::insert_outbox_tx(&tx, outbox, max_attempts)?;
            tx.commit()?;
            Ok(EnqueueOutcome::Inserted)
        })
    }

    fn validate_limits(&self) -> Result<(), OutboxError> {
        if self.limits.max_rows == 0 {
            return Err(OutboxError::InvalidInput("max_rows must be positive"));
        }
        if self.limits.default_max_attempts == 0 {
            return Err(OutboxError::InvalidInput(
                "default_max_attempts must be positive",
            ));
        }
        Ok(())
    }

    fn validate_enqueue(&self, input: &EnqueueOutboxMessage<'_>) -> Result<u32, OutboxError> {
        for (value, name) in [
            (input.event_id, "event_id"),
            (input.message_id, "message_id"),
            (input.peer_id, "peer_id"),
        ] {
            if value.trim().is_empty() {
                return Err(OutboxError::InvalidInput(name));
            }
        }
        if input.payload.is_empty() {
            return Err(OutboxError::InvalidInput("payload must not be empty"));
        }
        let max_attempts = input
            .max_attempts
            .unwrap_or(self.limits.default_max_attempts);
        if max_attempts == 0 {
            return Err(OutboxError::InvalidInput("max_attempts must be positive"));
        }
        Ok(max_attempts)
    }

    fn validate_outgoing_bindings(
        message: &MessageData,
        event: &RecordMessageEventParams<'_>,
        outbox: &EnqueueOutboxMessage<'_>,
    ) -> Result<(), OutboxError> {
        if message.event_id != event.event_id || event.event_id != outbox.event_id {
            return Err(OutboxError::BindingMismatch("event_id"));
        }
        if message.event_id != message.message_id {
            return Err(OutboxError::BindingMismatch(
                "create event_id must equal message_id",
            ));
        }
        if message.message_id != event.message_id || event.message_id != outbox.message_id {
            return Err(OutboxError::BindingMismatch("message_id"));
        }
        if message.conversation_id != event.conversation_id {
            return Err(OutboxError::BindingMismatch("conversation_id"));
        }
        if message.sender_peer_id != event.sender_peer_id {
            return Err(OutboxError::BindingMismatch("sender_peer_id"));
        }
        if message.recipient_peer_id != event.recipient_peer_id
            || event.recipient_peer_id != outbox.peer_id
        {
            return Err(OutboxError::BindingMismatch("recipient_peer_id"));
        }
        if event.payload_cbor != outbox.payload {
            return Err(OutboxError::BindingMismatch("encoded payload"));
        }
        if event.event_type != "sent" {
            return Err(OutboxError::BindingMismatch("event_type must be sent"));
        }
        if message.status.as_str() != "queued" {
            return Err(OutboxError::BindingMismatch(
                "message status must be queued",
            ));
        }
        Ok(())
    }

    fn enqueue_tx(
        &self,
        tx: &Transaction<'_>,
        input: &EnqueueOutboxMessage<'_>,
        max_attempts: u32,
    ) -> Result<EnqueueOutcome, OutboxError> {
        if let Some(existing) = Self::get_tx(tx, input.event_id)? {
            if existing.message_id == input.message_id
                && existing.peer_id == input.peer_id
                && existing.payload == input.payload
                && existing.max_attempts == max_attempts
            {
                return Ok(EnqueueOutcome::Existing);
            }
            return Err(OutboxError::EventCollision(input.event_id.to_owned()));
        }
        Self::ensure_capacity_tx(tx, self.limits.max_rows)?;
        Self::insert_outbox_tx(tx, input, max_attempts)?;
        Self::mirror_message_state_tx(tx, input.event_id, OutboxState::Queued, input.created_at)?;
        Ok(EnqueueOutcome::Inserted)
    }

    fn insert_outbox_tx(
        tx: &Transaction<'_>,
        input: &EnqueueOutboxMessage<'_>,
        max_attempts: u32,
    ) -> Result<(), OutboxError> {
        tx.execute(
            "INSERT INTO direct_message_outbox(
               event_id,message_id,peer_id,payload,state,attempt_count,max_attempts,
               next_attempt_at,attempt_deadline_at,last_error,created_at,updated_at,terminal_at
             ) VALUES(?,?,?,?,'queued',0,?,?,NULL,NULL,?,?,NULL)",
            params![
                input.event_id,
                input.message_id,
                input.peer_id,
                input.payload,
                i64::from(max_attempts),
                input.next_attempt_at,
                input.created_at,
                input.created_at
            ],
        )?;
        Ok(())
    }

    fn ensure_capacity_tx(tx: &Transaction<'_>, max_rows: u32) -> Result<(), OutboxError> {
        let count: i64 = tx.query_row("SELECT COUNT(*) FROM direct_message_outbox", [], |row| {
            row.get(0)
        })?;
        if count < i64::from(max_rows) {
            return Ok(());
        }
        let remove = count - i64::from(max_rows) + 1;
        tx.execute(
            "DELETE FROM direct_message_outbox WHERE event_id IN (
               SELECT event_id FROM direct_message_outbox
               WHERE state IN ('delivered','read','failed','canceled')
               ORDER BY terminal_at,created_at LIMIT ?
             )",
            [remove],
        )?;
        let remaining: i64 =
            tx.query_row("SELECT COUNT(*) FROM direct_message_outbox", [], |row| {
                row.get(0)
            })?;
        if remaining >= i64::from(max_rows) {
            return Err(OutboxError::CapacityExceeded(max_rows));
        }
        Ok(())
    }

    fn recover_expired_tx(tx: &Transaction<'_>, now: i64) -> Result<RecoveryOutcome, OutboxError> {
        let failed = tx.execute(
            "UPDATE direct_message_outbox
             SET state='failed',attempt_deadline_at=NULL,
                 last_error=COALESCE(last_error,'delivery acknowledgement timed out'),
                 updated_at=?,terminal_at=?
             WHERE state IN ('in_flight','sent') AND attempt_deadline_at<=?
               AND attempt_count>=max_attempts",
            params![now, now, now],
        )?;
        let requeued = tx.execute(
            "UPDATE direct_message_outbox
             SET state='queued',next_attempt_at=?,attempt_deadline_at=NULL,
                 last_error=COALESCE(last_error,'delivery acknowledgement timed out'),updated_at=?
             WHERE state IN ('in_flight','sent') AND attempt_deadline_at<=?
               AND attempt_count<max_attempts",
            params![now, now, now],
        )?;
        tx.execute(
            "UPDATE messages SET status='failed'
             WHERE status NOT IN ('delivered','read') AND message_id IN (
               SELECT message_id FROM direct_message_outbox
               WHERE state='failed' AND updated_at=? AND event_id=message_id
             )",
            [now],
        )?;
        tx.execute(
            "UPDATE messages SET status='queued'
             WHERE status IN ('pending','sent','failed') AND message_id IN (
               SELECT message_id FROM direct_message_outbox
               WHERE state='queued' AND updated_at=? AND event_id=message_id
             )",
            [now],
        )?;
        Ok(RecoveryOutcome { requeued, failed })
    }

    fn mirror_message_state_tx(
        tx: &Transaction<'_>,
        event_id: &str,
        state: OutboxState,
        now: i64,
    ) -> Result<(), rusqlite::Error> {
        let message_id: Option<String> = tx
            .query_row(
                "SELECT message_id FROM direct_message_outbox
                 WHERE event_id=? AND event_id=message_id",
                [event_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(message_id) = message_id else {
            return Ok(());
        };
        match state {
            OutboxState::Queued => {
                tx.execute(
                    "UPDATE messages SET status='queued'
                     WHERE message_id=? AND status IN ('pending','failed','queued')",
                    [message_id],
                )?;
            }
            OutboxState::Sent => {
                tx.execute(
                    "UPDATE messages SET status='sent'
                     WHERE message_id=? AND status IN ('queued','pending','failed','sent')",
                    [message_id],
                )?;
            }
            OutboxState::Delivered => {
                tx.execute(
                    "UPDATE messages SET status='delivered',delivered_at=COALESCE(delivered_at,?)
                     WHERE message_id=? AND status!='read'",
                    params![now, message_id],
                )?;
            }
            OutboxState::Read => {
                tx.execute(
                    "UPDATE messages SET status='read',delivered_at=COALESCE(delivered_at,?),
                         read_at=COALESCE(read_at,?) WHERE message_id=?",
                    params![now, now, message_id],
                )?;
            }
            OutboxState::Failed | OutboxState::Canceled => {
                tx.execute(
                    "UPDATE messages SET status='failed'
                     WHERE message_id=? AND status NOT IN ('delivered','read')",
                    [message_id],
                )?;
            }
            OutboxState::InFlight => {}
        }
        Ok(())
    }

    fn transition<F>(
        &self,
        event_id: &str,
        target: OutboxState,
        _now: i64,
        apply: F,
    ) -> Result<TransitionOutcome, OutboxError>
    where
        F: FnOnce(&Transaction<'_>) -> rusqlite::Result<usize>,
    {
        self.db.with_connection_mut_result(|connection| {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let current = Self::get_tx(&tx, event_id)?
                .ok_or_else(|| OutboxError::NotFound(event_id.to_owned()))?
                .state;
            if current == target {
                tx.commit()?;
                return Ok(TransitionOutcome::AlreadyInState);
            }
            if apply(&tx)? != 1 {
                return Err(OutboxError::InvalidTransition {
                    event_id: event_id.to_owned(),
                    from: current,
                    to: target,
                });
            }
            tx.commit()?;
            Ok(TransitionOutcome::Applied)
        })
    }

    fn identity_exists_tx(
        tx: &Transaction<'_>,
        message_id: &str,
        event_id: &str,
    ) -> Result<bool, OutboxError> {
        tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM messages WHERE message_id=? OR event_id=?
               UNION ALL SELECT 1 FROM message_events WHERE event_id=?
             )",
            params![message_id, event_id, event_id],
            |row| row.get(0),
        )
        .map_err(OutboxError::from)
    }

    fn same_committed_create(
        tx: &Transaction<'_>,
        message: &MessageData,
        event: &RecordMessageEventParams<'_>,
        outbox: &EnqueueOutboxMessage<'_>,
    ) -> Result<bool, OutboxError> {
        let stored_message: Option<(
            i64,
            String,
            String,
            String,
            String,
            Vec<u8>,
            Vec<u8>,
            String,
            Option<String>,
            i64,
            i64,
            i64,
        )> = tx
            .query_row(
                "SELECT protocol_version,event_id,conversation_id,sender_peer_id,recipient_peer_id,
                        nonce_id,content_encrypted,content_type,reply_to_message_id,nonce_counter,
                        lamport_clock,sent_at FROM messages WHERE message_id=?",
                [message.message_id.as_str()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                        row.get(11)?,
                    ))
                },
            )
            .optional()?;
        let same_message = stored_message.is_some_and(|stored| {
            stored.0 == i64::from(message.protocol_version)
                && stored.1 == message.event_id
                && stored.2 == message.conversation_id
                && stored.3 == message.sender_peer_id
                && stored.4 == message.recipient_peer_id
                && stored.5 == message.nonce_id
                && stored.6 == message.content_encrypted
                && stored.7 == message.content_type
                && stored.8 == message.reply_to_message_id
                && stored.9 == i64::try_from(message.nonce_counter).unwrap_or(-1)
                && stored.10 == message.lamport_clock
                && stored.11 == message.sent_at
        });
        let same_event: bool = tx
            .query_row(
                "SELECT event_type=? AND message_id=? AND conversation_id=?
                        AND sender_peer_id=? AND recipient_peer_id=? AND lamport_clock=?
                        AND timestamp=? AND payload_cbor=? AND signature=?
                 FROM message_events WHERE event_id=?",
                params![
                    event.event_type,
                    event.message_id,
                    event.conversation_id,
                    event.sender_peer_id,
                    event.recipient_peer_id,
                    event.lamport_clock,
                    event.timestamp,
                    event.payload_cbor,
                    event.signature,
                    event.event_id
                ],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(false);
        let same_outbox = Self::get_tx(tx, outbox.event_id)?.is_some_and(|stored| {
            stored.message_id == outbox.message_id
                && stored.peer_id == outbox.peer_id
                && stored.payload == outbox.payload
        });
        Ok(same_message && same_event && same_outbox)
    }

    fn get_tx(tx: &Transaction<'_>, event_id: &str) -> Result<Option<OutboxEntry>, OutboxError> {
        Self::get_connection(tx, event_id)
    }

    fn get_connection(
        connection: &rusqlite::Connection,
        event_id: &str,
    ) -> Result<Option<OutboxEntry>, OutboxError> {
        connection
            .query_row(
                "SELECT event_id,message_id,peer_id,payload,state,attempt_count,max_attempts,
                        next_attempt_at,attempt_deadline_at,last_error,created_at,updated_at,terminal_at
                 FROM direct_message_outbox WHERE event_id=?",
                [event_id],
                Self::map_entry,
            )
            .optional()
            .map_err(OutboxError::from)
    }

    fn map_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<OutboxEntry> {
        let state: String = row.get(4)?;
        let attempt_count: i64 = row.get(5)?;
        let max_attempts: i64 = row.get(6)?;
        let parsed_state = OutboxState::from_str(&state).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
        Ok(OutboxEntry {
            event_id: row.get(0)?,
            message_id: row.get(1)?,
            peer_id: row.get(2)?,
            payload: row.get(3)?,
            state: parsed_state,
            attempt_count: u32::try_from(attempt_count).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })?,
            max_attempts: u32::try_from(max_attempts).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })?,
            next_attempt_at: row.get(7)?,
            attempt_deadline_at: row.get(8)?,
            last_error: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
            terminal_at: row.get(12)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::MessageStatus;

    fn input<'a>(
        event_id: &'a str,
        message_id: &'a str,
        payload: &'a [u8],
        max_attempts: u32,
        at: i64,
    ) -> EnqueueOutboxMessage<'a> {
        EnqueueOutboxMessage {
            event_id,
            message_id,
            peer_id: "peer-b",
            payload,
            max_attempts: Some(max_attempts),
            next_attempt_at: at,
            created_at: at,
        }
    }

    fn limited_repo(db: &Database, max_rows: u32) -> MessageOutboxRepository<'_> {
        MessageOutboxRepository::with_limits(
            db,
            OutboxLimits {
                max_rows,
                default_max_attempts: 3,
            },
        )
    }

    fn outgoing_message() -> MessageData {
        MessageData {
            protocol_version: 2,
            event_id: "message-create".into(),
            message_id: "message-create".into(),
            conversation_id: "conversation-a-b".into(),
            sender_peer_id: "peer-a".into(),
            recipient_peer_id: "peer-b".into(),
            nonce_id: vec![7; 16],
            content_encrypted: vec![8, 9, 10],
            content_type: "text".into(),
            reply_to_message_id: None,
            nonce_counter: 1,
            lamport_clock: 2,
            sent_at: 100,
            received_at: None,
            status: MessageStatus::Queued,
        }
    }

    fn assert_message_state(db: &Database, expected: &str) {
        db.with_connection(|connection| {
            let state: String = connection.query_row(
                "SELECT status FROM messages WHERE message_id='message-create'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(state, expected);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn exact_payload_and_claim_state_survive_restart() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("outbox.db");
        let bytes = [0, 255, 19, 42, 7];
        {
            let db = Database::new(path.clone()).unwrap();
            let repo = MessageOutboxRepository::new(&db);
            assert_eq!(
                repo.enqueue(&input("event-1", "message-1", &bytes, 3, 10))
                    .unwrap(),
                EnqueueOutcome::Inserted
            );
            let claimed = repo.claim_due(10, 5, 1).unwrap();
            assert_eq!(claimed[0].payload, bytes);
            assert_eq!(claimed[0].attempt_count, 1);
        }
        {
            let db = Database::new(path).unwrap();
            let repo = MessageOutboxRepository::new(&db);
            assert_eq!(repo.get("event-1").unwrap().unwrap().payload, bytes);
            assert_eq!(
                repo.recover_expired(15).unwrap(),
                RecoveryOutcome {
                    requeued: 1,
                    failed: 0
                }
            );
            let claimed = repo.claim_due(15, 5, 1).unwrap();
            assert_eq!(claimed[0].payload, bytes);
            assert_eq!(claimed[0].attempt_count, 2);
        }
    }

    #[test]
    fn retries_exhaust_exactly_at_the_configured_limit() {
        let db = Database::in_memory().unwrap();
        let repo = MessageOutboxRepository::new(&db);
        repo.enqueue(&input("event-1", "message-1", &[1], 2, 0))
            .unwrap();

        assert_eq!(repo.claim_due(0, 5, 1).unwrap()[0].attempt_count, 1);
        assert_eq!(
            repo.record_attempt_failure("event-1", "offline", 10, 1)
                .unwrap(),
            OutboxState::Queued
        );
        assert!(repo.claim_due(9, 5, 1).unwrap().is_empty());
        assert_eq!(repo.claim_due(10, 5, 1).unwrap()[0].attempt_count, 2);
        assert_eq!(
            repo.record_attempt_failure("event-1", "still offline", 20, 11)
                .unwrap(),
            OutboxState::Failed
        );
        let stored = repo.get("event-1").unwrap().unwrap();
        assert_eq!(stored.state, OutboxState::Failed);
        assert_eq!(stored.attempt_count, 2);
        assert_eq!(stored.last_error.as_deref(), Some("still offline"));
        assert!(repo.claim_due(100, 5, 1).unwrap().is_empty());
    }

    #[test]
    fn cancellation_is_durable_and_cannot_be_resurrected() {
        let db = Database::in_memory().unwrap();
        let repo = MessageOutboxRepository::new(&db);
        repo.enqueue(&input("event-1", "message-1", &[1], 2, 0))
            .unwrap();
        assert_eq!(
            repo.cancel("event-1", 1).unwrap(),
            TransitionOutcome::Applied
        );
        assert_eq!(
            repo.cancel("event-1", 2).unwrap(),
            TransitionOutcome::AlreadyInState
        );
        assert!(repo.claim_due(100, 5, 1).unwrap().is_empty());
        assert!(matches!(
            repo.mark_sent("event-1", 110, 100),
            Err(OutboxError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn non_retryable_failure_is_terminal_from_every_active_state() {
        for (suffix, prepare) in [("queued", 0_u8), ("in-flight", 1_u8), ("sent", 2_u8)] {
            let db = Database::in_memory().unwrap();
            let repo = MessageOutboxRepository::new(&db);
            let event_id = format!("event-{suffix}");
            repo.enqueue(&input(&event_id, "message-1", &[1], 3, 0))
                .unwrap();
            if prepare >= 1 {
                repo.claim_due(0, 5, 1).unwrap();
            }
            if prepare >= 2 {
                repo.mark_sent(&event_id, 10, 1).unwrap();
            }
            assert_eq!(
                repo.fail_terminal(&event_id, "remote rejected payload", 2)
                    .unwrap(),
                TransitionOutcome::Applied
            );
            let stored = repo.get(&event_id).unwrap().unwrap();
            assert_eq!(stored.state, OutboxState::Failed);
            assert_eq!(
                stored.last_error.as_deref(),
                Some("remote rejected payload")
            );
            assert!(repo.claim_due(100, 5, 1).unwrap().is_empty());
            assert_eq!(
                repo.fail_terminal(&event_id, "remote rejected payload", 3)
                    .unwrap(),
                TransitionOutcome::AlreadyInState
            );
        }
    }

    #[test]
    fn sent_ack_timeout_recovers_then_fails_on_final_attempt() {
        let db = Database::in_memory().unwrap();
        let repo = MessageOutboxRepository::new(&db);
        repo.enqueue(&input("event-1", "message-1", &[1], 2, 0))
            .unwrap();
        repo.claim_due(0, 5, 1).unwrap();
        repo.mark_sent("event-1", 10, 1).unwrap();
        assert_eq!(repo.recover_expired(10).unwrap().requeued, 1);
        repo.claim_due(10, 5, 1).unwrap();
        repo.mark_sent("event-1", 20, 11).unwrap();
        assert_eq!(repo.recover_expired(20).unwrap().failed, 1);
        assert_eq!(
            repo.get("event-1").unwrap().unwrap().state,
            OutboxState::Failed
        );
    }

    #[test]
    fn transition_order_is_strict_and_idempotent() {
        let db = Database::in_memory().unwrap();
        let repo = MessageOutboxRepository::new(&db);
        repo.enqueue(&input("event-1", "message-1", &[1], 3, 0))
            .unwrap();
        assert!(matches!(
            repo.mark_sent("event-1", 10, 0),
            Err(OutboxError::InvalidTransition { .. })
        ));
        repo.claim_due(0, 5, 1).unwrap();
        repo.mark_sent("event-1", 10, 1).unwrap();
        assert!(matches!(
            repo.mark_read("event-1", 2),
            Err(OutboxError::InvalidTransition { .. })
        ));
        repo.mark_delivered("event-1", 3).unwrap();
        assert_eq!(
            repo.mark_delivered("event-1", 4).unwrap(),
            TransitionOutcome::AlreadyInState
        );
        repo.mark_read("event-1", 5).unwrap();
        assert_eq!(
            repo.mark_read("event-1", 6).unwrap(),
            TransitionOutcome::AlreadyInState
        );
    }

    #[test]
    fn claim_failure_injection_rolls_back_attempt_and_state() {
        let db = Database::in_memory().unwrap();
        let repo = MessageOutboxRepository::new(&db);
        repo.enqueue(&input("event-1", "message-1", &[1], 3, 0))
            .unwrap();
        db.with_connection(|connection| {
            connection.execute_batch(
                "CREATE TRIGGER inject_claim_failure
                 BEFORE UPDATE OF state ON direct_message_outbox
                 WHEN NEW.state='in_flight'
                 BEGIN SELECT RAISE(ABORT, 'injected claim failure'); END;",
            )
        })
        .unwrap();

        assert!(matches!(
            repo.claim_due(0, 5, 1),
            Err(OutboxError::Database(_))
        ));
        let stored = repo.get("event-1").unwrap().unwrap();
        assert_eq!(stored.state, OutboxState::Queued);
        assert_eq!(stored.attempt_count, 0);
    }

    #[test]
    fn row_limit_never_evicts_active_work() {
        let db = Database::in_memory().unwrap();
        let repo = limited_repo(&db, 2);
        repo.enqueue(&input("event-1", "message-1", &[1], 3, 0))
            .unwrap();
        repo.enqueue(&input("event-2", "message-2", &[2], 3, 1))
            .unwrap();
        assert!(matches!(
            repo.enqueue(&input("event-3", "message-3", &[3], 3, 2)),
            Err(OutboxError::CapacityExceeded(2))
        ));
        repo.cancel("event-1", 3).unwrap();
        repo.enqueue(&input("event-3", "message-3", &[3], 3, 4))
            .unwrap();
        assert_eq!(repo.count().unwrap(), 2);
        assert!(repo.get("event-1").unwrap().is_none());
        assert!(repo.get("event-2").unwrap().is_some());
    }

    #[test]
    fn outgoing_create_commit_and_status_mirroring_are_atomic() {
        let db = Database::in_memory().unwrap();
        let repo = MessageOutboxRepository::new(&db);
        let message = outgoing_message();
        let wire = [0xa1, 0x01, 0x02];
        let event = RecordMessageEventParams {
            event_id: &message.event_id,
            event_type: "sent",
            message_id: &message.message_id,
            conversation_id: &message.conversation_id,
            sender_peer_id: &message.sender_peer_id,
            recipient_peer_id: &message.recipient_peer_id,
            lamport_clock: message.lamport_clock,
            timestamp: message.sent_at,
            payload_cbor: &wire,
            signature: &[9; 64],
        };
        let outbox = input(
            &message.event_id,
            &message.message_id,
            &wire,
            3,
            message.sent_at,
        );
        assert_eq!(
            repo.commit_outgoing_create(&message, &event, &outbox)
                .unwrap(),
            EnqueueOutcome::Inserted
        );
        assert_eq!(
            repo.commit_outgoing_create(&message, &event, &outbox)
                .unwrap(),
            EnqueueOutcome::Existing
        );
        assert_message_state(&db, "queued");
        repo.claim_due(100, 5, 1).unwrap();
        assert_message_state(&db, "queued");
        repo.mark_sent(&message.event_id, 110, 101).unwrap();
        assert_message_state(&db, "sent");
        repo.mark_delivered(&message.event_id, 102).unwrap();
        assert_message_state(&db, "delivered");
        repo.mark_read(&message.event_id, 103).unwrap();
        assert_message_state(&db, "read");
        let stored = crate::db::repositories::MessagesRepository::get_by_message_id(
            &db,
            &message.message_id,
        )
        .unwrap()
        .unwrap();
        assert_eq!(stored.delivered_at, Some(102));
        assert_eq!(stored.read_at, Some(103));
        assert_eq!(
            repo.get_by_message_id(&message.message_id).unwrap().len(),
            1
        );
    }

    #[test]
    fn edit_outbox_failure_never_regresses_base_message_status() {
        let db = Database::in_memory().unwrap();
        let repo = MessageOutboxRepository::new(&db);
        let message = outgoing_message();
        let create_wire = [1, 2, 3];
        let create_event = RecordMessageEventParams {
            event_id: &message.event_id,
            event_type: "sent",
            message_id: &message.message_id,
            conversation_id: &message.conversation_id,
            sender_peer_id: &message.sender_peer_id,
            recipient_peer_id: &message.recipient_peer_id,
            lamport_clock: message.lamport_clock,
            timestamp: message.sent_at,
            payload_cbor: &create_wire,
            signature: &[8; 64],
        };
        repo.commit_outgoing_create(
            &message,
            &create_event,
            &input(&message.event_id, &message.message_id, &create_wire, 3, 100),
        )
        .unwrap();
        repo.claim_due(100, 5, 1).unwrap();
        repo.mark_sent(&message.event_id, 110, 101).unwrap();
        repo.mark_delivered(&message.event_id, 102).unwrap();
        assert_message_state(&db, "delivered");

        let edit = MessageEditEventData {
            event_id: "edit-event-1".into(),
            protocol_version: 2,
            message_id: message.message_id.clone(),
            conversation_id: message.conversation_id.clone(),
            author_peer_id: message.sender_peer_id.clone(),
            recipient_peer_id: message.recipient_peer_id.clone(),
            revision: 1,
            nonce_id: vec![6; 16],
            nonce_counter: 2,
            lamport_clock: 3,
            encrypted_content: vec![4, 5, 6],
            signature: vec![7; 64],
            timestamp: 103,
        };
        repo.commit_outgoing_edit(
            &edit,
            &input("edit-event-1", &message.message_id, &[9, 9, 9], 3, 103),
        )
        .unwrap();
        repo.claim_due(103, 5, 1).unwrap();
        repo.fail_terminal("edit-event-1", "remote rejected edit", 104)
            .unwrap();
        assert_message_state(&db, "delivered");
    }

    #[test]
    fn outgoing_commit_failure_injection_leaves_no_partial_rows() {
        let db = Database::in_memory().unwrap();
        db.with_connection(|connection| {
            connection.execute_batch(
                "CREATE TRIGGER inject_outbox_insert_failure
                 BEFORE INSERT ON direct_message_outbox
                 BEGIN SELECT RAISE(ABORT, 'injected outbox failure'); END;",
            )
        })
        .unwrap();
        let repo = MessageOutboxRepository::new(&db);
        let message = outgoing_message();
        let wire = [1, 2, 3];
        let event = RecordMessageEventParams {
            event_id: &message.event_id,
            event_type: "sent",
            message_id: &message.message_id,
            conversation_id: &message.conversation_id,
            sender_peer_id: &message.sender_peer_id,
            recipient_peer_id: &message.recipient_peer_id,
            lamport_clock: message.lamport_clock,
            timestamp: message.sent_at,
            payload_cbor: &wire,
            signature: &[8; 64],
        };
        let outbox = input(
            &message.event_id,
            &message.message_id,
            &wire,
            3,
            message.sent_at,
        );
        assert!(matches!(
            repo.commit_outgoing_create(&message, &event, &outbox),
            Err(OutboxError::Database(_))
        ));
        db.with_connection(|connection| {
            for table in [
                "message_crypto_nonces",
                "messages",
                "message_events",
                "direct_message_outbox",
            ] {
                let count: i64 =
                    connection.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })?;
                assert_eq!(count, 0, "{table} was not rolled back");
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn outgoing_edit_and_outbox_roll_back_together() {
        let db = Database::in_memory().unwrap();
        let repo = MessageOutboxRepository::new(&db);
        let message = outgoing_message();
        let create_wire = [1, 2, 3];
        repo.commit_outgoing_create(
            &message,
            &RecordMessageEventParams {
                event_id: &message.event_id,
                event_type: "sent",
                message_id: &message.message_id,
                conversation_id: &message.conversation_id,
                sender_peer_id: &message.sender_peer_id,
                recipient_peer_id: &message.recipient_peer_id,
                lamport_clock: message.lamport_clock,
                timestamp: message.sent_at,
                payload_cbor: &create_wire,
                signature: &[8; 64],
            },
            &input(&message.event_id, &message.message_id, &create_wire, 3, 100),
        )
        .unwrap();
        db.with_connection(|connection| {
            connection.execute_batch(
                "CREATE TRIGGER inject_edit_outbox_failure
                 BEFORE INSERT ON direct_message_outbox
                 WHEN NEW.event_id='edit-fail'
                 BEGIN SELECT RAISE(ABORT, 'injected edit outbox failure'); END;",
            )
        })
        .unwrap();

        let edit = MessageEditEventData {
            event_id: "edit-fail".into(),
            protocol_version: 2,
            message_id: message.message_id.clone(),
            conversation_id: message.conversation_id.clone(),
            author_peer_id: message.sender_peer_id.clone(),
            recipient_peer_id: message.recipient_peer_id.clone(),
            revision: 1,
            nonce_id: vec![5; 16],
            nonce_counter: 2,
            lamport_clock: 3,
            encrypted_content: vec![4, 5, 6],
            signature: vec![7; 64],
            timestamp: 103,
        };
        assert!(matches!(
            repo.commit_outgoing_edit(
                &edit,
                &input("edit-fail", &message.message_id, &[9, 9], 3, 103),
            ),
            Err(OutboxError::Database(_))
        ));
        db.with_connection(|connection| {
            let edits: i64 = connection.query_row(
                "SELECT COUNT(*) FROM message_edit_events WHERE event_id='edit-fail'",
                [],
                |row| row.get(0),
            )?;
            let nonces: i64 = connection.query_row(
                "SELECT COUNT(*) FROM message_crypto_nonces WHERE event_id='edit-fail'",
                [],
                |row| row.get(0),
            )?;
            let queued: i64 = connection.query_row(
                "SELECT COUNT(*) FROM direct_message_outbox WHERE event_id='edit-fail'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!((edits, nonces, queued), (0, 0, 0));
            Ok(())
        })
        .unwrap();
    }
}
