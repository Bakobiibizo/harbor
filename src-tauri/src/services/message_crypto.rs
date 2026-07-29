//! Version 2 direct-message cryptographic primitives.
//!
//! The v1 message construction used one conversation key and a sender-local counter as the
//! AES-GCM nonce. Two peers could therefore reuse the same key/nonce pair, and restoring an old
//! counter could repeat it again. V2 separates traffic keys by direction and derives a fresh AEAD
//! key and nonce for each immutable message event. The replay counter is authenticated metadata,
//! but it is deliberately not the source of nonce uniqueness.

use std::fmt;

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng, Payload},
    Aes256Gcm, Nonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{AppError, Result};

pub const MESSAGE_CRYPTO_VERSION: u16 = 2;
pub const MESSAGE_NONCE_ID_LEN: usize = 16;

const KDF_SALT_DOMAIN: &[u8] = b"harbor/direct-message/v2/kdf-salt";
const DIRECTION_KEY_DOMAIN: &[u8] = b"harbor/direct-message/v2/direction-key";
const EVENT_CONTEXT_DOMAIN: &[u8] = b"harbor/direct-message/v2/event-context";
const EVENT_KEY_DOMAIN: &[u8] = b"harbor/direct-message/v2/event-key";
const EVENT_NONCE_DOMAIN: &[u8] = b"harbor/direct-message/v2/event-nonce";
const EVENT_AAD_DOMAIN: &[u8] = b"harbor/direct-message/v2/aad";

/// The immutable operation encrypted by a message envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageEventKind {
    Create,
    Edit,
}

impl MessageEventKind {
    fn domain_byte(self) -> u8 {
        match self {
            Self::Create => 0,
            Self::Edit => 1,
        }
    }
}

/// Public metadata cryptographically bound to one encrypted message event.
///
/// A create event uses `revision == 0` and `event_id == message_id`. An edit uses a positive
/// revision and a distinct, freshly generated `event_id`. `nonce_counter` is durable replay/order
/// metadata. It may roll back without repeating AEAD material because `event_id` and `nonce_id`
/// are also part of the derivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageEventContext<'a> {
    pub protocol_version: u16,
    pub conversation_id: &'a str,
    pub sender_peer_id: &'a str,
    pub recipient_peer_id: &'a str,
    pub message_id: &'a str,
    pub event_id: &'a str,
    pub kind: MessageEventKind,
    pub revision: u64,
    pub nonce_counter: u64,
}

impl MessageEventContext<'_> {
    pub fn validate(&self) -> Result<()> {
        if self.protocol_version != MESSAGE_CRYPTO_VERSION {
            return Err(AppError::Validation(format!(
                "Unsupported message crypto version {}",
                self.protocol_version
            )));
        }

        for (label, value) in [
            ("conversation_id", self.conversation_id),
            ("sender_peer_id", self.sender_peer_id),
            ("recipient_peer_id", self.recipient_peer_id),
            ("message_id", self.message_id),
            ("event_id", self.event_id),
        ] {
            if value.is_empty() {
                return Err(AppError::Validation(format!(
                    "Message v2 {label} must not be empty"
                )));
            }
        }

        if self.sender_peer_id == self.recipient_peer_id {
            return Err(AppError::Validation(
                "Message v2 sender and recipient must differ".to_string(),
            ));
        }

        match self.kind {
            MessageEventKind::Create if self.revision != 0 || self.event_id != self.message_id => {
                Err(AppError::Validation(
                    "Message v2 create events require revision 0 and event_id == message_id"
                        .to_string(),
                ))
            }
            MessageEventKind::Edit if self.revision == 0 || self.event_id == self.message_id => {
                Err(AppError::Validation(
                    "Message v2 edit events require a positive revision and a fresh event_id"
                        .to_string(),
                ))
            }
            _ => Ok(()),
        }
    }
}

/// A fresh public identifier carried by each encrypted event.
///
/// This is not secret. It is combined with the directional traffic key and the complete event
/// context to derive event-specific AEAD material.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageNonceId([u8; MESSAGE_NONCE_ID_LEN]);

impl MessageNonceId {
    pub fn generate() -> Self {
        let mut bytes = [0u8; MESSAGE_NONCE_ID_LEN];
        OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    pub const fn from_bytes(bytes: [u8; MESSAGE_NONCE_ID_LEN]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; MESSAGE_NONCE_ID_LEN] {
        &self.0
    }

    pub fn try_from_slice(bytes: &[u8]) -> Result<Self> {
        let bytes = <[u8; MESSAGE_NONCE_ID_LEN]>::try_from(bytes).map_err(|_| {
            AppError::Validation(format!(
                "Message v2 nonce_id must be exactly {MESSAGE_NONCE_ID_LEN} bytes"
            ))
        })?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for MessageNonceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("MessageNonceId")
            .field(&hex::encode(self.0))
            .finish()
    }
}

/// A direction-specific conversation traffic key.
///
/// The key does not implement `Debug` with its bytes, reducing accidental log disclosure.
#[derive(Clone, PartialEq, Eq)]
pub struct DirectionalMessageKey([u8; 32]);

impl fmt::Debug for DirectionalMessageKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DirectionalMessageKey([REDACTED])")
    }
}

impl DirectionalMessageKey {
    #[cfg(test)]
    fn test_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Ciphertext and the public nonce identifier needed to decrypt it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedMessageEvent {
    pub nonce_id: MessageNonceId,
    pub ciphertext: Vec<u8>,
}

/// Derive one traffic key for exactly `sender_peer_id -> recipient_peer_id`.
///
/// Reversing sender and recipient yields an independent key even though both peers hold the same
/// X25519 shared secret. Canonical participant ordering is included in the salt so conversation
/// context cannot be substituted across a different pair.
pub fn derive_directional_message_key(
    shared_secret: &[u8; 32],
    conversation_id: &str,
    sender_peer_id: &str,
    recipient_peer_id: &str,
) -> Result<DirectionalMessageKey> {
    if shared_secret.iter().all(|byte| *byte == 0) {
        return Err(AppError::Crypto(
            "Message v2 rejected an invalid all-zero X25519 shared secret".to_string(),
        ));
    }
    if conversation_id.is_empty() || sender_peer_id.is_empty() || recipient_peer_id.is_empty() {
        return Err(AppError::Validation(
            "Message v2 key context must not be empty".to_string(),
        ));
    }
    if sender_peer_id == recipient_peer_id {
        return Err(AppError::Validation(
            "Message v2 sender and recipient must differ".to_string(),
        ));
    }

    let (first, second) = if sender_peer_id < recipient_peer_id {
        (sender_peer_id, recipient_peer_id)
    } else {
        (recipient_peer_id, sender_peer_id)
    };

    let salt = hash_parts(
        KDF_SALT_DOMAIN,
        [
            conversation_id.as_bytes(),
            first.as_bytes(),
            second.as_bytes(),
        ],
    );
    let info = encode_parts(
        DIRECTION_KEY_DOMAIN,
        [sender_peer_id.as_bytes(), recipient_peer_id.as_bytes()],
    );
    let hkdf = Hkdf::<Sha256>::new(Some(&salt), shared_secret);
    let mut key = [0u8; 32];
    hkdf.expand(&info, &mut key)
        .map_err(|_| AppError::Crypto("Failed to derive message v2 directional key".to_string()))?;
    Ok(DirectionalMessageKey(key))
}

/// Encrypt a new immutable event using a freshly generated nonce identifier.
pub fn encrypt_message_event(
    directional_key: &DirectionalMessageKey,
    context: &MessageEventContext<'_>,
    plaintext: &[u8],
) -> Result<EncryptedMessageEvent> {
    encrypt_message_event_with_nonce(
        directional_key,
        context,
        MessageNonceId::generate(),
        plaintext,
    )
}

/// Encrypt using a caller-supplied nonce identifier.
///
/// Production callers should use [`encrypt_message_event`]. This entry point exists for durable
/// retry (where the exact envelope is reconstructed) and deterministic protocol vectors. A nonce
/// identifier must never be assigned to two different events under the same directional key.
pub fn encrypt_message_event_with_nonce(
    directional_key: &DirectionalMessageKey,
    context: &MessageEventContext<'_>,
    nonce_id: MessageNonceId,
    plaintext: &[u8],
) -> Result<EncryptedMessageEvent> {
    context.validate()?;
    let material = derive_event_material(directional_key, context, &nonce_id);
    let cipher = Aes256Gcm::new_from_slice(&material.key)
        .map_err(|e| AppError::CryptoEncryption(format!("Failed to create cipher: {e}")))?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&material.nonce),
            Payload {
                msg: plaintext,
                aad: &material.aad,
            },
        )
        .map_err(|e| AppError::CryptoEncryption(format!("Message v2 encryption failed: {e}")))?;

    Ok(EncryptedMessageEvent {
        nonce_id,
        ciphertext,
    })
}

/// Authenticate and decrypt one immutable event.
pub fn decrypt_message_event(
    directional_key: &DirectionalMessageKey,
    context: &MessageEventContext<'_>,
    encrypted: &EncryptedMessageEvent,
) -> Result<Vec<u8>> {
    context.validate()?;
    let material = derive_event_material(directional_key, context, &encrypted.nonce_id);
    let cipher = Aes256Gcm::new_from_slice(&material.key)
        .map_err(|e| AppError::CryptoDecryption(format!("Failed to create cipher: {e}")))?;
    cipher
        .decrypt(
            Nonce::from_slice(&material.nonce),
            Payload {
                msg: &encrypted.ciphertext,
                aad: &material.aad,
            },
        )
        .map_err(|_| {
            AppError::CryptoDecryption("Message v2 authentication or decryption failed".to_string())
        })
}

struct EventMaterial {
    key: [u8; 32],
    nonce: [u8; 12],
    aad: Vec<u8>,
}

fn derive_event_material(
    directional_key: &DirectionalMessageKey,
    context: &MessageEventContext<'_>,
    nonce_id: &MessageNonceId,
) -> EventMaterial {
    let context_bytes = event_context_bytes(context);
    let context_digest: [u8; 32] = Sha256::digest(&context_bytes).into();

    let hkdf = Hkdf::<Sha256>::new(Some(nonce_id.as_bytes()), &directional_key.0);
    let key_info = encode_parts(EVENT_KEY_DOMAIN, [context_digest.as_slice()]);
    let mut key = [0u8; 32];
    hkdf.expand(&key_info, &mut key)
        .expect("fixed-size message v2 HKDF output is valid");

    let nonce_digest = hash_parts(
        EVENT_NONCE_DOMAIN,
        [nonce_id.as_bytes().as_slice(), context_digest.as_slice()],
    );
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&nonce_digest[..12]);

    let aad = encode_parts(
        EVENT_AAD_DOMAIN,
        [nonce_id.as_bytes().as_slice(), context_bytes.as_slice()],
    );
    EventMaterial { key, nonce, aad }
}

fn event_context_bytes(context: &MessageEventContext<'_>) -> Vec<u8> {
    let version = context.protocol_version.to_be_bytes();
    let kind = [context.kind.domain_byte()];
    let revision = context.revision.to_be_bytes();
    let counter = context.nonce_counter.to_be_bytes();
    encode_parts(
        EVENT_CONTEXT_DOMAIN,
        [
            version.as_slice(),
            context.conversation_id.as_bytes(),
            context.sender_peer_id.as_bytes(),
            context.recipient_peer_id.as_bytes(),
            context.message_id.as_bytes(),
            context.event_id.as_bytes(),
            kind.as_slice(),
            revision.as_slice(),
            counter.as_slice(),
        ],
    )
}

fn hash_parts<'a>(domain: &[u8], parts: impl IntoIterator<Item = &'a [u8]>) -> [u8; 32] {
    Sha256::digest(encode_parts(domain, parts)).into()
}

fn encode_parts<'a>(domain: &[u8], parts: impl IntoIterator<Item = &'a [u8]>) -> Vec<u8> {
    let mut encoded = Vec::new();
    push_part(&mut encoded, domain);
    for part in parts {
        push_part(&mut encoded, part);
    }
    encoded
}

fn push_part(encoded: &mut Vec<u8>, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("message context field length fits u64");
    encoded.extend_from_slice(&length.to_be_bytes());
    encoded.extend_from_slice(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHARED_SECRET: [u8; 32] = [0x42; 32];
    const ALICE: &str = "12D3KooWAlice";
    const BOB: &str = "12D3KooWBob";
    const CONVERSATION: &str = "c4fe7f978e9b95f583a71ef7b7f80ea1";

    fn create_context<'a>(sender: &'a str, recipient: &'a str) -> MessageEventContext<'a> {
        MessageEventContext {
            protocol_version: MESSAGE_CRYPTO_VERSION,
            conversation_id: CONVERSATION,
            sender_peer_id: sender,
            recipient_peer_id: recipient,
            message_id: "message-0001",
            event_id: "message-0001",
            kind: MessageEventKind::Create,
            revision: 0,
            nonce_counter: 1,
        }
    }

    #[test]
    fn simultaneous_first_messages_use_independent_directional_keys_and_material() {
        let alice_to_bob =
            derive_directional_message_key(&SHARED_SECRET, CONVERSATION, ALICE, BOB).unwrap();
        let bob_to_alice =
            derive_directional_message_key(&SHARED_SECRET, CONVERSATION, BOB, ALICE).unwrap();
        assert_ne!(alice_to_bob, bob_to_alice);

        let nonce_id = MessageNonceId::from_bytes([0x11; MESSAGE_NONCE_ID_LEN]);
        let ab_material =
            derive_event_material(&alice_to_bob, &create_context(ALICE, BOB), &nonce_id);
        let ba_material =
            derive_event_material(&bob_to_alice, &create_context(BOB, ALICE), &nonce_id);
        assert_ne!(ab_material.key, ba_material.key);
        assert_ne!(ab_material.nonce, ba_material.nonce);
    }

    #[test]
    fn restart_and_counter_rollback_do_not_repeat_event_material() {
        let key = derive_directional_message_key(&SHARED_SECRET, CONVERSATION, ALICE, BOB).unwrap();
        let first = create_context(ALICE, BOB);
        let restarted = MessageEventContext {
            message_id: "message-after-restart",
            event_id: "message-after-restart",
            nonce_counter: 1,
            ..first
        };
        let first_material = derive_event_material(
            &key,
            &first,
            &MessageNonceId::from_bytes([0x21; MESSAGE_NONCE_ID_LEN]),
        );
        let restarted_material = derive_event_material(
            &key,
            &restarted,
            &MessageNonceId::from_bytes([0x21; MESSAGE_NONCE_ID_LEN]),
        );
        assert_ne!(first_material.key, restarted_material.key);
        assert_ne!(first_material.nonce, restarted_material.nonce);

        // Even an exact rolled-back public context gets fresh material from a fresh nonce ID.
        let rollback_material = derive_event_material(
            &key,
            &first,
            &MessageNonceId::from_bytes([0x22; MESSAGE_NONCE_ID_LEN]),
        );
        assert_ne!(first_material.key, rollback_material.key);
        assert_ne!(first_material.nonce, rollback_material.nonce);
    }

    #[test]
    fn every_edit_revision_has_fresh_event_material() {
        let key = derive_directional_message_key(&SHARED_SECRET, CONVERSATION, ALICE, BOB).unwrap();
        let edit_one = MessageEventContext {
            protocol_version: MESSAGE_CRYPTO_VERSION,
            conversation_id: CONVERSATION,
            sender_peer_id: ALICE,
            recipient_peer_id: BOB,
            message_id: "message-0001",
            event_id: "edit-event-0001",
            kind: MessageEventKind::Edit,
            revision: 1,
            nonce_counter: 2,
        };
        let edit_two = MessageEventContext {
            event_id: "edit-event-0002",
            revision: 2,
            nonce_counter: 3,
            ..edit_one
        };
        let one = derive_event_material(
            &key,
            &edit_one,
            &MessageNonceId::from_bytes([0x31; MESSAGE_NONCE_ID_LEN]),
        );
        let two = derive_event_material(
            &key,
            &edit_two,
            &MessageNonceId::from_bytes([0x31; MESSAGE_NONCE_ID_LEN]),
        );
        assert_ne!(one.key, two.key);
        assert_ne!(one.nonce, two.nonce);

        let retried_as_fresh_event = derive_event_material(
            &key,
            &edit_one,
            &MessageNonceId::from_bytes([0x32; MESSAGE_NONCE_ID_LEN]),
        );
        assert_ne!(one.key, retried_as_fresh_event.key);
        assert_ne!(one.nonce, retried_as_fresh_event.nonce);
    }

    #[test]
    fn roundtrip_and_every_bound_field_is_authenticated() {
        let key = derive_directional_message_key(&SHARED_SECRET, CONVERSATION, ALICE, BOB).unwrap();
        let context = create_context(ALICE, BOB);
        let encrypted = encrypt_message_event_with_nonce(
            &key,
            &context,
            MessageNonceId::from_bytes([0x41; MESSAGE_NONCE_ID_LEN]),
            b"hello harbor",
        )
        .unwrap();
        assert!(!encrypted
            .ciphertext
            .windows(b"hello harbor".len())
            .any(|window| window == b"hello harbor"));
        assert_eq!(
            decrypt_message_event(&key, &context, &encrypted).unwrap(),
            b"hello harbor"
        );

        let mutations = [
            MessageEventContext {
                conversation_id: "other-conversation",
                ..context
            },
            MessageEventContext {
                sender_peer_id: BOB,
                recipient_peer_id: ALICE,
                ..context
            },
            MessageEventContext {
                message_id: "other-message",
                event_id: "other-message",
                ..context
            },
            MessageEventContext {
                nonce_counter: 2,
                ..context
            },
        ];
        for changed in mutations {
            assert!(decrypt_message_event(&key, &changed, &encrypted).is_err());
        }

        let mut tampered = encrypted.clone();
        tampered.ciphertext[0] ^= 0x80;
        assert!(decrypt_message_event(&key, &context, &tampered).is_err());

        let mut wrong_nonce = encrypted.clone();
        wrong_nonce.nonce_id = MessageNonceId::from_bytes([0x42; MESSAGE_NONCE_ID_LEN]);
        assert!(decrypt_message_event(&key, &context, &wrong_nonce).is_err());

        let reverse_key =
            derive_directional_message_key(&SHARED_SECRET, CONVERSATION, BOB, ALICE).unwrap();
        assert!(decrypt_message_event(&reverse_key, &context, &encrypted).is_err());
    }

    #[test]
    fn rejects_invalid_create_and_edit_shapes() {
        let create = create_context(ALICE, BOB);
        assert!(MessageEventContext {
            protocol_version: 1,
            ..create
        }
        .validate()
        .is_err());
        assert!(MessageEventContext {
            revision: 1,
            ..create
        }
        .validate()
        .is_err());
        assert!(MessageEventContext {
            event_id: "different-event",
            ..create
        }
        .validate()
        .is_err());

        let edit = MessageEventContext {
            event_id: "edit-1",
            kind: MessageEventKind::Edit,
            revision: 1,
            ..create
        };
        assert!(edit.validate().is_ok());
        assert!(MessageEventContext {
            revision: 0,
            ..edit
        }
        .validate()
        .is_err());
        assert!(MessageEventContext {
            event_id: edit.message_id,
            ..edit
        }
        .validate()
        .is_err());
    }

    #[test]
    fn nonce_identifier_requires_exact_wire_length() {
        assert!(MessageNonceId::try_from_slice(&[0x44; MESSAGE_NONCE_ID_LEN]).is_ok());
        assert!(MessageNonceId::try_from_slice(&[0x44; MESSAGE_NONCE_ID_LEN - 1]).is_err());
        assert!(MessageNonceId::try_from_slice(&[0x44; MESSAGE_NONCE_ID_LEN + 1]).is_err());
    }

    #[test]
    fn rejects_invalid_key_agreement_and_identity_context() {
        assert!(derive_directional_message_key(&[0; 32], CONVERSATION, ALICE, BOB).is_err());
        assert!(derive_directional_message_key(&SHARED_SECRET, "", ALICE, BOB).is_err());
        assert!(
            derive_directional_message_key(&SHARED_SECRET, CONVERSATION, ALICE, ALICE).is_err()
        );
    }

    #[test]
    fn golden_vector_locks_v2_key_nonce_aad_and_ciphertext() {
        let key = derive_directional_message_key(&SHARED_SECRET, CONVERSATION, ALICE, BOB).unwrap();
        let context = create_context(ALICE, BOB);
        let nonce_id = MessageNonceId::from_bytes([
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ]);
        let material = derive_event_material(&key, &context, &nonce_id);
        let encrypted =
            encrypt_message_event_with_nonce(&key, &context, nonce_id, b"Harbor v2 vector")
                .unwrap();

        // Independently reproduced with Node's HKDF-SHA256 and AES-256-GCM implementations.
        assert_eq!(
            hex::encode(key.test_bytes()),
            "608dcb76b46b101ad86a2586925dcd78fe974cfe9927fee85651d9ba74d385e3"
        );
        assert_eq!(
            hex::encode(material.key),
            "633b9a19c56ee6f50a17f1e1fee333f1acb019c420f3727132cc2e11c411b743"
        );
        assert_eq!(hex::encode(material.nonce), "96a2e76429571f56de4d8c93");
        assert_eq!(
            hex::encode(Sha256::digest(&material.aad)),
            "22c669c027e0813d3d590efe7fa8384d698f42d33ff7885a4cd66fa4aa1cf4d0"
        );
        assert_eq!(
            hex::encode(encrypted.ciphertext),
            "cbca770931ab0df7a8ae07415989815be847e087d161f1c38e65c566cb4e133c"
        );
    }
}
