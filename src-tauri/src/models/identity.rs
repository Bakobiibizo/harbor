use serde::{Deserialize, Serialize};

/// Local identity stored in the database
#[derive(Debug, Clone)]
pub struct LocalIdentity {
    pub peer_id: String,
    pub public_key: Vec<u8>,
    pub x25519_public: Vec<u8>,
    pub private_key_encrypted: Vec<u8>,
    pub display_name: String,
    pub avatar_hash: Option<String>,
    pub bio: Option<String>,
    pub passphrase_hint: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Identity info sent to frontend (no private keys)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInfo {
    pub peer_id: String,
    pub public_key: String,    // base64 encoded
    pub x25519_public: String, // base64 encoded
    pub display_name: String,
    pub avatar_hash: Option<String>,
    pub bio: Option<String>,
    pub passphrase_hint: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<LocalIdentity> for IdentityInfo {
    fn from(identity: LocalIdentity) -> Self {
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;

        Self {
            peer_id: identity.peer_id,
            public_key: engine.encode(&identity.public_key),
            x25519_public: engine.encode(&identity.x25519_public),
            display_name: identity.display_name,
            avatar_hash: identity.avatar_hash,
            bio: identity.bio,
            passphrase_hint: identity.passphrase_hint,
            created_at: identity.created_at,
            updated_at: identity.updated_at,
        }
    }
}

/// Request to create a new identity
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateIdentityRequest {
    pub display_name: String,
    pub passphrase: String,
    pub bio: Option<String>,
    pub passphrase_hint: Option<String>,
}

/// Request to unlock identity
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockIdentityRequest {
    pub passphrase: String,
}

/// Encrypted keys stored in database
/// Contains both Ed25519 (signing) and X25519 (key agreement) private keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedKeys {
    pub ed25519_private: Vec<u8>, // 32 bytes
    pub x25519_private: Vec<u8>,  // 32 bytes
}
