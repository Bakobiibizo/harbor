//! Messaging service for sending and receiving direct messages

use std::sync::Arc;
use uuid::Uuid;
use ed25519_dalek::VerifyingKey;
use x25519_dalek::PublicKey as X25519Public;

use crate::db::{
    Database, Capability, MessagesRepository, MessageData, MessageStatus, Conversation,
};
use crate::error::{AppError, Result};
use crate::p2p::protocols::messaging::derive_conversation_id;
use crate::services::{
    ContactsService, CryptoService, IdentityService, PermissionsService,
    verify, Signable, SignableDirectMessage, SignableMessageAck,
};

/// Service for managing direct messages
pub struct MessagingService {
    db: Arc<Database>,
    identity_service: Arc<IdentityService>,
    contacts_service: Arc<ContactsService>,
    permissions_service: Arc<PermissionsService>,
}

/// A decrypted message for the UI
#[derive(Debug, Clone)]
pub struct DecryptedMessage {
    pub message_id: String,
    pub conversation_id: String,
    pub sender_peer_id: String,
    pub recipient_peer_id: String,
    pub content: String,
    pub content_type: String,
    pub reply_to_message_id: Option<String>,
    pub sent_at: i64,
    pub delivered_at: Option<i64>,
    pub read_at: Option<i64>,
    pub status: String,
    pub is_outgoing: bool,
}

/// A message ready to be sent over the network
#[derive(Debug, Clone)]
pub struct OutgoingMessage {
    pub message_id: String,
    pub conversation_id: String,
    pub sender_peer_id: String,
    pub recipient_peer_id: String,
    pub content_encrypted: Vec<u8>,
    pub content_type: String,
    pub reply_to: Option<String>,
    pub nonce_counter: u64,
    pub lamport_clock: u64,
    pub timestamp: i64,
    pub signature: Vec<u8>,
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

    /// Send a new message to a peer
    pub fn send_message(
        &self,
        recipient_peer_id: &str,
        content: &str,
        content_type: &str,
        reply_to: Option<&str>,
    ) -> Result<OutgoingMessage> {
        // Get our identity
        let identity = self.identity_service.get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        // Check we have chat permission with this peer
        if !self.permissions_service.peer_has_capability(recipient_peer_id, Capability::Chat)? {
            return Err(AppError::PermissionDenied(
                "No chat permission with this peer".to_string()
            ));
        }

        // Get recipient's X25519 public key for encryption
        let x25519_public = self.contacts_service.get_x25519_public(recipient_peer_id)?
            .ok_or_else(|| AppError::NotFound("Contact not found".to_string()))?;

        // Get our X25519 keys
        let our_keys = self.identity_service.get_unlocked_keys()?;

        // Derive conversation ID and encryption key
        let conversation_id = derive_conversation_id(&identity.peer_id, recipient_peer_id);
        let their_public = X25519Public::from(
            <[u8; 32]>::try_from(x25519_public.as_slice())
                .map_err(|_| AppError::Crypto("Invalid X25519 key".to_string()))?
        );
        let shared_secret = CryptoService::x25519_dh(&our_keys.x25519_secret, &their_public);
        let conv_key = CryptoService::derive_conversation_key(
            &shared_secret,
            &conversation_id,
            &identity.peer_id,
            recipient_peer_id,
        );

        // Get next nonce counter
        let nonce_counter = self.db.next_send_counter(&conversation_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Encrypt content
        let content_encrypted = CryptoService::encrypt_message_with_counter(
            &conv_key,
            content.as_bytes(),
            nonce_counter,
        )?;

        // Create message
        let message_id = Uuid::new_v4().to_string();
        let lamport_clock = self.db.next_lamport_clock(&identity.peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))? as u64;
        let timestamp = chrono::Utc::now().timestamp();

        // Create signable and sign
        let signable = SignableDirectMessage {
            message_id: message_id.clone(),
            conversation_id: conversation_id.clone(),
            sender_peer_id: identity.peer_id.clone(),
            recipient_peer_id: recipient_peer_id.to_string(),
            content_encrypted: content_encrypted.clone(),
            content_type: content_type.to_string(),
            reply_to: reply_to.map(String::from),
            nonce_counter,
            lamport_clock,
            timestamp,
        };

        let signature = self.identity_service.sign(&signable)?;

        // Store locally
        let msg_data = MessageData {
            message_id: message_id.clone(),
            conversation_id: conversation_id.clone(),
            sender_peer_id: identity.peer_id.clone(),
            recipient_peer_id: recipient_peer_id.to_string(),
            content_encrypted: content_encrypted.clone(),
            content_type: content_type.to_string(),
            reply_to_message_id: reply_to.map(String::from),
            nonce_counter,
            lamport_clock: lamport_clock as i64,
            sent_at: timestamp,
            received_at: None,
            status: MessageStatus::Pending,
        };

        MessagesRepository::insert_message(&self.db, &msg_data)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Record event
        let event_id = format!("sent:{}", message_id);
        let payload_cbor = signable.signable_bytes()?;
        MessagesRepository::record_message_event(
            &self.db,
            &event_id,
            "sent",
            &message_id,
            &conversation_id,
            &identity.peer_id,
            recipient_peer_id,
            lamport_clock as i64,
            timestamp,
            &payload_cbor,
            &signature,
        ).map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(OutgoingMessage {
            message_id,
            conversation_id,
            sender_peer_id: identity.peer_id,
            recipient_peer_id: recipient_peer_id.to_string(),
            content_encrypted,
            content_type: content_type.to_string(),
            reply_to: reply_to.map(String::from),
            nonce_counter,
            lamport_clock,
            timestamp,
            signature,
        })
    }

    /// Process an incoming message from the network
    pub fn process_incoming_message(
        &self,
        message_id: &str,
        conversation_id: &str,
        sender_peer_id: &str,
        recipient_peer_id: &str,
        content_encrypted: &[u8],
        content_type: &str,
        reply_to: Option<&str>,
        nonce_counter: u64,
        lamport_clock: u64,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<()> {
        // Verify we are the recipient
        let identity = self.identity_service.get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        if recipient_peer_id != identity.peer_id {
            return Err(AppError::Validation("Message not for us".to_string()));
        }

        // Check for replay (BEFORE decryption)
        if !self.db.check_and_record_nonce(conversation_id, sender_peer_id, nonce_counter)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
        {
            return Err(AppError::Crypto("Replay attack detected".to_string()));
        }

        // Get sender's public key for verification
        let sender_public_key = self.contacts_service.get_public_key(sender_peer_id)?
            .ok_or_else(|| AppError::NotFound("Sender not in contacts".to_string()))?;

        // Verify signature
        let signable = SignableDirectMessage {
            message_id: message_id.to_string(),
            conversation_id: conversation_id.to_string(),
            sender_peer_id: sender_peer_id.to_string(),
            recipient_peer_id: recipient_peer_id.to_string(),
            content_encrypted: content_encrypted.to_vec(),
            content_type: content_type.to_string(),
            reply_to: reply_to.map(String::from),
            nonce_counter,
            lamport_clock,
            timestamp,
        };

        let verifying_key = VerifyingKey::from_bytes(
            sender_public_key.as_slice().try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?
        ).map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto("Invalid message signature".to_string()));
        }

        // Check for deduplication
        if MessagesRepository::message_exists(&self.db, message_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
        {
            return Ok(()); // Already processed
        }

        // Update lamport clock
        self.db.update_lamport_clock(sender_peer_id, lamport_clock as i64)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Store message
        let received_at = chrono::Utc::now().timestamp();
        let msg_data = MessageData {
            message_id: message_id.to_string(),
            conversation_id: conversation_id.to_string(),
            sender_peer_id: sender_peer_id.to_string(),
            recipient_peer_id: recipient_peer_id.to_string(),
            content_encrypted: content_encrypted.to_vec(),
            content_type: content_type.to_string(),
            reply_to_message_id: reply_to.map(String::from),
            nonce_counter,
            lamport_clock: lamport_clock as i64,
            sent_at: timestamp,
            received_at: Some(received_at),
            status: MessageStatus::Delivered,
        };

        MessagesRepository::insert_message(&self.db, &msg_data)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Record event
        let event_id = format!("received:{}", message_id);
        let payload_cbor = signable.signable_bytes()?;
        MessagesRepository::record_message_event(
            &self.db,
            &event_id,
            "received",
            message_id,
            conversation_id,
            sender_peer_id,
            recipient_peer_id,
            lamport_clock as i64,
            timestamp,
            &payload_cbor,
            signature,
        ).map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(())
    }

    /// Create a delivery acknowledgment
    pub fn create_delivery_ack(&self, message_id: &str) -> Result<(SignableMessageAck, Vec<u8>)> {
        let identity = self.identity_service.get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let message = MessagesRepository::get_by_message_id(&self.db, message_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Message not found".to_string()))?;

        let timestamp = chrono::Utc::now().timestamp();

        let signable = SignableMessageAck {
            message_id: message_id.to_string(),
            conversation_id: message.conversation_id.clone(),
            ack_sender_peer_id: identity.peer_id.clone(),
            status: "delivered".to_string(),
            timestamp,
        };

        let signature = self.identity_service.sign(&signable)?;

        Ok((signable, signature))
    }

    /// Create a read acknowledgment
    pub fn create_read_ack(&self, message_id: &str) -> Result<(SignableMessageAck, Vec<u8>)> {
        let identity = self.identity_service.get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let message = MessagesRepository::get_by_message_id(&self.db, message_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Message not found".to_string()))?;

        let timestamp = chrono::Utc::now().timestamp();

        let signable = SignableMessageAck {
            message_id: message_id.to_string(),
            conversation_id: message.conversation_id.clone(),
            ack_sender_peer_id: identity.peer_id.clone(),
            status: "read".to_string(),
            timestamp,
        };

        let signature = self.identity_service.sign(&signable)?;

        // Mark as read locally
        MessagesRepository::mark_read(&self.db, message_id, timestamp)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok((signable, signature))
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
        // Get the ack sender's public key
        let sender_public_key = self.contacts_service.get_public_key(ack_sender_peer_id)?
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
            sender_public_key.as_slice().try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?
        ).map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto("Invalid ack signature".to_string()));
        }

        // Update message status
        match status {
            "delivered" => {
                MessagesRepository::mark_delivered(&self.db, message_id, timestamp)
                    .map_err(|e| AppError::DatabaseString(e.to_string()))?;
            }
            "read" => {
                MessagesRepository::mark_read(&self.db, message_id, timestamp)
                    .map_err(|e| AppError::DatabaseString(e.to_string()))?;
            }
            _ => {
                return Err(AppError::Validation(format!("Invalid ack status: {}", status)));
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
        let identity = self.identity_service.get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let conversation_id = derive_conversation_id(&identity.peer_id, peer_id);

        // Get encrypted messages
        let messages = MessagesRepository::get_conversation_messages(
            &self.db,
            &conversation_id,
            limit,
            before_timestamp,
        ).map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Get peer's X25519 key for decryption
        let x25519_public = self.contacts_service.get_x25519_public(peer_id)?
            .ok_or_else(|| AppError::NotFound("Contact not found".to_string()))?;

        let our_keys = self.identity_service.get_unlocked_keys()?;

        // Derive conversation key
        let their_public = X25519Public::from(
            <[u8; 32]>::try_from(x25519_public.as_slice())
                .map_err(|_| AppError::Crypto("Invalid X25519 key".to_string()))?
        );
        let shared_secret = CryptoService::x25519_dh(&our_keys.x25519_secret, &their_public);
        let conv_key = CryptoService::derive_conversation_key(
            &shared_secret,
            &conversation_id,
            &identity.peer_id,
            peer_id,
        );

        // Decrypt messages
        let mut decrypted = Vec::new();
        for msg in messages {
            let content = match CryptoService::decrypt_message_with_counter(
                &conv_key,
                &msg.content_encrypted,
                msg.nonce_counter,
            ) {
                Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                Err(_) => "[Decryption failed]".to_string(),
            };

            decrypted.push(DecryptedMessage {
                message_id: msg.message_id,
                conversation_id: msg.conversation_id,
                sender_peer_id: msg.sender_peer_id.clone(),
                recipient_peer_id: msg.recipient_peer_id,
                content,
                content_type: msg.content_type,
                reply_to_message_id: msg.reply_to_message_id,
                sent_at: msg.sent_at,
                delivered_at: msg.delivered_at,
                read_at: msg.read_at,
                status: msg.status,
                is_outgoing: msg.sender_peer_id == identity.peer_id,
            });
        }

        Ok(decrypted)
    }

    /// Get all conversations
    pub fn get_conversations(&self) -> Result<Vec<Conversation>> {
        let identity = self.identity_service.get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        MessagesRepository::get_conversations(&self.db, &identity.peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Mark a conversation as read
    pub fn mark_conversation_read(&self, peer_id: &str) -> Result<i64> {
        let identity = self.identity_service.get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let conversation_id = derive_conversation_id(&identity.peer_id, peer_id);
        let timestamp = chrono::Utc::now().timestamp();

        MessagesRepository::mark_conversation_read(
            &self.db,
            &conversation_id,
            &identity.peer_id,
            timestamp,
        ).map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get unread count for a conversation
    pub fn get_unread_count(&self, peer_id: &str) -> Result<i64> {
        let identity = self.identity_service.get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let conversation_id = derive_conversation_id(&identity.peer_id, peer_id);

        MessagesRepository::get_unread_count(&self.db, &conversation_id, &identity.peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Update message status (for network events)
    pub fn update_message_status(&self, message_id: &str, status: MessageStatus) -> Result<bool> {
        MessagesRepository::update_status(&self.db, message_id, status)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }
}
