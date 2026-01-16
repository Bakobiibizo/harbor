use crate::error::{AppError, Result};
use crate::models::EncryptedKeys;

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use rand::RngCore;
use sha2::{Sha256, Digest};
use x25519_dalek::{StaticSecret as X25519Secret, PublicKey as X25519Public};

/// Cryptographic operations service
pub struct CryptoService;

impl CryptoService {
    /// Generate a new Ed25519 keypair for signing
    pub fn generate_ed25519_keypair() -> (SigningKey, VerifyingKey) {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    /// Generate a new X25519 keypair for key agreement
    pub fn generate_x25519_keypair() -> (X25519Secret, X25519Public) {
        let secret = X25519Secret::random_from_rng(OsRng);
        let public = X25519Public::from(&secret);
        (secret, public)
    }

    /// Derive a peer ID from an Ed25519 public key
    /// Uses a simple hash-based approach compatible with libp2p
    pub fn derive_peer_id(public_key: &VerifyingKey) -> String {
        let mut hasher = Sha256::new();
        hasher.update(public_key.as_bytes());
        let hash = hasher.finalize();

        // Format as base58-like string with "12D3KooW" prefix (simplified)
        // In production, this should match libp2p's PeerId derivation
        format!("12D3KooW{}", hex::encode(&hash[..16]))
    }

    /// Encrypt private keys using a passphrase
    pub fn encrypt_keys(
        ed25519_private: &[u8],
        x25519_private: &[u8],
        passphrase: &str,
    ) -> Result<Vec<u8>> {
        // Derive encryption key from passphrase using Argon2id
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(passphrase.as_bytes(), &salt)
            .map_err(|e| AppError::Crypto(format!("Failed to hash passphrase: {}", e)))?;

        let hash_bytes = password_hash.hash.ok_or_else(|| {
            AppError::Crypto("Failed to get hash bytes".to_string())
        })?;

        // Use first 32 bytes of hash as AES key
        let key_bytes: [u8; 32] = hash_bytes.as_bytes()[..32]
            .try_into()
            .map_err(|_| AppError::Crypto("Invalid key length".to_string()))?;

        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| AppError::Crypto(format!("Failed to create cipher: {}", e)))?;

        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Combine keys for encryption
        let keys = EncryptedKeys {
            ed25519_private: ed25519_private.to_vec(),
            x25519_private: x25519_private.to_vec(),
        };
        let plaintext = serde_json::to_vec(&keys)
            .map_err(|e| AppError::Serialization(format!("Failed to serialize keys: {}", e)))?;

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|e| AppError::Crypto(format!("Encryption failed: {}", e)))?;

        // Combine: salt (22 bytes as string) + nonce (12 bytes) + ciphertext
        let salt_bytes = salt.as_str().as_bytes();
        let mut result = Vec::with_capacity(salt_bytes.len() + 1 + 12 + ciphertext.len());
        result.push(salt_bytes.len() as u8);
        result.extend_from_slice(salt_bytes);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt private keys using a passphrase
    pub fn decrypt_keys(encrypted: &[u8], passphrase: &str) -> Result<EncryptedKeys> {
        if encrypted.is_empty() {
            return Err(AppError::Crypto("Empty encrypted data".to_string()));
        }

        // Parse: salt_len (1 byte) + salt + nonce (12 bytes) + ciphertext
        let salt_len = encrypted[0] as usize;
        if encrypted.len() < 1 + salt_len + 12 {
            return Err(AppError::Crypto("Invalid encrypted data format".to_string()));
        }

        let salt_str = std::str::from_utf8(&encrypted[1..1 + salt_len])
            .map_err(|e| AppError::Crypto(format!("Invalid salt: {}", e)))?;

        let salt = SaltString::from_b64(salt_str)
            .map_err(|e| AppError::Crypto(format!("Invalid salt format: {}", e)))?;

        let nonce_start = 1 + salt_len;
        let nonce_bytes: [u8; 12] = encrypted[nonce_start..nonce_start + 12]
            .try_into()
            .map_err(|_| AppError::Crypto("Invalid nonce length".to_string()))?;

        let ciphertext = &encrypted[nonce_start + 12..];

        // Derive key from passphrase
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(passphrase.as_bytes(), &salt)
            .map_err(|e| AppError::Crypto(format!("Failed to hash passphrase: {}", e)))?;

        let hash_bytes = password_hash.hash.ok_or_else(|| {
            AppError::Crypto("Failed to get hash bytes".to_string())
        })?;

        let key_bytes: [u8; 32] = hash_bytes.as_bytes()[..32]
            .try_into()
            .map_err(|_| AppError::Crypto("Invalid key length".to_string()))?;

        // Decrypt
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| AppError::Crypto(format!("Failed to create cipher: {}", e)))?;

        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| AppError::Crypto("Decryption failed - wrong passphrase?".to_string()))?;

        let keys: EncryptedKeys = serde_json::from_slice(&plaintext)
            .map_err(|e| AppError::Serialization(format!("Failed to deserialize keys: {}", e)))?;

        Ok(keys)
    }

    /// Sign data using Ed25519
    pub fn sign(signing_key: &SigningKey, data: &[u8]) -> Signature {
        signing_key.sign(data)
    }

    /// Verify an Ed25519 signature
    pub fn verify(verifying_key: &VerifyingKey, data: &[u8], signature: &Signature) -> bool {
        verifying_key.verify(data, signature).is_ok()
    }

    /// Perform X25519 Diffie-Hellman key exchange
    pub fn x25519_dh(our_secret: &X25519Secret, their_public: &X25519Public) -> [u8; 32] {
        our_secret.diffie_hellman(their_public).to_bytes()
    }

    /// Derive a symmetric key from shared secret using HKDF
    ///
    /// DEPRECATED: Use `derive_conversation_key` instead for conversation encryption.
    /// This function is kept for backwards compatibility only.
    pub fn derive_symmetric_key(shared_secret: &[u8], context: &[u8]) -> [u8; 32] {
        use hkdf::Hkdf;

        let hk = Hkdf::<Sha256>::new(Some(context), shared_secret);
        let mut key = [0u8; 32];
        hk.expand(b"harbor-v1", &mut key).expect("HKDF expand failed");
        key
    }

    /// Derive a conversation encryption key from X25519 shared secret
    ///
    /// The salt includes:
    /// - Protocol version prefix for domain separation
    /// - Conversation ID (deterministic from peer IDs)
    /// - Both peer IDs in sorted order (for consistency regardless of who initiates)
    ///
    /// This hardening prevents accidental cross-context key reuse.
    pub fn derive_conversation_key(
        shared_secret: &[u8; 32],
        conversation_id: &str,
        peer_a: &str,
        peer_b: &str,
    ) -> [u8; 32] {
        use hkdf::Hkdf;

        // Sort peer IDs for consistent salt regardless of direction
        let (first, second) = if peer_a < peer_b {
            (peer_a, peer_b)
        } else {
            (peer_b, peer_a)
        };

        // Build salt with full context
        let salt = format!(
            "harbor:v1:conv:{}:{}:{}",
            conversation_id, first, second
        );

        let hk = Hkdf::<Sha256>::new(Some(salt.as_bytes()), shared_secret);
        let mut key = [0u8; 32];
        hk.expand(b"conversation-key", &mut key).expect("HKDF expand failed");
        key
    }

    /// Encrypt a message using AES-256-GCM
    pub fn encrypt_message(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| AppError::Crypto(format!("Failed to create cipher: {}", e)))?;

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| AppError::Crypto(format!("Encryption failed: {}", e)))?;

        // Combine nonce + ciphertext
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt a message using AES-256-GCM
    pub fn decrypt_message(key: &[u8; 32], encrypted: &[u8]) -> Result<Vec<u8>> {
        if encrypted.len() < 12 {
            return Err(AppError::Crypto("Invalid encrypted message".to_string()));
        }

        let nonce = Nonce::from_slice(&encrypted[..12]);
        let ciphertext = &encrypted[12..];

        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| AppError::Crypto(format!("Failed to create cipher: {}", e)))?;

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| AppError::Crypto("Decryption failed".to_string()))?;

        Ok(plaintext)
    }

    /// Hash data using SHA-256
    pub fn sha256(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    // ============================================================
    // Counter-based Nonce Functions (for conversation encryption)
    // ============================================================

    /// Generate a deterministic nonce from a send counter
    ///
    /// The nonce is 12 bytes (96 bits) for AES-GCM:
    /// - First 4 bytes: 0x00 (reserved for future use/direction flag)
    /// - Next 8 bytes: counter as big-endian u64
    ///
    /// This ensures unique nonces as long as:
    /// 1. Counter is never reused for the same conversation
    /// 2. Counter increases monotonically
    pub fn nonce_from_counter(counter: u64) -> [u8; 12] {
        let mut nonce = [0u8; 12];
        // First 4 bytes are zero (can use for direction flag later)
        // Last 8 bytes are the counter
        nonce[4..12].copy_from_slice(&counter.to_be_bytes());
        nonce
    }

    /// Encrypt a message using AES-256-GCM with a counter-based nonce
    ///
    /// IMPORTANT: The counter MUST be unique for each message in a conversation.
    /// Use `Database::next_send_counter()` to get the next counter value.
    pub fn encrypt_message_with_counter(
        key: &[u8; 32],
        plaintext: &[u8],
        counter: u64,
    ) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| AppError::Crypto(format!("Failed to create cipher: {}", e)))?;

        let nonce_bytes = Self::nonce_from_counter(counter);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| AppError::Crypto(format!("Encryption failed: {}", e)))?;

        // Return only ciphertext - counter is sent separately in message header
        Ok(ciphertext)
    }

    /// Decrypt a message using AES-256-GCM with a counter-based nonce
    ///
    /// IMPORTANT: Before calling this, verify the counter hasn't been used before
    /// using `Database::check_and_record_nonce()`.
    pub fn decrypt_message_with_counter(
        key: &[u8; 32],
        ciphertext: &[u8],
        counter: u64,
    ) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| AppError::Crypto(format!("Failed to create cipher: {}", e)))?;

        let nonce_bytes = Self::nonce_from_counter(counter);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| AppError::Crypto("Decryption failed".to_string()))?;

        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ed25519_keypair_generation() {
        let (signing_key, verifying_key) = CryptoService::generate_ed25519_keypair();

        // Sign and verify
        let message = b"Hello, World!";
        let signature = CryptoService::sign(&signing_key, message);
        assert!(CryptoService::verify(&verifying_key, message, &signature));

        // Wrong message should fail
        assert!(!CryptoService::verify(&verifying_key, b"Wrong message", &signature));
    }

    #[test]
    fn test_x25519_key_exchange() {
        let (alice_secret, alice_public) = CryptoService::generate_x25519_keypair();
        let (bob_secret, bob_public) = CryptoService::generate_x25519_keypair();

        let alice_shared = CryptoService::x25519_dh(&alice_secret, &bob_public);
        let bob_shared = CryptoService::x25519_dh(&bob_secret, &alice_public);

        assert_eq!(alice_shared, bob_shared);
    }

    #[test]
    fn test_key_encryption_decryption() {
        let ed25519_private = [1u8; 32];
        let x25519_private = [2u8; 32];
        let passphrase = "test-passphrase-123";

        let encrypted = CryptoService::encrypt_keys(
            &ed25519_private,
            &x25519_private,
            passphrase,
        ).unwrap();

        let decrypted = CryptoService::decrypt_keys(&encrypted, passphrase).unwrap();

        assert_eq!(decrypted.ed25519_private, ed25519_private);
        assert_eq!(decrypted.x25519_private, x25519_private);
    }

    #[test]
    fn test_key_decryption_wrong_passphrase() {
        let ed25519_private = [1u8; 32];
        let x25519_private = [2u8; 32];

        let encrypted = CryptoService::encrypt_keys(
            &ed25519_private,
            &x25519_private,
            "correct-passphrase",
        ).unwrap();

        let result = CryptoService::decrypt_keys(&encrypted, "wrong-passphrase");
        assert!(result.is_err());
    }

    #[test]
    fn test_message_encryption() {
        let key = [0u8; 32];
        let message = b"Secret message";

        let encrypted = CryptoService::encrypt_message(&key, message).unwrap();
        let decrypted = CryptoService::decrypt_message(&key, &encrypted).unwrap();

        assert_eq!(decrypted, message);
    }

    #[test]
    fn test_peer_id_derivation() {
        let (_, verifying_key) = CryptoService::generate_ed25519_keypair();
        let peer_id = CryptoService::derive_peer_id(&verifying_key);

        assert!(peer_id.starts_with("12D3KooW"));
        assert_eq!(peer_id.len(), 8 + 32); // prefix + hex
    }

    #[test]
    fn test_nonce_from_counter() {
        let nonce1 = CryptoService::nonce_from_counter(1);
        let nonce2 = CryptoService::nonce_from_counter(2);
        let nonce_max = CryptoService::nonce_from_counter(u64::MAX);

        // Nonces should be different
        assert_ne!(nonce1, nonce2);

        // First 4 bytes should be zero
        assert_eq!(&nonce1[0..4], &[0, 0, 0, 0]);
        assert_eq!(&nonce_max[0..4], &[0, 0, 0, 0]);

        // Counter should be in last 8 bytes as big-endian
        assert_eq!(&nonce1[4..12], &1u64.to_be_bytes());
        assert_eq!(&nonce2[4..12], &2u64.to_be_bytes());
    }

    #[test]
    fn test_counter_based_encryption() {
        let key = [42u8; 32];
        let message = b"Secret message with counter";

        // Encrypt with counter 1
        let ciphertext = CryptoService::encrypt_message_with_counter(&key, message, 1).unwrap();

        // Decrypt with same counter
        let decrypted = CryptoService::decrypt_message_with_counter(&key, &ciphertext, 1).unwrap();
        assert_eq!(decrypted, message);

        // Decrypt with wrong counter should fail
        let result = CryptoService::decrypt_message_with_counter(&key, &ciphertext, 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_same_message_different_counters() {
        let key = [42u8; 32];
        let message = b"Same message";

        let ciphertext1 = CryptoService::encrypt_message_with_counter(&key, message, 1).unwrap();
        let ciphertext2 = CryptoService::encrypt_message_with_counter(&key, message, 2).unwrap();

        // Same plaintext with different counters produces different ciphertext
        assert_ne!(ciphertext1, ciphertext2);

        // Both decrypt correctly with their respective counters
        let decrypted1 = CryptoService::decrypt_message_with_counter(&key, &ciphertext1, 1).unwrap();
        let decrypted2 = CryptoService::decrypt_message_with_counter(&key, &ciphertext2, 2).unwrap();
        assert_eq!(decrypted1, message);
        assert_eq!(decrypted2, message);
    }

    #[test]
    fn test_derive_conversation_key_deterministic() {
        let shared_secret = [0x42u8; 32];
        let conv_id = "conv-123";
        let peer_a = "12D3KooWAlice";
        let peer_b = "12D3KooWBob";

        let key1 = CryptoService::derive_conversation_key(&shared_secret, conv_id, peer_a, peer_b);
        let key2 = CryptoService::derive_conversation_key(&shared_secret, conv_id, peer_a, peer_b);

        assert_eq!(key1, key2, "Same inputs should produce same key");
    }

    #[test]
    fn test_derive_conversation_key_order_independent() {
        let shared_secret = [0x42u8; 32];
        let conv_id = "conv-123";
        let peer_a = "12D3KooWAlice";
        let peer_b = "12D3KooWBob";

        // Order of peer IDs shouldn't matter
        let key_ab = CryptoService::derive_conversation_key(&shared_secret, conv_id, peer_a, peer_b);
        let key_ba = CryptoService::derive_conversation_key(&shared_secret, conv_id, peer_b, peer_a);

        assert_eq!(key_ab, key_ba, "Peer order should not affect key derivation");
    }

    #[test]
    fn test_derive_conversation_key_different_conversations() {
        let shared_secret = [0x42u8; 32];
        let peer_a = "12D3KooWAlice";
        let peer_b = "12D3KooWBob";

        let key1 = CryptoService::derive_conversation_key(&shared_secret, "conv-1", peer_a, peer_b);
        let key2 = CryptoService::derive_conversation_key(&shared_secret, "conv-2", peer_a, peer_b);

        assert_ne!(key1, key2, "Different conversations should have different keys");
    }

    #[test]
    fn test_derive_conversation_key_different_peers() {
        let shared_secret = [0x42u8; 32];
        let conv_id = "conv-123";

        let key1 = CryptoService::derive_conversation_key(
            &shared_secret, conv_id, "12D3KooWAlice", "12D3KooWBob"
        );
        let key2 = CryptoService::derive_conversation_key(
            &shared_secret, conv_id, "12D3KooWAlice", "12D3KooWCharlie"
        );

        assert_ne!(key1, key2, "Different peer combinations should have different keys");
    }

    #[test]
    fn test_derive_conversation_key_different_secrets() {
        let secret1 = [0x42u8; 32];
        let secret2 = [0x43u8; 32];
        let conv_id = "conv-123";
        let peer_a = "12D3KooWAlice";
        let peer_b = "12D3KooWBob";

        let key1 = CryptoService::derive_conversation_key(&secret1, conv_id, peer_a, peer_b);
        let key2 = CryptoService::derive_conversation_key(&secret2, conv_id, peer_a, peer_b);

        assert_ne!(key1, key2, "Different shared secrets should produce different keys");
    }
}
