use serde::{Deserialize, Serialize};

/// A direct message between two peers
///
/// # Nonce Counter & Replay Protection
///
/// The `nonce_counter` field is critical for AES-256-GCM encryption security.
/// It must be:
/// - Unique per message within a conversation
/// - Monotonically increasing for the sender
///
/// ## Sender Rules:
/// 1. Get next counter via `Database::next_send_counter(conversation_id)`
/// 2. Use counter for AES-GCM nonce generation
/// 3. Include counter in this message (signed)
///
/// ## Receiver Rules:
/// 1. BEFORE decrypting, call `Database::check_and_record_nonce()`
/// 2. If returns `false` (replay detected), reject the entire message
/// 3. If returns `true`, proceed with decryption
/// 4. The nonce is permanently recorded to prevent future replay
///
/// This prevents attackers from re-sending captured messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectMessage {
    /// Unique message ID (UUID v4)
    pub message_id: String,
    /// Conversation ID (derived from sorted peer IDs)
    pub conversation_id: String,
    /// Sender's peer ID
    pub sender_peer_id: String,
    /// Recipient's peer ID
    pub recipient_peer_id: String,
    /// Encrypted message content (AES-256-GCM with counter-based nonce)
    pub content_encrypted: Vec<u8>,
    /// Content type (text, image, etc.)
    pub content_type: String,
    /// ID of message being replied to (optional)
    pub reply_to: Option<String>,
    /// Counter used for AES-GCM nonce generation (for replay protection)
    /// Must be unique per sender per conversation
    pub nonce_counter: u64,
    /// Lamport timestamp for ordering
    pub lamport_clock: u64,
    /// Unix timestamp when message was created
    pub timestamp: i64,
    /// Signature over all fields above (excluding signature itself)
    pub signature: Vec<u8>,
}

/// Acknowledgment of message delivery/read
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAck {
    /// ID of the message being acknowledged
    pub message_id: String,
    /// Conversation ID
    pub conversation_id: String,
    /// Peer ID of the one sending the ack
    pub peer_id: String,
    /// Status: delivered or read
    pub status: AckStatus,
    /// Unix timestamp
    pub timestamp: i64,
    /// Signature over all fields above
    pub signature: Vec<u8>,
}

/// Message acknowledgment status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AckStatus {
    Delivered,
    Read,
}

/// Request/response wrapper for messaging protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessagingMessage {
    /// A direct message
    Message(DirectMessage),
    /// An acknowledgment
    Ack(MessageAck),
    /// An edit to a previously sent message
    EditMessage {
        /// The ID of the message being edited
        message_id: String,
        /// The new plaintext content (will be encrypted by the receiver's local store)
        new_content: String,
        /// Timestamp of the edit
        edited_at: i64,
    },
}

/// Codec for messaging protocol
#[derive(Debug, Clone, Default)]
pub struct MessagingCodec;

impl MessagingCodec {
    /// Encode a messaging message to CBOR bytes
    pub fn encode(msg: &MessagingMessage) -> Result<Vec<u8>, ciborium::ser::Error<std::io::Error>> {
        let mut bytes = Vec::new();
        ciborium::into_writer(msg, &mut bytes)?;
        Ok(bytes)
    }

    /// Decode a messaging message from CBOR bytes
    pub fn decode(bytes: &[u8]) -> Result<MessagingMessage, ciborium::de::Error<std::io::Error>> {
        ciborium::from_reader(bytes)
    }
}

/// Helper to derive conversation ID from two peer IDs
pub fn derive_conversation_id(peer_a: &str, peer_b: &str) -> String {
    use sha2::{Digest, Sha256};

    // Sort peer IDs to ensure consistent conversation ID regardless of direction
    let (first, second) = if peer_a < peer_b {
        (peer_a, peer_b)
    } else {
        (peer_b, peer_a)
    };

    let mut hasher = Sha256::new();
    hasher.update(first.as_bytes());
    hasher.update(b":");
    hasher.update(second.as_bytes());
    let result = hasher.finalize();

    hex::encode(&result[..16]) // First 16 bytes = 32 hex chars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_message_roundtrip() {
        let msg = DirectMessage {
            message_id: "msg-123".to_string(),
            conversation_id: "conv-456".to_string(),
            sender_peer_id: "peer-a".to_string(),
            recipient_peer_id: "peer-b".to_string(),
            content_encrypted: vec![1, 2, 3, 4],
            content_type: "text".to_string(),
            reply_to: None,
            nonce_counter: 1,
            lamport_clock: 1,
            timestamp: 1234567890,
            signature: vec![5, 6, 7, 8],
        };

        let wrapped = MessagingMessage::Message(msg.clone());
        let encoded = MessagingCodec::encode(&wrapped).unwrap();
        let decoded = MessagingCodec::decode(&encoded).unwrap();

        if let MessagingMessage::Message(decoded_msg) = decoded {
            assert_eq!(decoded_msg.message_id, msg.message_id);
            assert_eq!(decoded_msg.content_encrypted, msg.content_encrypted);
        } else {
            panic!("Expected Message variant");
        }
    }

    #[test]
    fn test_message_ack_roundtrip() {
        let ack = MessageAck {
            message_id: "msg-123".to_string(),
            conversation_id: "conv-456".to_string(),
            peer_id: "peer-b".to_string(),
            status: AckStatus::Delivered,
            timestamp: 1234567890,
            signature: vec![1, 2, 3],
        };

        let wrapped = MessagingMessage::Ack(ack.clone());
        let encoded = MessagingCodec::encode(&wrapped).unwrap();
        let decoded = MessagingCodec::decode(&encoded).unwrap();

        if let MessagingMessage::Ack(decoded_ack) = decoded {
            assert_eq!(decoded_ack.message_id, ack.message_id);
            assert_eq!(decoded_ack.status, AckStatus::Delivered);
        } else {
            panic!("Expected Ack variant");
        }
    }

    #[test]
    fn test_conversation_id_deterministic() {
        let id1 = derive_conversation_id("peer-a", "peer-b");
        let id2 = derive_conversation_id("peer-b", "peer-a");

        assert_eq!(
            id1, id2,
            "Conversation ID should be the same regardless of order"
        );
    }
}
