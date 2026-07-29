//! Messaging service for sending and receiving direct messages

use ed25519_dalek::VerifyingKey;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use x25519_dalek::PublicKey as X25519Public;

use crate::db::repositories::message_outbox_repo::{
    EnqueueOutboxMessage, MessageOutboxRepository, OutboxEntry, OutboxError, OutboxState,
};
use crate::db::repositories::{EditEventApplyOutcome, MessageEditEventData};
use crate::db::{
    Capability, Conversation, Database, IncomingMessageCommit, IncomingMessagePersistenceError,
    MessageData, MessageStatus, MessagesRepository, RecordMessageEventParams,
};
use crate::error::{AppError, Result};
use crate::p2p::protocols::messaging::{
    derive_conversation_id, AckStatus, DirectMessageV2, MessageAck, MessagingCodec,
    MessagingMessage,
};
use crate::p2p::types::{MessageDeliveryFailure, MessageDeliveryReceipt};
use crate::services::{
    decrypt_message_event, derive_directional_message_key, encrypt_message_event, verify,
    ContactsService, CryptoService, EncryptedMessageEvent, IdentityService, MessageEventContext,
    MessageEventKind, MessageNonceId, PermissionsService, Signable, SignableDirectMessageV2,
    SignableMessageAck, SignableMessageEditV2, MESSAGE_CRYPTO_VERSION, MESSAGE_NONCE_ID_LEN,
};

/// Service for managing direct messages
pub struct MessagingService {
    db: Arc<Database>,
    identity_service: Arc<IdentityService>,
    contacts_service: Arc<ContactsService>,
    permissions_service: Arc<PermissionsService>,
}

const READ_RECEIPTS_POLICY_KEY: &str = "privacy.read_receipts.enabled";

/// Profile-scoped protocol policy. Presence is intentionally absent until a
/// signed presence protocol exists; the application must not infer or publish it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagingPrivacyPolicy {
    pub read_receipts_enabled: bool,
}

/// A decrypted message for the UI
#[derive(Debug, Clone)]
pub struct DecryptedMessage {
    pub message_id: String,
    pub conversation_id: String,
    pub sender_peer_id: String,
    pub recipient_peer_id: String,
    pub content_state: MessageContentState,
    pub content_type: String,
    pub reply_to_message_id: Option<String>,
    pub sent_at: i64,
    pub delivered_at: Option<i64>,
    pub read_at: Option<i64>,
    pub status: String,
    pub is_outgoing: bool,
    pub edited_at: Option<i64>,
}

/// The only message-content states that may cross the IPC boundary.
///
/// Authentication or decryption failures are data, not authored text. Keeping
/// them typed prevents a diagnostic from being searched, edited, parsed as
/// media, or rendered as though the sender wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageContentState {
    Plaintext { text: String },
    Tampered,
    WrongKey,
    UnsupportedVersion { version: u16 },
    CorruptPayload,
}

const AES_GCM_TAG_LEN: usize = 16;

fn inspect_encrypted_event_shape(
    protocol_version: u16,
    nonce_id: &[u8],
    ciphertext: &[u8],
) -> std::result::Result<MessageNonceId, MessageContentState> {
    if protocol_version != MESSAGE_CRYPTO_VERSION {
        return Err(MessageContentState::UnsupportedVersion {
            version: protocol_version,
        });
    }
    if ciphertext.len() < AES_GCM_TAG_LEN {
        return Err(MessageContentState::CorruptPayload);
    }
    MessageNonceId::try_from_slice(nonce_id).map_err(|_| MessageContentState::CorruptPayload)
}

/// A message ready to be sent over the network
#[derive(Debug, Clone)]
pub struct OutgoingMessage {
    pub protocol_version: u16,
    pub message_id: String,
    pub event_id: String,
    pub conversation_id: String,
    pub sender_peer_id: String,
    pub recipient_peer_id: String,
    pub nonce_id: [u8; MESSAGE_NONCE_ID_LEN],
    pub content_encrypted: Vec<u8>,
    pub content_type: String,
    pub reply_to: Option<String>,
    pub nonce_counter: u64,
    pub lamport_clock: u64,
    pub timestamp: i64,
    pub signature: Vec<u8>,
    /// Exact versioned CBOR envelope retained by the durable outbox.
    pub wire_payload: Vec<u8>,
}

/// An encrypted, signed, immutable edit ready for network transmission.
#[derive(Debug, Clone)]
pub struct OutgoingMessageEdit {
    pub protocol_version: u16,
    pub event_id: String,
    pub message_id: String,
    pub conversation_id: String,
    pub author_peer_id: String,
    pub recipient_peer_id: String,
    pub revision: u64,
    pub nonce_id: [u8; MESSAGE_NONCE_ID_LEN],
    pub content_encrypted: Vec<u8>,
    pub nonce_counter: u64,
    pub lamport_clock: u64,
    pub authored_at: i64,
    pub signature: Vec<u8>,
    /// Exact signed edit envelope retained for durable retry.
    pub wire_payload: Vec<u8>,
}

/// A durable delivery-state change that should be reflected in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDeliveryStateChange {
    pub message_id: String,
    pub status: String,
    pub timestamp: i64,
    pub error: Option<String>,
}

/// Parameters for processing an incoming message from the network
pub struct IncomingMessageParams<'a> {
    pub protocol_version: u16,
    pub message_id: &'a str,
    pub event_id: &'a str,
    pub conversation_id: &'a str,
    pub sender_peer_id: &'a str,
    pub recipient_peer_id: &'a str,
    pub nonce_id: &'a [u8],
    pub content_encrypted: &'a [u8],
    pub content_type: &'a str,
    pub reply_to: Option<&'a str>,
    pub nonce_counter: u64,
    pub lamport_clock: u64,
    pub timestamp: i64,
    pub signature: &'a [u8],
}

/// Parameters for a verified-shape encrypted edit received from the network.
pub struct IncomingMessageEditParams<'a> {
    pub protocol_version: u16,
    pub event_id: &'a str,
    pub message_id: &'a str,
    pub conversation_id: &'a str,
    pub author_peer_id: &'a str,
    pub recipient_peer_id: &'a str,
    pub revision: u64,
    pub nonce_id: &'a [u8],
    pub content_encrypted: &'a [u8],
    pub nonce_counter: u64,
    pub lamport_clock: u64,
    pub authored_at: i64,
    pub signature: &'a [u8],
}

impl MessagingService {
    /// Create a new messaging service
    pub fn new(
        db: Arc<Database>,
        identity_service: Arc<IdentityService>,
        contacts_service: Arc<ContactsService>,
        permissions_service: Arc<PermissionsService>,
    ) -> Self {
        Self {
            db,
            identity_service,
            contacts_service,
            permissions_service,
        }
    }

    /// Read the protocol policy from the selected profile database. Read
    /// receipts default off so a missing or newly-created preference cannot
    /// disclose message-reading activity.
    pub fn privacy_policy(&self) -> Result<MessagingPrivacyPolicy> {
        let enabled = self
            .db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT value FROM settings WHERE key = ?1",
                    [READ_RECEIPTS_POLICY_KEY],
                    |row| row.get::<_, String>(0),
                )
                .optional()
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))?
            .is_some_and(|value| value == "1");
        Ok(MessagingPrivacyPolicy {
            read_receipts_enabled: enabled,
        })
    }

    /// Persist the authoritative policy before reporting success to the UI.
    pub fn set_read_receipts_enabled(&self, enabled: bool) -> Result<MessagingPrivacyPolicy> {
        let now = chrono::Utc::now().timestamp();
        self.db
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO settings(key, value, updated_at) VALUES(?1, ?2, ?3)\
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                    rusqlite::params![READ_RECEIPTS_POLICY_KEY, if enabled { "1" } else { "0" }, now],
                )?;
                Ok(())
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
        Ok(MessagingPrivacyPolicy {
            read_receipts_enabled: enabled,
        })
    }

    /// Send a new message to a peer
    pub fn send_message(
        &self,
        recipient_peer_id: &str,
        content: &str,
        content_type: &str,
        reply_to: Option<&str>,
    ) -> Result<OutgoingMessage> {
        crate::services::IdentityPublishingPolicy::enforce(&self.db, &self.identity_service)?;
        // Get our identity
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        // Check we have chat permission with this peer
        if !self
            .permissions_service
            .peer_has_capability(recipient_peer_id, Capability::Chat)?
        {
            return Err(AppError::PermissionDenied(
                "No chat permission with this peer".to_string(),
            ));
        }

        // Get recipient's X25519 public key for encryption
        let x25519_public = self
            .contacts_service
            .get_x25519_public(recipient_peer_id)?
            .ok_or_else(|| AppError::NotFound("Contact not found".to_string()))?;

        // Get our X25519 keys
        let our_keys = self.identity_service.get_unlocked_keys()?;

        // Derive the direction-specific v2 traffic key. Reversing sender and
        // recipient produces an independent key on the other device.
        let conversation_id = derive_conversation_id(&identity.peer_id, recipient_peer_id);
        let their_public = X25519Public::from(
            <[u8; 32]>::try_from(x25519_public.as_slice())
                .map_err(|_| AppError::Crypto("Invalid X25519 key".to_string()))?,
        );
        let shared_secret = CryptoService::x25519_dh(&our_keys.x25519_secret, &their_public);
        let directional_key = derive_directional_message_key(
            &shared_secret,
            &conversation_id,
            &identity.peer_id,
            recipient_peer_id,
        )?;

        // Get next nonce counter
        let nonce_counter = self
            .db
            .next_send_counter(&conversation_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Create the immutable event identity before encryption so every bound
        // field participates in v2 AEAD material.
        let message_id = Uuid::new_v4().to_string();
        let event_id = message_id.clone();
        let context = MessageEventContext {
            protocol_version: MESSAGE_CRYPTO_VERSION,
            conversation_id: &conversation_id,
            sender_peer_id: &identity.peer_id,
            recipient_peer_id,
            message_id: &message_id,
            event_id: &event_id,
            kind: MessageEventKind::Create,
            revision: 0,
            nonce_counter,
        };
        let encrypted = encrypt_message_event(&directional_key, &context, content.as_bytes())?;
        let nonce_id = *encrypted.nonce_id.as_bytes();
        let content_encrypted = encrypted.ciphertext;

        let lamport_clock =
            self.db
                .next_lamport_clock(&identity.peer_id)
                .map_err(|e| AppError::DatabaseString(e.to_string()))? as u64;
        let timestamp = chrono::Utc::now().timestamp();

        // Create signable and sign
        tracing::info!(
            "MESSAGE SEND - sender_peer_id: {} (len={}), recipient_peer_id: {} (len={})",
            identity.peer_id,
            identity.peer_id.len(),
            recipient_peer_id,
            recipient_peer_id.len()
        );
        let signable = SignableDirectMessageV2 {
            protocol_version: MESSAGE_CRYPTO_VERSION,
            message_id: message_id.clone(),
            event_id: event_id.clone(),
            conversation_id: conversation_id.clone(),
            sender_peer_id: identity.peer_id.clone(),
            recipient_peer_id: recipient_peer_id.to_string(),
            nonce_id,
            content_encrypted: content_encrypted.clone(),
            content_type: content_type.to_string(),
            reply_to: reply_to.map(String::from),
            nonce_counter,
            lamport_clock,
            timestamp,
        };

        let signature = self.identity_service.sign(&signable)?;

        let wire_payload = MessagingCodec::encode(&MessagingMessage::MessageV2(DirectMessageV2 {
            protocol_version: MESSAGE_CRYPTO_VERSION,
            message_id: message_id.clone(),
            event_id: event_id.clone(),
            conversation_id: conversation_id.clone(),
            sender_peer_id: identity.peer_id.clone(),
            recipient_peer_id: recipient_peer_id.to_string(),
            nonce_id,
            content_encrypted: content_encrypted.clone(),
            content_type: content_type.to_string(),
            reply_to: reply_to.map(String::from),
            nonce_counter,
            lamport_clock,
            timestamp,
            signature: signature.clone(),
        }))
        .map_err(|error| AppError::Serialization(error.to_string()))?;

        // Persist the local message, immutable event, nonce claim, and exact
        // delivery bytes in one transaction before any network attempt.
        let msg_data = MessageData {
            protocol_version: MESSAGE_CRYPTO_VERSION,
            event_id: event_id.clone(),
            message_id: message_id.clone(),
            conversation_id: conversation_id.clone(),
            sender_peer_id: identity.peer_id.clone(),
            recipient_peer_id: recipient_peer_id.to_string(),
            nonce_id: nonce_id.to_vec(),
            content_encrypted: content_encrypted.clone(),
            content_type: content_type.to_string(),
            reply_to_message_id: reply_to.map(String::from),
            nonce_counter,
            lamport_clock: lamport_clock as i64,
            sent_at: timestamp,
            received_at: None,
            status: MessageStatus::Queued,
        };
        MessageOutboxRepository::new(&self.db)
            .commit_outgoing_create(
                &msg_data,
                &RecordMessageEventParams {
                    event_id: &event_id,
                    event_type: "sent",
                    message_id: &message_id,
                    conversation_id: &conversation_id,
                    sender_peer_id: &identity.peer_id,
                    recipient_peer_id,
                    lamport_clock: lamport_clock as i64,
                    timestamp,
                    payload_cbor: &wire_payload,
                    signature: &signature,
                },
                &EnqueueOutboxMessage {
                    event_id: &event_id,
                    message_id: &message_id,
                    peer_id: recipient_peer_id,
                    payload: &wire_payload,
                    max_attempts: None,
                    next_attempt_at: timestamp,
                    created_at: timestamp,
                },
            )
            .map_err(|error| AppError::Internal(format!("Could not queue message: {error}")))?;

        Ok(OutgoingMessage {
            protocol_version: MESSAGE_CRYPTO_VERSION,
            message_id,
            event_id,
            conversation_id,
            sender_peer_id: identity.peer_id,
            recipient_peer_id: recipient_peer_id.to_string(),
            nonce_id,
            content_encrypted,
            content_type: content_type.to_string(),
            reply_to: reply_to.map(String::from),
            nonce_counter,
            lamport_clock,
            timestamp,
            signature,
            wire_payload,
        })
    }

    /// Atomically claim a bounded batch of exact wire events for the network
    /// actor. Expired claims are recovered by the repository before selection.
    pub fn claim_due_outbox(
        &self,
        now: i64,
        lease_seconds: u32,
        limit: u32,
    ) -> Result<Vec<OutboxEntry>> {
        MessageOutboxRepository::new(&self.db)
            .claim_due(now, lease_seconds, limit)
            .map_err(Self::map_outbox_error)
    }

    /// Record that the exact bytes were handed to libp2p and are awaiting the
    /// correlated peer response. This is the user-visible `sent` boundary.
    pub fn record_outbox_sent(
        &self,
        event_id: &str,
        message_id: &str,
        now: i64,
    ) -> Result<Option<MessageDeliveryStateChange>> {
        let repository = MessageOutboxRepository::new(&self.db);
        let Some(entry) = repository.get(event_id).map_err(Self::map_outbox_error)? else {
            return Ok(None);
        };
        if entry.message_id != message_id {
            return Err(AppError::Validation(
                "Sent event does not match the queued message".to_string(),
            ));
        }
        repository
            .mark_sent(event_id, now.saturating_add(30), now)
            .map_err(Self::map_outbox_error)?;
        Ok(
            (entry.event_id == entry.message_id).then(|| MessageDeliveryStateChange {
                message_id: entry.message_id,
                status: "sent".to_string(),
                timestamp: now,
                error: None,
            }),
        )
    }

    /// Apply a peer-correlated successful request outcome to durable state.
    /// A successful protocol response is only produced after the receiver has
    /// authenticated and committed the event, so it is a delivery boundary.
    pub fn record_outbox_delivery(
        &self,
        receipt: &MessageDeliveryReceipt,
        now: i64,
    ) -> Result<Option<MessageDeliveryStateChange>> {
        let repository = MessageOutboxRepository::new(&self.db);
        let Some(entry) = repository
            .get(&receipt.event_id)
            .map_err(Self::map_outbox_error)?
        else {
            // Foreground protocol operations can use the same correlated
            // transport without belonging to the durable direct-message queue.
            return Ok(None);
        };
        if entry.message_id != receipt.message_id {
            return Err(AppError::Validation(
                "Delivery receipt does not match the queued message".to_string(),
            ));
        }

        match entry.state {
            OutboxState::InFlight => {
                repository
                    .mark_sent(&entry.event_id, now.saturating_add(60), now)
                    .map_err(Self::map_outbox_error)?;
                repository
                    .mark_delivered(&entry.event_id, now)
                    .map_err(Self::map_outbox_error)?;
            }
            OutboxState::Sent => {
                repository
                    .mark_delivered(&entry.event_id, now)
                    .map_err(Self::map_outbox_error)?;
            }
            OutboxState::Delivered | OutboxState::Read => return Ok(None),
            state => {
                return Err(AppError::Internal(format!(
                    "Delivery completed from invalid durable state: {}",
                    state.as_str()
                )));
            }
        }

        Ok(
            (entry.event_id == entry.message_id).then(|| MessageDeliveryStateChange {
                message_id: entry.message_id,
                status: "delivered".to_string(),
                timestamp: now,
                error: None,
            }),
        )
    }

    /// Apply one terminal transport attempt. Retryable outcomes are rescheduled
    /// with bounded exponential backoff; protocol rejection and invalid
    /// responses fail closed immediately.
    pub fn record_outbox_failure(
        &self,
        failure: &MessageDeliveryFailure,
        now: i64,
    ) -> Result<Option<MessageDeliveryStateChange>> {
        let repository = MessageOutboxRepository::new(&self.db);
        let Some(entry) = repository
            .get(&failure.event_id)
            .map_err(Self::map_outbox_error)?
        else {
            return Ok(None);
        };
        if entry.message_id != failure.message_id {
            return Err(AppError::Validation(
                "Delivery failure does not match the queued message".to_string(),
            ));
        }

        let detail = failure.stable_message();
        let state = if failure.kind.retryable() {
            let exponent = entry.attempt_count.saturating_sub(1).min(8);
            let delay = (1_i64 << exponent).min(300);
            repository
                .record_attempt_failure(&entry.event_id, &detail, now.saturating_add(delay), now)
                .map_err(Self::map_outbox_error)?
        } else {
            repository
                .fail_terminal(&entry.event_id, &detail, now)
                .map_err(Self::map_outbox_error)?;
            OutboxState::Failed
        };

        Ok(
            (entry.event_id == entry.message_id).then(|| MessageDeliveryStateChange {
                message_id: entry.message_id,
                status: match state {
                    OutboxState::Failed | OutboxState::Canceled => "failed",
                    _ => "queued",
                }
                .to_string(),
                timestamp: now,
                error: (state == OutboxState::Failed).then_some(detail),
            }),
        )
    }

    fn map_outbox_error(error: OutboxError) -> AppError {
        AppError::DatabaseString(error.to_string())
    }

    /// Process an incoming message from the network
    pub fn process_incoming_message(&self, params: &IncomingMessageParams<'_>) -> Result<()> {
        if params.protocol_version != MESSAGE_CRYPTO_VERSION {
            return Err(AppError::Validation(
                "Unsupported direct-message protocol version".to_string(),
            ));
        }
        let message_id = params.message_id;
        let event_id = params.event_id;
        let conversation_id = params.conversation_id;
        let sender_peer_id = params.sender_peer_id;
        let recipient_peer_id = params.recipient_peer_id;
        let content_encrypted = params.content_encrypted;
        let content_type = params.content_type;
        let reply_to = params.reply_to;
        let nonce_counter = params.nonce_counter;
        let lamport_clock = params.lamport_clock;
        let timestamp = params.timestamp;
        let signature = params.signature;
        if event_id != message_id {
            return Err(AppError::Validation(
                "Direct-message create event ID must equal its message ID".to_string(),
            ));
        }
        let nonce_id = MessageNonceId::try_from_slice(params.nonce_id)?;
        // Verify we are the recipient
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        tracing::info!(
            "MESSAGE RECEIVE - recipient in msg: {} (len={}) vs our identity: {} (len={})",
            recipient_peer_id,
            recipient_peer_id.len(),
            identity.peer_id,
            identity.peer_id.len()
        );

        if recipient_peer_id != identity.peer_id {
            tracing::error!(
                "MESSAGE REJECTED - peer ID mismatch. Message for {} but we are {}",
                recipient_peer_id,
                identity.peer_id
            );
            return Err(AppError::Validation("Message not for us".to_string()));
        }
        if derive_conversation_id(sender_peer_id, recipient_peer_id) != conversation_id {
            return Err(AppError::Validation(
                "Direct-message conversation binding is invalid".to_string(),
            ));
        }

        // Get sender's public key for verification
        tracing::info!("Looking up sender {} in contacts", sender_peer_id);
        let sender_public_key = self
            .contacts_service
            .get_public_key(sender_peer_id)?
            .ok_or_else(|| {
                tracing::error!(
                    "CONTACT LOOKUP FAILED - sender_peer_id {} not found in contacts",
                    sender_peer_id
                );
                AppError::NotFound("Sender not in contacts".to_string())
            })?;

        // Receiving a packet from a known contact is not itself authorization
        // to write it into chat history. The local user must currently grant
        // that contact the chat capability.
        if !self
            .permissions_service
            .peer_has_capability(sender_peer_id, Capability::Chat)?
        {
            return Err(AppError::PermissionDenied(
                "Sender does not have chat permission".to_string(),
            ));
        }

        // Verify signature
        let signable = SignableDirectMessageV2 {
            protocol_version: params.protocol_version,
            message_id: message_id.to_string(),
            event_id: event_id.to_string(),
            conversation_id: conversation_id.to_string(),
            sender_peer_id: sender_peer_id.to_string(),
            recipient_peer_id: recipient_peer_id.to_string(),
            nonce_id: *nonce_id.as_bytes(),
            content_encrypted: content_encrypted.to_vec(),
            content_type: content_type.to_string(),
            reply_to: reply_to.map(String::from),
            nonce_counter,
            lamport_clock,
            timestamp,
        };

        let verifying_key = VerifyingKey::from_bytes(
            sender_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto("Invalid message signature".to_string()));
        }

        // Authenticate the ciphertext before any replay state is consumed.
        let sender_x25519 = self
            .contacts_service
            .get_x25519_public(sender_peer_id)?
            .ok_or_else(|| AppError::NotFound("Sender encryption key not found".to_string()))?;
        let their_public = X25519Public::from(
            <[u8; 32]>::try_from(sender_x25519.as_slice())
                .map_err(|_| AppError::Crypto("Invalid X25519 key".to_string()))?,
        );
        let our_keys = self.identity_service.get_unlocked_keys()?;
        let shared_secret = CryptoService::x25519_dh(&our_keys.x25519_secret, &their_public);
        let directional_key = derive_directional_message_key(
            &shared_secret,
            conversation_id,
            sender_peer_id,
            recipient_peer_id,
        )?;
        let context = MessageEventContext {
            protocol_version: params.protocol_version,
            conversation_id,
            sender_peer_id,
            recipient_peer_id,
            message_id,
            event_id,
            kind: MessageEventKind::Create,
            revision: 0,
            nonce_counter,
        };
        let encrypted = EncryptedMessageEvent {
            nonce_id,
            ciphertext: content_encrypted.to_vec(),
        };
        let plaintext = decrypt_message_event(&directional_key, &context, &encrypted)?;
        String::from_utf8(plaintext)
            .map_err(|_| AppError::InvalidData("Message content is not UTF-8".to_string()))?;

        // Authentication is now complete. From this point onward there is one
        // persistence boundary: nonce/counter claim, message, immutable event,
        // and Lamport advance either all commit or none do.
        let durable_lamport_clock = i64::try_from(lamport_clock).map_err(|_| {
            AppError::Validation("Message Lamport clock exceeds durable range".to_string())
        })?;
        let received_at = chrono::Utc::now().timestamp();
        let msg_data = MessageData {
            protocol_version: params.protocol_version,
            event_id: event_id.to_string(),
            message_id: message_id.to_string(),
            conversation_id: conversation_id.to_string(),
            sender_peer_id: sender_peer_id.to_string(),
            recipient_peer_id: recipient_peer_id.to_string(),
            nonce_id: params.nonce_id.to_vec(),
            content_encrypted: content_encrypted.to_vec(),
            content_type: content_type.to_string(),
            reply_to_message_id: reply_to.map(String::from),
            nonce_counter,
            lamport_clock: durable_lamport_clock,
            sent_at: timestamp,
            received_at: Some(received_at),
            status: MessageStatus::Delivered,
        };
        let payload_cbor = signable.signable_bytes()?;
        MessagesRepository::commit_incoming_message(
            &self.db,
            &IncomingMessageCommit {
                message: &msg_data,
                payload_cbor: &payload_cbor,
                signature,
            },
        )
        .map_err(|error| match error {
            IncomingMessagePersistenceError::Database(error) => {
                AppError::DatabaseString(error.to_string())
            }
            replay @ (IncomingMessagePersistenceError::IdentityCollision(_)
            | IncomingMessagePersistenceError::NonceReplay
            | IncomingMessagePersistenceError::CounterReplay) => {
                AppError::Crypto(replay.to_string())
            }
            invalid @ (IncomingMessagePersistenceError::InvalidMessage(_)
            | IncomingMessagePersistenceError::IntegerOverflow(_)) => {
                AppError::Validation(invalid.to_string())
            }
        })?;

        // The request-response success already proves durable receipt to the
        // authenticated transport peer. Queue a signed receipt as well so the
        // sender can independently verify and persist the delivery transition.
        if let Err(error) = self.enqueue_message_ack(message_id, AckStatus::Delivered) {
            tracing::warn!("Could not queue signed delivery receipt for {message_id}: {error}");
        }

        Ok(())
    }

    fn enqueue_message_ack(&self, message_id: &str, status: AckStatus) -> Result<()> {
        let status_label = match status {
            AckStatus::Delivered => "delivered",
            AckStatus::Read => "read",
        };
        let event_id = format!("ack:{status_label}:{message_id}");
        let repository = MessageOutboxRepository::new(&self.db);
        if repository
            .get(&event_id)
            .map_err(Self::map_outbox_error)?
            .is_some()
        {
            return Ok(());
        }

        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;
        let message = MessagesRepository::get_by_message_id(&self.db, message_id)
            .map_err(|error| AppError::DatabaseString(error.to_string()))?
            .ok_or_else(|| AppError::NotFound("Message not found".to_string()))?;
        if message.recipient_peer_id != identity.peer_id {
            return Err(AppError::Validation(
                "Only the message recipient can acknowledge it".to_string(),
            ));
        }

        let timestamp = chrono::Utc::now().timestamp();
        let signable = SignableMessageAck {
            message_id: message_id.to_string(),
            conversation_id: message.conversation_id.clone(),
            ack_sender_peer_id: identity.peer_id.clone(),
            status: status_label.to_string(),
            timestamp,
        };
        let signature = self.identity_service.sign(&signable)?;
        let payload = MessagingCodec::encode(&MessagingMessage::Ack(MessageAck {
            message_id: message_id.to_string(),
            conversation_id: message.conversation_id,
            peer_id: identity.peer_id,
            status,
            timestamp,
            signature,
        }))
        .map_err(|error| AppError::Serialization(error.to_string()))?;
        repository
            .enqueue(&EnqueueOutboxMessage {
                event_id: &event_id,
                message_id,
                peer_id: &message.sender_peer_id,
                payload: &payload,
                max_attempts: None,
                next_attempt_at: timestamp,
                created_at: timestamp,
            })
            .map_err(Self::map_outbox_error)?;
        Ok(())
    }

    /// Process an incoming acknowledgment
    pub fn process_incoming_ack(
        &self,
        message_id: &str,
        conversation_id: &str,
        ack_sender_peer_id: &str,
        status: &str,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<()> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;
        let message = MessagesRepository::get_by_message_id(&self.db, message_id)
            .map_err(|error| AppError::DatabaseString(error.to_string()))?
            .ok_or_else(|| AppError::NotFound("Acknowledged message not found".to_string()))?;
        if message.sender_peer_id != identity.peer_id
            || message.recipient_peer_id != ack_sender_peer_id
            || message.conversation_id != conversation_id
        {
            return Err(AppError::Validation(
                "Acknowledgment does not match the original message route".to_string(),
            ));
        }
        if timestamp < message.sent_at.saturating_sub(300)
            || timestamp > chrono::Utc::now().timestamp().saturating_add(300)
        {
            return Err(AppError::Validation(
                "Acknowledgment timestamp is outside the accepted window".to_string(),
            ));
        }

        // Get the ack sender's public key
        let sender_public_key = self
            .contacts_service
            .get_public_key(ack_sender_peer_id)?
            .ok_or_else(|| AppError::NotFound("Ack sender not in contacts".to_string()))?;

        // Verify signature
        let signable = SignableMessageAck {
            message_id: message_id.to_string(),
            conversation_id: conversation_id.to_string(),
            ack_sender_peer_id: ack_sender_peer_id.to_string(),
            status: status.to_string(),
            timestamp,
        };

        let verifying_key = VerifyingKey::from_bytes(
            sender_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto("Invalid ack signature".to_string()));
        }

        let repository = MessageOutboxRepository::new(&self.db);
        let entry = repository
            .get(message_id)
            .map_err(Self::map_outbox_error)?
            .ok_or_else(|| AppError::NotFound("Queued message event not found".to_string()))?;
        let now = chrono::Utc::now().timestamp();
        let acknowledged_at = timestamp.max(message.sent_at);

        // Apply receipts monotonically. A signed receipt can race ahead of the
        // request-response frame, so it may legitimately complete an in-flight
        // attempt directly.
        let ensure_delivered = || -> Result<()> {
            let current = repository
                .get(message_id)
                .map_err(Self::map_outbox_error)?
                .ok_or_else(|| AppError::NotFound("Queued message event not found".to_string()))?;
            match current.state {
                OutboxState::InFlight => {
                    repository
                        .mark_sent(message_id, now.saturating_add(60), now)
                        .map_err(Self::map_outbox_error)?;
                    repository
                        .mark_delivered(message_id, acknowledged_at)
                        .map_err(Self::map_outbox_error)?;
                }
                OutboxState::Sent => {
                    repository
                        .mark_delivered(message_id, acknowledged_at)
                        .map_err(Self::map_outbox_error)?;
                }
                OutboxState::Delivered | OutboxState::Read => {}
                state => {
                    return Err(AppError::Validation(format!(
                        "Acknowledgment arrived while message was {}",
                        state.as_str()
                    )));
                }
            }
            Ok(())
        };

        match status {
            "delivered" => {
                ensure_delivered()?;
            }
            "read" => {
                ensure_delivered()?;
                if entry.state != OutboxState::Read {
                    repository
                        .mark_read(message_id, acknowledged_at)
                        .map_err(Self::map_outbox_error)?;
                }
            }
            _ => {
                return Err(AppError::Validation(format!(
                    "Invalid ack status: {}",
                    status
                )));
            }
        }

        Ok(())
    }

    /// Get messages for a conversation, decrypted
    pub fn get_conversation_messages(
        &self,
        peer_id: &str,
        limit: i64,
        before_timestamp: Option<i64>,
    ) -> Result<Vec<DecryptedMessage>> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        let conversation_id = derive_conversation_id(&identity.peer_id, peer_id);

        // Get encrypted messages
        let messages = MessagesRepository::get_conversation_messages(
            &self.db,
            &conversation_id,
            limit,
            before_timestamp,
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        let our_keys = self.identity_service.get_unlocked_keys()?;

        // Both directions share the X25519 secret, but derive distinct v2
        // traffic keys below from each stored event's signed direction.
        // Missing or malformed contact key material is represented per message
        // as `WrongKey`; it must not turn into authored-looking fallback text.
        let shared_secret = self
            .contacts_service
            .get_x25519_public(peer_id)?
            .and_then(|bytes| <[u8; 32]>::try_from(bytes.as_slice()).ok())
            .map(|bytes| {
                let their_public = X25519Public::from(bytes);
                CryptoService::x25519_dh(&our_keys.x25519_secret, &their_public)
            });

        // Authenticate each durable event again before decrypting it. This
        // catches local storage tampering after the message was accepted.
        let mut decrypted = Vec::new();
        for msg in messages {
            let edit = MessagesRepository::get_current_edit_event(&self.db, &msg.message_id)
                .map_err(|e| AppError::DatabaseString(e.to_string()))?;
            let edited_at = edit.as_ref().map(|event| event.timestamp);
            let sender_public_key = if msg.sender_peer_id == identity.peer_id {
                Some(identity.public_key.clone())
            } else {
                self.contacts_service.get_public_key(&msg.sender_peer_id)?
            };
            let create_signature =
                MessagesRepository::get_create_event_signature(&self.db, &msg.event_id)
                    .map_err(|e| AppError::DatabaseString(e.to_string()))?;

            let content_state = 'content: {
                let create_nonce = match inspect_encrypted_event_shape(
                    msg.protocol_version,
                    &msg.nonce_id,
                    &msg.content_encrypted,
                ) {
                    Ok(nonce) => nonce,
                    Err(state) => break 'content state,
                };
                let verifying_key = match sender_public_key.as_deref().and_then(|bytes| {
                    let bytes: &[u8; 32] = bytes.try_into().ok()?;
                    VerifyingKey::from_bytes(bytes).ok()
                }) {
                    Some(key) => key,
                    None => break 'content MessageContentState::WrongKey,
                };
                let create_signature = match create_signature.as_deref() {
                    Some(signature) => signature,
                    None => break 'content MessageContentState::CorruptPayload,
                };
                let create_lamport = match u64::try_from(msg.lamport_clock) {
                    Ok(clock) => clock,
                    Err(_) => break 'content MessageContentState::CorruptPayload,
                };
                let create_signable = SignableDirectMessageV2 {
                    protocol_version: msg.protocol_version,
                    message_id: msg.message_id.clone(),
                    event_id: msg.event_id.clone(),
                    conversation_id: msg.conversation_id.clone(),
                    sender_peer_id: msg.sender_peer_id.clone(),
                    recipient_peer_id: msg.recipient_peer_id.clone(),
                    nonce_id: *create_nonce.as_bytes(),
                    content_encrypted: msg.content_encrypted.clone(),
                    content_type: msg.content_type.clone(),
                    reply_to: msg.reply_to_message_id.clone(),
                    nonce_counter: msg.nonce_counter,
                    lamport_clock: create_lamport,
                    timestamp: msg.sent_at,
                };
                match verify(&verifying_key, &create_signable, create_signature) {
                    Ok(true) => {}
                    Ok(false) => break 'content MessageContentState::Tampered,
                    Err(_) => break 'content MessageContentState::CorruptPayload,
                }

                let (
                    protocol_version,
                    event_id,
                    nonce_id,
                    ciphertext,
                    nonce_counter,
                    revision,
                    kind,
                ) = if let Some(edit) = edit.as_ref() {
                    if edit.message_id != msg.message_id
                        || edit.conversation_id != msg.conversation_id
                        || edit.author_peer_id != msg.sender_peer_id
                        || edit.recipient_peer_id != msg.recipient_peer_id
                    {
                        break 'content MessageContentState::Tampered;
                    }
                    let nonce_id = match inspect_encrypted_event_shape(
                        edit.protocol_version,
                        &edit.nonce_id,
                        &edit.encrypted_content,
                    ) {
                        Ok(nonce) => nonce,
                        Err(state) => break 'content state,
                    };
                    let edit_signable = SignableMessageEditV2 {
                        protocol_version: edit.protocol_version,
                        event_id: edit.event_id.clone(),
                        message_id: edit.message_id.clone(),
                        conversation_id: edit.conversation_id.clone(),
                        author_peer_id: edit.author_peer_id.clone(),
                        recipient_peer_id: edit.recipient_peer_id.clone(),
                        revision: edit.revision,
                        nonce_id: *nonce_id.as_bytes(),
                        content_encrypted: edit.encrypted_content.clone(),
                        nonce_counter: edit.nonce_counter,
                        lamport_clock: edit.lamport_clock,
                        authored_at: edit.timestamp,
                    };
                    match verify(&verifying_key, &edit_signable, &edit.signature) {
                        Ok(true) => {}
                        Ok(false) => break 'content MessageContentState::Tampered,
                        Err(_) => break 'content MessageContentState::CorruptPayload,
                    }
                    (
                        edit.protocol_version,
                        edit.event_id.as_str(),
                        nonce_id,
                        edit.encrypted_content.as_slice(),
                        edit.nonce_counter,
                        edit.revision,
                        MessageEventKind::Edit,
                    )
                } else {
                    (
                        msg.protocol_version,
                        msg.event_id.as_str(),
                        create_nonce,
                        msg.content_encrypted.as_slice(),
                        msg.nonce_counter,
                        0,
                        MessageEventKind::Create,
                    )
                };

                let shared_secret = match shared_secret.as_ref() {
                    Some(secret) => secret,
                    None => break 'content MessageContentState::WrongKey,
                };
                let directional_key = match derive_directional_message_key(
                    shared_secret,
                    &msg.conversation_id,
                    &msg.sender_peer_id,
                    &msg.recipient_peer_id,
                ) {
                    Ok(key) => key,
                    Err(_) => break 'content MessageContentState::CorruptPayload,
                };
                let context = MessageEventContext {
                    protocol_version,
                    conversation_id: &msg.conversation_id,
                    sender_peer_id: &msg.sender_peer_id,
                    recipient_peer_id: &msg.recipient_peer_id,
                    message_id: &msg.message_id,
                    event_id,
                    kind,
                    revision,
                    nonce_counter,
                };
                let encrypted = EncryptedMessageEvent {
                    nonce_id,
                    ciphertext: ciphertext.to_vec(),
                };
                match decrypt_message_event(&directional_key, &context, &encrypted) {
                    Ok(bytes) => match String::from_utf8(bytes) {
                        Ok(text) => MessageContentState::Plaintext { text },
                        Err(_) => MessageContentState::CorruptPayload,
                    },
                    Err(_) => MessageContentState::WrongKey,
                }
            };

            decrypted.push(DecryptedMessage {
                message_id: msg.message_id,
                conversation_id: msg.conversation_id,
                sender_peer_id: msg.sender_peer_id.clone(),
                recipient_peer_id: msg.recipient_peer_id,
                content_state,
                content_type: msg.content_type,
                reply_to_message_id: msg.reply_to_message_id,
                sent_at: msg.sent_at,
                delivered_at: msg.delivered_at,
                read_at: msg.read_at,
                status: msg.status,
                is_outgoing: msg.sender_peer_id == identity.peer_id,
                edited_at,
            });
        }

        Ok(decrypted)
    }

    /// Get all conversations
    pub fn get_conversations(&self) -> Result<Vec<Conversation>> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        MessagesRepository::get_conversations(&self.db, &identity.peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Mark a conversation as read
    pub fn mark_conversation_read(&self, peer_id: &str) -> Result<i64> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        let conversation_id = derive_conversation_id(&identity.peer_id, peer_id);
        let timestamp = chrono::Utc::now().timestamp();

        let unread_message_ids = MessagesRepository::get_conversation_messages(
            &self.db,
            &conversation_id,
            i64::MAX,
            None,
        )
        .map_err(|error| AppError::DatabaseString(error.to_string()))?
        .into_iter()
        .filter(|message| {
            message.recipient_peer_id == identity.peer_id
                && matches!(message.status.as_str(), "sent" | "delivered")
        })
        .map(|message| message.message_id)
        .collect::<Vec<_>>();

        // Read activity is disclosed only when the authoritative profile
        // policy allows it. Local unread state still advances when disabled.
        if self.privacy_policy()?.read_receipts_enabled {
            // Queue verifiable read receipts before changing local state. Each
            // receipt has a deterministic event ID, so retries and repeated UI
            // actions remain idempotent.
            for message_id in &unread_message_ids {
                self.enqueue_message_ack(message_id, AckStatus::Read)?;
            }
        }

        MessagesRepository::mark_conversation_read(
            &self.db,
            &conversation_id,
            &identity.peer_id,
            timestamp,
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get unread count for a conversation
    pub fn get_unread_count(&self, peer_id: &str) -> Result<i64> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        let conversation_id = derive_conversation_id(&identity.peer_id, peer_id);

        MessagesRepository::get_unread_count(&self.db, &conversation_id, &identity.peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Update message status (for network events)
    pub fn update_message_status(&self, message_id: &str, status: MessageStatus) -> Result<bool> {
        MessagesRepository::update_status(&self.db, message_id, status)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Clear all messages in a conversation (keeps the conversation visible if new messages arrive)
    pub fn clear_conversation_history(&self, peer_id: &str) -> Result<i64> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let conversation_id = derive_conversation_id(&identity.peer_id, peer_id);

        MessagesRepository::clear_conversation_messages(&self.db, &conversation_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Create, sign, and durably append an encrypted immutable edit event.
    pub fn edit_message(&self, message_id: &str, new_content: &str) -> Result<OutgoingMessageEdit> {
        crate::services::IdentityPublishingPolicy::enforce(&self.db, &self.identity_service)?;
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        // Get the original message
        let original = MessagesRepository::get_by_message_id(&self.db, message_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Message not found".to_string()))?;

        // Verify we are the sender
        if original.sender_peer_id != identity.peer_id {
            return Err(AppError::PermissionDenied(
                "Can only edit your own messages".to_string(),
            ));
        }

        let peer_id = original.recipient_peer_id.clone();

        // Get peer's X25519 key for encryption
        let x25519_public = self
            .contacts_service
            .get_x25519_public(&peer_id)?
            .ok_or_else(|| AppError::NotFound("Contact not found".to_string()))?;

        let our_keys = self.identity_service.get_unlocked_keys()?;

        let their_public = X25519Public::from(
            <[u8; 32]>::try_from(x25519_public.as_slice())
                .map_err(|_| AppError::Crypto("Invalid X25519 key".to_string()))?,
        );
        let shared_secret = CryptoService::x25519_dh(&our_keys.x25519_secret, &their_public);
        let directional_key = derive_directional_message_key(
            &shared_secret,
            &original.conversation_id,
            &identity.peer_id,
            &peer_id,
        )?;

        // Revision reservation is a separate committed transaction. A failed
        // encryption/sign/send cannot cause that revision to be reused.
        let revision = MessagesRepository::reserve_edit_revision(
            &self.db,
            message_id,
            &identity.peer_id,
            &peer_id,
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        let nonce_counter = self
            .db
            .next_send_counter(&original.conversation_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        let event_id = Uuid::new_v4().to_string();
        let lamport_clock =
            self.db
                .next_lamport_clock(&identity.peer_id)
                .map_err(|e| AppError::DatabaseString(e.to_string()))? as u64;
        let authored_at = chrono::Utc::now().timestamp();
        let context = MessageEventContext {
            protocol_version: MESSAGE_CRYPTO_VERSION,
            conversation_id: &original.conversation_id,
            sender_peer_id: &identity.peer_id,
            recipient_peer_id: &peer_id,
            message_id,
            event_id: &event_id,
            kind: MessageEventKind::Edit,
            revision,
            nonce_counter,
        };
        let encrypted = encrypt_message_event(&directional_key, &context, new_content.as_bytes())?;
        let nonce_id = *encrypted.nonce_id.as_bytes();
        let signable = SignableMessageEditV2 {
            protocol_version: MESSAGE_CRYPTO_VERSION,
            event_id: event_id.clone(),
            message_id: message_id.to_string(),
            conversation_id: original.conversation_id.clone(),
            author_peer_id: identity.peer_id.clone(),
            recipient_peer_id: peer_id.clone(),
            revision,
            nonce_id,
            content_encrypted: encrypted.ciphertext.clone(),
            nonce_counter,
            lamport_clock,
            authored_at,
        };
        let signature = self.identity_service.sign(&signable)?;
        let wire_payload = MessagingCodec::encode(&MessagingMessage::EditV2(
            crate::p2p::protocols::messaging::MessageEditV2 {
                protocol_version: MESSAGE_CRYPTO_VERSION,
                event_id: event_id.clone(),
                message_id: message_id.to_string(),
                conversation_id: original.conversation_id.clone(),
                author_peer_id: identity.peer_id.clone(),
                recipient_peer_id: peer_id.clone(),
                revision,
                nonce_id,
                content_encrypted: encrypted.ciphertext.clone(),
                nonce_counter,
                lamport_clock,
                authored_at,
                signature: signature.clone(),
            },
        ))
        .map_err(|error| AppError::Serialization(error.to_string()))?;
        let edit_event = MessageEditEventData {
            event_id: event_id.clone(),
            protocol_version: MESSAGE_CRYPTO_VERSION,
            message_id: message_id.to_string(),
            conversation_id: original.conversation_id.clone(),
            author_peer_id: identity.peer_id.clone(),
            recipient_peer_id: peer_id.clone(),
            revision,
            nonce_id: nonce_id.to_vec(),
            nonce_counter,
            lamport_clock,
            encrypted_content: encrypted.ciphertext.clone(),
            signature: signature.clone(),
            timestamp: authored_at,
        };
        MessageOutboxRepository::new(&self.db)
            .commit_outgoing_edit(
                &edit_event,
                &EnqueueOutboxMessage {
                    event_id: &event_id,
                    message_id,
                    peer_id: &peer_id,
                    payload: &wire_payload,
                    max_attempts: None,
                    next_attempt_at: authored_at,
                    created_at: authored_at,
                },
            )
            .map_err(Self::map_outbox_error)?;

        Ok(OutgoingMessageEdit {
            protocol_version: MESSAGE_CRYPTO_VERSION,
            event_id,
            message_id: message_id.to_string(),
            conversation_id: original.conversation_id,
            author_peer_id: identity.peer_id,
            recipient_peer_id: peer_id,
            revision,
            nonce_id,
            content_encrypted: encrypted.ciphertext,
            nonce_counter,
            lamport_clock,
            authored_at,
            signature,
            wire_payload,
        })
    }

    /// Authenticate, decrypt, and durably append an incoming v2 edit event.
    pub fn apply_incoming_edit(&self, params: &IncomingMessageEditParams<'_>) -> Result<()> {
        if params.protocol_version != MESSAGE_CRYPTO_VERSION {
            return Err(AppError::Validation(
                "Unsupported message-edit protocol version".to_string(),
            ));
        }
        let nonce_id = MessageNonceId::try_from_slice(params.nonce_id)?;
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        // Get the original message
        if params.recipient_peer_id != identity.peer_id {
            return Err(AppError::Validation(
                "Message edit is not for us".to_string(),
            ));
        }

        let original = MessagesRepository::get_by_message_id(&self.db, params.message_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Message not found".to_string()))?;

        if original.sender_peer_id != params.author_peer_id
            || original.recipient_peer_id != params.recipient_peer_id
            || original.conversation_id != params.conversation_id
        {
            return Err(AppError::Validation(
                "Message edit does not match its original message".to_string(),
            ));
        }

        let sender_public_key = self
            .contacts_service
            .get_public_key(params.author_peer_id)?
            .ok_or_else(|| AppError::NotFound("Edit author not in contacts".to_string()))?;
        let verifying_key = VerifyingKey::from_bytes(
            sender_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {e}")))?;
        let signable = SignableMessageEditV2 {
            protocol_version: params.protocol_version,
            event_id: params.event_id.to_string(),
            message_id: params.message_id.to_string(),
            conversation_id: params.conversation_id.to_string(),
            author_peer_id: params.author_peer_id.to_string(),
            recipient_peer_id: params.recipient_peer_id.to_string(),
            revision: params.revision,
            nonce_id: *nonce_id.as_bytes(),
            content_encrypted: params.content_encrypted.to_vec(),
            nonce_counter: params.nonce_counter,
            lamport_clock: params.lamport_clock,
            authored_at: params.authored_at,
        };
        if !verify(&verifying_key, &signable, params.signature)? {
            return Err(AppError::Crypto(
                "Invalid message-edit signature".to_string(),
            ));
        }

        let x25519_public = self
            .contacts_service
            .get_x25519_public(params.author_peer_id)?
            .ok_or_else(|| {
                AppError::NotFound("Edit author encryption key not found".to_string())
            })?;
        let our_keys = self.identity_service.get_unlocked_keys()?;
        let their_public = X25519Public::from(
            <[u8; 32]>::try_from(x25519_public.as_slice())
                .map_err(|_| AppError::Crypto("Invalid X25519 key".to_string()))?,
        );
        let shared_secret = CryptoService::x25519_dh(&our_keys.x25519_secret, &their_public);
        let directional_key = derive_directional_message_key(
            &shared_secret,
            params.conversation_id,
            params.author_peer_id,
            params.recipient_peer_id,
        )?;
        let context = MessageEventContext {
            protocol_version: params.protocol_version,
            conversation_id: params.conversation_id,
            sender_peer_id: params.author_peer_id,
            recipient_peer_id: params.recipient_peer_id,
            message_id: params.message_id,
            event_id: params.event_id,
            kind: MessageEventKind::Edit,
            revision: params.revision,
            nonce_counter: params.nonce_counter,
        };
        let plaintext = decrypt_message_event(
            &directional_key,
            &context,
            &EncryptedMessageEvent {
                nonce_id,
                ciphertext: params.content_encrypted.to_vec(),
            },
        )?;
        String::from_utf8(plaintext).map_err(|_| {
            AppError::InvalidData("Edited message content is not UTF-8".to_string())
        })?;

        let outcome = MessagesRepository::record_verified_edit_event(
            &self.db,
            &MessageEditEventData {
                event_id: params.event_id.to_string(),
                protocol_version: params.protocol_version,
                message_id: params.message_id.to_string(),
                conversation_id: params.conversation_id.to_string(),
                author_peer_id: params.author_peer_id.to_string(),
                recipient_peer_id: params.recipient_peer_id.to_string(),
                revision: params.revision,
                nonce_id: params.nonce_id.to_vec(),
                nonce_counter: params.nonce_counter,
                lamport_clock: params.lamport_clock,
                encrypted_content: params.content_encrypted.to_vec(),
                signature: params.signature.to_vec(),
                timestamp: params.authored_at,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        if outcome == EditEventApplyOutcome::Inserted {
            self.db
                .update_lamport_clock(params.author_peer_id, params.lamport_clock as i64)
                .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        }

        Ok(())
    }

    /// Get the database reference (for testing)
    #[cfg(test)]
    pub fn db(&self) -> &Database {
        &self.db
    }

    /// Delete a conversation and all its messages entirely
    pub fn delete_conversation(&self, peer_id: &str) -> Result<i64> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let conversation_id = derive_conversation_id(&identity.peer_id, peer_id);

        MessagesRepository::delete_conversation(&self.db, &conversation_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Capability, ContactData, ContactsRepository};
    use crate::models::{CreateIdentityRequest, IdentityInfo};
    use crate::services::{ContactsService, CryptoService, PermissionsService};
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use std::path::Path;
    use std::sync::Arc;

    const TEST_PASSWORD: &str = "test-pass";

    struct TestNode {
        messaging: MessagingService,
        identity: Arc<IdentityService>,
        contacts: Arc<ContactsService>,
        permissions: Arc<PermissionsService>,
        info: IdentityInfo,
    }

    fn create_node(db: Arc<Database>, name: &str) -> TestNode {
        let identity = Arc::new(IdentityService::new(db.clone()));
        let contacts = Arc::new(ContactsService::new(db.clone(), identity.clone()));
        let permissions = Arc::new(PermissionsService::new(db.clone(), identity.clone()));
        let info = identity
            .create_identity(CreateIdentityRequest {
                display_name: name.to_string(),
                passphrase: TEST_PASSWORD.to_string(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO identity_publishing_state(peer_id, mode, updated_at) VALUES(?, 'unverified', 1)",
                [&info.peer_id],
            )?;
            Ok(())
        })
        .unwrap();
        let messaging =
            MessagingService::new(db, identity.clone(), contacts.clone(), permissions.clone());
        TestNode {
            messaging,
            identity,
            contacts,
            permissions,
            info,
        }
    }

    fn reopen_node(path: &Path) -> TestNode {
        let db = Arc::new(Database::new(path.to_path_buf()).unwrap());
        let identity = Arc::new(IdentityService::new(db.clone()));
        let info = identity.unlock(TEST_PASSWORD).unwrap();
        let contacts = Arc::new(ContactsService::new(db.clone(), identity.clone()));
        let permissions = Arc::new(PermissionsService::new(db.clone(), identity.clone()));
        let messaging =
            MessagingService::new(db, identity.clone(), contacts.clone(), permissions.clone());
        TestNode {
            messaging,
            identity,
            contacts,
            permissions,
            info,
        }
    }

    fn add_contact(node: &TestNode, contact: &TestNode) {
        node.contacts
            .add_contact(
                &contact.info.peer_id,
                &STANDARD.decode(&contact.info.public_key).unwrap(),
                &STANDARD.decode(&contact.info.x25519_public).unwrap(),
                &contact.info.display_name,
                None,
                None,
            )
            .unwrap();
    }

    fn grant_chat(node: &TestNode, peer_id: &str) {
        node.permissions
            .create_permission_grant(peer_id, Capability::Chat, None)
            .unwrap();
    }

    fn connect_for_chat(first: &TestNode, second: &TestNode) {
        add_contact(first, second);
        add_contact(second, first);
        grant_chat(first, &second.info.peer_id);
        grant_chat(second, &first.info.peer_id);
    }

    fn receive(service: &MessagingService, message: &OutgoingMessage) -> Result<()> {
        service.process_incoming_message(&IncomingMessageParams {
            protocol_version: message.protocol_version,
            message_id: &message.message_id,
            event_id: &message.event_id,
            conversation_id: &message.conversation_id,
            sender_peer_id: &message.sender_peer_id,
            recipient_peer_id: &message.recipient_peer_id,
            nonce_id: &message.nonce_id,
            content_encrypted: &message.content_encrypted,
            content_type: &message.content_type,
            reply_to: message.reply_to.as_deref(),
            nonce_counter: message.nonce_counter,
            lamport_clock: message.lamport_clock,
            timestamp: message.timestamp,
            signature: &message.signature,
        })
    }

    fn resign(node: &TestNode, message: &mut OutgoingMessage) {
        message.signature = node
            .identity
            .sign(&SignableDirectMessageV2 {
                protocol_version: message.protocol_version,
                message_id: message.message_id.clone(),
                event_id: message.event_id.clone(),
                conversation_id: message.conversation_id.clone(),
                sender_peer_id: message.sender_peer_id.clone(),
                recipient_peer_id: message.recipient_peer_id.clone(),
                nonce_id: message.nonce_id,
                content_encrypted: message.content_encrypted.clone(),
                content_type: message.content_type.clone(),
                reply_to: message.reply_to.clone(),
                nonce_counter: message.nonce_counter,
                lamport_clock: message.lamport_clock,
                timestamp: message.timestamp,
            })
            .unwrap();
    }

    /// Set up two identities (ours and a peer) and return the service plus metadata.
    /// The peer is added as a contact with chat permission granted.
    fn create_test_env() -> (
        MessagingService,
        Arc<IdentityService>,
        String, // our peer_id
        String, // peer's peer_id
    ) {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
        let permissions_service = Arc::new(PermissionsService::new(
            db.clone(),
            identity_service.clone(),
        ));

        // Create our identity
        let info = identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Our User".to_string(),
                passphrase: "test-pass".to_string(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        let our_peer_id = info.peer_id;
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO identity_publishing_state(peer_id, mode, updated_at) VALUES(?, 'unverified', 1)",
                [&our_peer_id],
            )?;
            Ok(())
        })
        .unwrap();

        // Create a fake peer with X25519 keys
        let (_peer_ed25519, peer_verifying) = CryptoService::generate_ed25519_keypair();
        let (_peer_x25519_secret, peer_x25519_public) = CryptoService::generate_x25519_keypair();

        let peer_peer_id = "12D3KooWPeerTest123456789".to_string();

        // Add the peer as a contact
        let contact_data = ContactData {
            peer_id: peer_peer_id.clone(),
            public_key: peer_verifying.to_bytes().to_vec(),
            x25519_public: peer_x25519_public.to_bytes().to_vec(),
            display_name: "Peer User".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        // Grant chat permission to the peer
        permissions_service
            .create_permission_grant(&peer_peer_id, Capability::Chat, None)
            .unwrap();

        let messaging_service = MessagingService::new(
            db,
            identity_service.clone(),
            contacts_service,
            permissions_service,
        );

        (
            messaging_service,
            identity_service,
            our_peer_id,
            peer_peer_id,
        )
    }

    #[test]
    fn test_send_message_success() {
        let (service, _identity, our_peer_id, peer_peer_id) = create_test_env();

        let msg = service
            .send_message(&peer_peer_id, "Hello!", "text", None)
            .unwrap();

        assert!(!msg.message_id.is_empty());
        assert_eq!(msg.sender_peer_id, our_peer_id);
        assert_eq!(msg.recipient_peer_id, peer_peer_id);
        assert!(!msg.content_encrypted.is_empty());
        assert!(!msg.signature.is_empty());
        assert_eq!(msg.content_type, "text");

        let queued = MessageOutboxRepository::new(service.db())
            .get(&msg.event_id)
            .unwrap()
            .expect("outgoing message must be durable before send returns");
        assert_eq!(queued.state, OutboxState::Queued);
        assert_eq!(queued.message_id, msg.message_id);
        assert_eq!(queued.payload, msg.wire_payload);

        let stored = MessagesRepository::get_by_message_id(service.db(), &msg.message_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, "queued");
    }

    #[test]
    fn correlated_delivery_outcome_updates_outbox_and_message_atomically() {
        let (service, _identity, _our_peer_id, peer_peer_id) = create_test_env();
        let message = service
            .send_message(&peer_peer_id, "Hello!", "text", None)
            .unwrap();
        let claimed = service.claim_due_outbox(message.timestamp, 30, 1).unwrap();
        assert_eq!(claimed.len(), 1);

        let sent = service
            .record_outbox_sent(&message.event_id, &message.message_id, message.timestamp)
            .unwrap()
            .unwrap();
        assert_eq!(sent.status, "sent");

        let change = service
            .record_outbox_delivery(
                &MessageDeliveryReceipt {
                    event_id: message.event_id.clone(),
                    message_id: message.message_id.clone(),
                },
                message.timestamp + 1,
            )
            .unwrap()
            .unwrap();
        assert_eq!(change.status, "delivered");

        let queued = MessageOutboxRepository::new(service.db())
            .get(&message.event_id)
            .unwrap()
            .unwrap();
        assert_eq!(queued.state, OutboxState::Delivered);
        let stored = MessagesRepository::get_by_message_id(service.db(), &message.message_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, "delivered");
        assert_eq!(stored.delivered_at, Some(message.timestamp + 1));
    }

    #[test]
    fn non_retryable_delivery_failure_is_terminal() {
        let (service, _identity, _our_peer_id, peer_peer_id) = create_test_env();
        let message = service
            .send_message(&peer_peer_id, "Hello!", "text", None)
            .unwrap();
        service.claim_due_outbox(message.timestamp, 30, 1).unwrap();
        service
            .record_outbox_sent(&message.event_id, &message.message_id, message.timestamp)
            .unwrap();

        let change = service
            .record_outbox_failure(
                &MessageDeliveryFailure {
                    event_id: message.event_id.clone(),
                    message_id: message.message_id.clone(),
                    kind: crate::p2p::types::MessageDeliveryFailureKind::InvalidResponse,
                    detail: "mismatched peer response".to_string(),
                },
                message.timestamp + 1,
            )
            .unwrap()
            .unwrap();
        assert_eq!(change.status, "failed");
        assert!(change.error.unwrap().contains("MESSAGE_INVALID_RESPONSE"));

        let queued = MessageOutboxRepository::new(service.db())
            .get(&message.event_id)
            .unwrap()
            .unwrap();
        assert_eq!(queued.state, OutboxState::Failed);
    }

    #[test]
    fn retryable_failure_after_send_requeues_with_backoff() {
        let (service, _identity, _our_peer_id, peer_peer_id) = create_test_env();
        let message = service
            .send_message(&peer_peer_id, "retry", "text", None)
            .unwrap();
        service.claim_due_outbox(message.timestamp, 30, 1).unwrap();
        service
            .record_outbox_sent(&message.event_id, &message.message_id, message.timestamp)
            .unwrap();

        let change = service
            .record_outbox_failure(
                &MessageDeliveryFailure {
                    event_id: message.event_id.clone(),
                    message_id: message.message_id.clone(),
                    kind: crate::p2p::types::MessageDeliveryFailureKind::Network,
                    detail: "connection reset".to_string(),
                },
                message.timestamp + 1,
            )
            .unwrap()
            .unwrap();
        assert_eq!(change.status, "queued");
        assert_eq!(change.error, None);

        let queued = MessageOutboxRepository::new(service.db())
            .get(&message.event_id)
            .unwrap()
            .unwrap();
        assert_eq!(queued.state, OutboxState::Queued);
        assert_eq!(queued.attempt_count, 1);
        assert_eq!(queued.next_attempt_at, message.timestamp + 2);
    }

    #[test]
    fn signed_delivery_ack_advances_only_the_outgoing_create() {
        let sender_dir = tempfile::tempdir().unwrap();
        let receiver_dir = tempfile::tempdir().unwrap();
        let sender = create_node(
            Arc::new(Database::new(sender_dir.path().join("sender.db")).unwrap()),
            "sender",
        );
        let receiver = create_node(
            Arc::new(Database::new(receiver_dir.path().join("receiver.db")).unwrap()),
            "receiver",
        );
        connect_for_chat(&sender, &receiver);

        let message = sender
            .messaging
            .send_message(&receiver.info.peer_id, "receipt", "text", None)
            .unwrap();
        sender
            .messaging
            .claim_due_outbox(message.timestamp, 30, 1)
            .unwrap();
        receive(&receiver.messaging, &message).unwrap();

        let ack_entry = MessageOutboxRepository::new(receiver.messaging.db())
            .get(&format!("ack:delivered:{}", message.message_id))
            .unwrap()
            .expect("receiver must durably queue a signed delivery receipt");
        assert_eq!(ack_entry.state, OutboxState::Queued);
        let received =
            MessagesRepository::get_by_message_id(receiver.messaging.db(), &message.message_id)
                .unwrap()
                .unwrap();
        assert_eq!(received.status, "delivered");

        receiver.messaging.set_read_receipts_enabled(true).unwrap();

        let MessagingMessage::Ack(ack) = MessagingCodec::decode(&ack_entry.payload).unwrap() else {
            panic!("queued receipt must use the signed ACK wire format");
        };
        sender
            .messaging
            .process_incoming_ack(
                &ack.message_id,
                &ack.conversation_id,
                &ack.peer_id,
                "delivered",
                ack.timestamp,
                &ack.signature,
            )
            .unwrap();
        let sender_outbox = MessageOutboxRepository::new(sender.messaging.db())
            .get(&message.event_id)
            .unwrap()
            .unwrap();
        assert_eq!(sender_outbox.state, OutboxState::Delivered);

        assert_eq!(
            receiver
                .messaging
                .mark_conversation_read(&sender.info.peer_id)
                .unwrap(),
            1
        );
        let read_entry = MessageOutboxRepository::new(receiver.messaging.db())
            .get(&format!("ack:read:{}", message.message_id))
            .unwrap()
            .expect("reading an incoming message must queue a signed receipt");
        let MessagingMessage::Ack(read_ack) = MessagingCodec::decode(&read_entry.payload).unwrap()
        else {
            panic!("queued read receipt must use the signed ACK wire format");
        };
        sender
            .messaging
            .process_incoming_ack(
                &read_ack.message_id,
                &read_ack.conversation_id,
                &read_ack.peer_id,
                "read",
                read_ack.timestamp,
                &read_ack.signature,
            )
            .unwrap();
        let sender_outbox = MessageOutboxRepository::new(sender.messaging.db())
            .get(&message.event_id)
            .unwrap()
            .unwrap();
        assert_eq!(sender_outbox.state, OutboxState::Read);
    }

    #[test]
    fn disabled_read_receipts_survive_restart_and_never_enter_direct_or_relay_wire_outbox() {
        let sender_dir = tempfile::tempdir().unwrap();
        let receiver_dir = tempfile::tempdir().unwrap();
        let receiver_db_path = receiver_dir.path().join("receiver.db");
        let sender = create_node(
            Arc::new(Database::new(sender_dir.path().join("sender.db")).unwrap()),
            "sender",
        );
        let receiver = create_node(
            Arc::new(Database::new(receiver_db_path.clone()).unwrap()),
            "receiver",
        );
        connect_for_chat(&sender, &receiver);

        let message = sender
            .messaging
            .send_message(&receiver.info.peer_id, "private reading", "text", None)
            .unwrap();
        receive(&receiver.messaging, &message).unwrap();
        assert_eq!(
            receiver.messaging.privacy_policy().unwrap(),
            MessagingPrivacyPolicy {
                read_receipts_enabled: false,
            }
        );
        receiver.messaging.set_read_receipts_enabled(false).unwrap();
        drop(receiver);

        let reopened = reopen_node(&receiver_db_path);
        assert!(
            !reopened
                .messaging
                .privacy_policy()
                .unwrap()
                .read_receipts_enabled
        );
        assert_eq!(
            reopened
                .messaging
                .mark_conversation_read(&sender.info.peer_id)
                .unwrap(),
            1
        );

        let repository = MessageOutboxRepository::new(reopened.messaging.db());
        assert!(repository
            .get(&format!("ack:read:{}", message.message_id))
            .unwrap()
            .is_none());

        // Direct and relay delivery use this same durable wire outbox. The
        // delivery acknowledgement may be present, but no claimed frame may
        // disclose that the message was read.
        for entry in reopened
            .messaging
            .claim_due_outbox(chrono::Utc::now().timestamp(), 30, 32)
            .unwrap()
        {
            if let MessagingMessage::Ack(ack) = MessagingCodec::decode(&entry.payload).unwrap() {
                assert_ne!(ack.status, AckStatus::Read);
            }
        }
    }

    #[test]
    fn encrypted_edit_is_queued_as_its_exact_wire_event() {
        let sender_dir = tempfile::tempdir().unwrap();
        let receiver_dir = tempfile::tempdir().unwrap();
        let sender = create_node(
            Arc::new(Database::new(sender_dir.path().join("sender.db")).unwrap()),
            "sender",
        );
        let receiver = create_node(
            Arc::new(Database::new(receiver_dir.path().join("receiver.db")).unwrap()),
            "receiver",
        );
        connect_for_chat(&sender, &receiver);
        let message = sender
            .messaging
            .send_message(&receiver.info.peer_id, "before", "text", None)
            .unwrap();
        receive(&receiver.messaging, &message).unwrap();

        let edit = sender
            .messaging
            .edit_message(&message.message_id, "after")
            .unwrap();
        let queued = MessageOutboxRepository::new(sender.messaging.db())
            .get(&edit.event_id)
            .unwrap()
            .expect("edit must be durable before the service returns");
        assert_eq!(queued.state, OutboxState::Queued);
        assert_eq!(queued.message_id, message.message_id);
        assert_eq!(queued.payload, edit.wire_payload);
        let MessagingMessage::EditV2(encoded) = MessagingCodec::decode(&queued.payload).unwrap()
        else {
            panic!("queued edit must use the encrypted v2 wire envelope");
        };
        assert_eq!(encoded.event_id, edit.event_id);
        assert_eq!(encoded.revision, edit.revision);
    }

    #[test]
    fn test_send_message_requires_identity() {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
        let permissions_service = Arc::new(PermissionsService::new(
            db.clone(),
            identity_service.clone(),
        ));
        let service =
            MessagingService::new(db, identity_service, contacts_service, permissions_service);

        let result = service.send_message("12D3KooWPeer", "Hello!", "text", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_send_message_no_permission_fails() {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
        let permissions_service = Arc::new(PermissionsService::new(
            db.clone(),
            identity_service.clone(),
        ));

        let info = identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Test".to_string(),
                passphrase: "test-pass".to_string(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO identity_publishing_state(peer_id, mode, updated_at) VALUES(?, 'unverified', 1)",
                [&info.peer_id],
            )?;
            Ok(())
        })
        .unwrap();

        let service =
            MessagingService::new(db, identity_service, contacts_service, permissions_service);

        // No permission granted to this peer
        let result = service.send_message("12D3KooWUnknownPeer", "Hello!", "text", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_send_multiple_messages_increment_counters() {
        let (service, _identity, _our_peer_id, peer_peer_id) = create_test_env();

        let msg1 = service
            .send_message(&peer_peer_id, "First", "text", None)
            .unwrap();
        let msg2 = service
            .send_message(&peer_peer_id, "Second", "text", None)
            .unwrap();

        assert!(msg2.nonce_counter > msg1.nonce_counter);
        assert!(msg2.lamport_clock > msg1.lamport_clock);
    }

    #[test]
    fn test_get_conversations() {
        let (service, _identity, _our_peer_id, peer_peer_id) = create_test_env();

        // Send a message to create a conversation
        service
            .send_message(&peer_peer_id, "Hello!", "text", None)
            .unwrap();

        let conversations = service.get_conversations().unwrap();
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].peer_id, peer_peer_id);
    }

    #[test]
    fn test_get_conversations_empty() {
        let (service, _identity, _our_peer_id, _peer_peer_id) = create_test_env();

        let conversations = service.get_conversations().unwrap();
        assert!(conversations.is_empty());
    }

    #[test]
    fn test_update_message_status() {
        let (service, _identity, _our_peer_id, peer_peer_id) = create_test_env();

        let msg = service
            .send_message(&peer_peer_id, "Hello!", "text", None)
            .unwrap();

        let updated = service
            .update_message_status(&msg.message_id, MessageStatus::Sent)
            .unwrap();
        assert!(updated);
    }

    #[test]
    fn test_update_message_status_nonexistent() {
        let (service, _identity, _our_peer_id, _peer_peer_id) = create_test_env();

        let updated = service
            .update_message_status("nonexistent-msg", MessageStatus::Sent)
            .unwrap();
        assert!(!updated);
    }

    #[test]
    fn test_clear_conversation_history() {
        let (service, _identity, _our_peer_id, peer_peer_id) = create_test_env();

        // Send some messages
        service
            .send_message(&peer_peer_id, "Msg 1", "text", None)
            .unwrap();
        service
            .send_message(&peer_peer_id, "Msg 2", "text", None)
            .unwrap();

        let cleared = service.clear_conversation_history(&peer_peer_id).unwrap();
        assert_eq!(cleared, 2);
    }

    #[test]
    fn test_clear_empty_conversation() {
        let (service, _identity, _our_peer_id, peer_peer_id) = create_test_env();

        let cleared = service.clear_conversation_history(&peer_peer_id).unwrap();
        assert_eq!(cleared, 0);
    }

    #[test]
    fn test_delete_conversation() {
        let (service, _identity, _our_peer_id, peer_peer_id) = create_test_env();

        service
            .send_message(&peer_peer_id, "Msg 1", "text", None)
            .unwrap();
        service
            .send_message(&peer_peer_id, "Msg 2", "text", None)
            .unwrap();

        let deleted = service.delete_conversation(&peer_peer_id).unwrap();
        assert_eq!(deleted, 2);

        // Conversations should be empty now
        let conversations = service.get_conversations().unwrap();
        assert!(conversations.is_empty());
    }

    #[test]
    fn test_send_message_locked_identity_fails() {
        let (service, identity_service, _our_peer_id, peer_peer_id) = create_test_env();

        identity_service.lock();

        let result = service.send_message(&peer_peer_id, "Hello!", "text", None);
        assert!(result.is_err());
    }

    #[test]
    fn stored_message_content_uses_typed_plaintext_and_security_states() {
        let sender = create_node(Arc::new(Database::in_memory().unwrap()), "Sender");
        let receiver = create_node(Arc::new(Database::in_memory().unwrap()), "Receiver");
        connect_for_chat(&sender, &receiver);

        let tampered = sender
            .messaging
            .send_message(&receiver.info.peer_id, "authentic one", "text", None)
            .unwrap();
        let corrupt = sender
            .messaging
            .send_message(&receiver.info.peer_id, "authentic two", "text", None)
            .unwrap();
        let wrong_key = sender
            .messaging
            .send_message(&receiver.info.peer_id, "authentic three", "text", None)
            .unwrap();
        receive(&receiver.messaging, &tampered).unwrap();
        receive(&receiver.messaging, &corrupt).unwrap();
        receive(&receiver.messaging, &wrong_key).unwrap();

        let initial = receiver
            .messaging
            .get_conversation_messages(&sender.info.peer_id, 10, None)
            .unwrap();
        assert_eq!(initial.len(), 3);
        assert!(initial.iter().any(|message| {
            message.message_id == tampered.message_id
                && message.content_state
                    == MessageContentState::Plaintext {
                        text: "authentic one".to_string(),
                    }
        }));

        let mut changed_ciphertext = tampered.content_encrypted.clone();
        changed_ciphertext[0] ^= 0x80;
        let (_, replacement_x25519) = CryptoService::generate_x25519_keypair();
        receiver
            .messaging
            .db()
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE messages SET content_encrypted = ? WHERE message_id = ?",
                    rusqlite::params![changed_ciphertext, tampered.message_id],
                )?;
                conn.execute(
                    "UPDATE messages SET content_encrypted = ? WHERE message_id = ?",
                    rusqlite::params![vec![1_u8], corrupt.message_id],
                )?;
                conn.execute(
                    "UPDATE contacts SET x25519_public = ? WHERE peer_id = ?",
                    rusqlite::params![replacement_x25519.to_bytes().to_vec(), sender.info.peer_id],
                )?;
                Ok(())
            })
            .unwrap();

        let states = receiver
            .messaging
            .get_conversation_messages(&sender.info.peer_id, 10, None)
            .unwrap()
            .into_iter()
            .map(|message| (message.message_id, message.content_state))
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            states.get(&tampered.message_id),
            Some(&MessageContentState::Tampered)
        );
        assert_eq!(
            states.get(&corrupt.message_id),
            Some(&MessageContentState::CorruptPayload)
        );
        assert_eq!(
            states.get(&wrong_key.message_id),
            Some(&MessageContentState::WrongKey)
        );
    }

    #[test]
    fn encrypted_event_shape_reports_unsupported_and_corrupt_payloads() {
        assert_eq!(
            inspect_encrypted_event_shape(
                MESSAGE_CRYPTO_VERSION + 1,
                &[0; MESSAGE_NONCE_ID_LEN],
                &[0; AES_GCM_TAG_LEN],
            ),
            Err(MessageContentState::UnsupportedVersion {
                version: MESSAGE_CRYPTO_VERSION + 1,
            })
        );
        assert_eq!(
            inspect_encrypted_event_shape(
                MESSAGE_CRYPTO_VERSION,
                &[0; MESSAGE_NONCE_ID_LEN],
                &[0; AES_GCM_TAG_LEN - 1],
            ),
            Err(MessageContentState::CorruptPayload)
        );
        assert_eq!(
            inspect_encrypted_event_shape(MESSAGE_CRYPTO_VERSION, &[0; 1], &[0; AES_GCM_TAG_LEN]),
            Err(MessageContentState::CorruptPayload)
        );
    }

    #[test]
    fn invalid_inbound_packets_do_not_consume_replay_state() {
        let sender = create_node(Arc::new(Database::in_memory().unwrap()), "Sender");
        let receiver = create_node(Arc::new(Database::in_memory().unwrap()), "Receiver");
        connect_for_chat(&sender, &receiver);

        let valid_after_envelope_failures = sender
            .messaging
            .send_message(&receiver.info.peer_id, "envelope", "text", None)
            .unwrap();
        let mut invalid_version = valid_after_envelope_failures.clone();
        invalid_version.protocol_version = MESSAGE_CRYPTO_VERSION + 1;
        assert!(receive(&receiver.messaging, &invalid_version).is_err());
        let mut invalid_event_id = valid_after_envelope_failures.clone();
        invalid_event_id.event_id = Uuid::new_v4().to_string();
        assert!(receive(&receiver.messaging, &invalid_event_id).is_err());
        let mut invalid_recipient = valid_after_envelope_failures.clone();
        invalid_recipient.recipient_peer_id = sender.info.peer_id.clone();
        assert!(receive(&receiver.messaging, &invalid_recipient).is_err());
        receive(&receiver.messaging, &valid_after_envelope_failures).unwrap();

        let valid_after_binding_failure = sender
            .messaging
            .send_message(&receiver.info.peer_id, "binding", "text", None)
            .unwrap();
        let mut invalid_binding = valid_after_binding_failure.clone();
        invalid_binding.conversation_id = "wrong-conversation".to_string();
        resign(&sender, &mut invalid_binding);
        assert!(receive(&receiver.messaging, &invalid_binding).is_err());
        receive(&receiver.messaging, &valid_after_binding_failure).unwrap();

        let valid_after_signature_failure = sender
            .messaging
            .send_message(&receiver.info.peer_id, "signature", "text", None)
            .unwrap();
        let mut invalid_signature = valid_after_signature_failure.clone();
        invalid_signature.signature[0] ^= 0x80;
        assert!(receive(&receiver.messaging, &invalid_signature).is_err());
        receive(&receiver.messaging, &valid_after_signature_failure).unwrap();

        let valid_after_aead_failure = sender
            .messaging
            .send_message(&receiver.info.peer_id, "aead", "text", None)
            .unwrap();
        let mut invalid_ciphertext = valid_after_aead_failure.clone();
        invalid_ciphertext.content_encrypted[0] ^= 0x01;
        resign(&sender, &mut invalid_ciphertext);
        assert!(receive(&receiver.messaging, &invalid_ciphertext).is_err());
        receive(&receiver.messaging, &valid_after_aead_failure).unwrap();

        let valid_after_clock_failure = sender
            .messaging
            .send_message(&receiver.info.peer_id, "clock", "text", None)
            .unwrap();
        let mut invalid_clock = valid_after_clock_failure.clone();
        invalid_clock.lamport_clock = u64::MAX;
        resign(&sender, &mut invalid_clock);
        assert!(matches!(
            receive(&receiver.messaging, &invalid_clock),
            Err(AppError::Validation(_))
        ));
        receive(&receiver.messaging, &valid_after_clock_failure).unwrap();

        let (messages, nonces) = receiver
            .messaging
            .db()
            .with_connection(|conn| {
                Ok((
                    conn.query_row("SELECT COUNT(*) FROM messages", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                    conn.query_row("SELECT COUNT(*) FROM message_crypto_nonces", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                ))
            })
            .unwrap();
        assert_eq!(messages, 5);
        assert_eq!(nonces, 5);
    }

    #[test]
    fn inbound_chat_capability_is_required_before_replay_state_is_committed() {
        let sender = create_node(Arc::new(Database::in_memory().unwrap()), "Sender");
        let receiver = create_node(Arc::new(Database::in_memory().unwrap()), "Receiver");
        add_contact(&sender, &receiver);
        add_contact(&receiver, &sender);
        grant_chat(&sender, &receiver.info.peer_id);

        let message = sender
            .messaging
            .send_message(&receiver.info.peer_id, "permission", "text", None)
            .unwrap();
        assert!(matches!(
            receive(&receiver.messaging, &message),
            Err(AppError::PermissionDenied(_))
        ));

        grant_chat(&receiver, &sender.info.peer_id);
        receive(&receiver.messaging, &message).unwrap();
    }

    #[test]
    fn exact_inbound_duplicate_is_idempotent_after_database_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let receiver_path = directory.path().join("receiver.sqlite");
        let sender = create_node(Arc::new(Database::in_memory().unwrap()), "Sender");

        let message = {
            let receiver = create_node(
                Arc::new(Database::new(receiver_path.clone()).unwrap()),
                "Receiver",
            );
            connect_for_chat(&sender, &receiver);
            let message = sender
                .messaging
                .send_message(&receiver.info.peer_id, "once", "text", None)
                .unwrap();
            receive(&receiver.messaging, &message).unwrap();
            receive(&receiver.messaging, &message).unwrap();
            message
        };

        let reopened = reopen_node(&receiver_path);
        receive(&reopened.messaging, &message).unwrap();
        let (messages, events, nonces) = reopened
            .messaging
            .db()
            .with_connection(|conn| {
                Ok((
                    conn.query_row("SELECT COUNT(*) FROM messages", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                    conn.query_row(
                        "SELECT COUNT(*) FROM message_events WHERE event_id = ?",
                        [&message.event_id],
                        |row| row.get::<_, i64>(0),
                    )?,
                    conn.query_row(
                        "SELECT COUNT(*) FROM message_crypto_nonces WHERE event_id = ?",
                        [&message.event_id],
                        |row| row.get::<_, i64>(0),
                    )?,
                ))
            })
            .unwrap();
        assert_eq!((messages, events, nonces), (1, 1, 1));
    }

    #[test]
    fn test_conversation_id_is_deterministic() {
        // Conversation IDs should be the same regardless of direction
        let id1 = derive_conversation_id("peer-a", "peer-b");
        let id2 = derive_conversation_id("peer-b", "peer-a");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_conversation_id_different_peers() {
        let id1 = derive_conversation_id("peer-a", "peer-b");
        let id2 = derive_conversation_id("peer-a", "peer-c");
        assert_ne!(id1, id2);
    }
}
