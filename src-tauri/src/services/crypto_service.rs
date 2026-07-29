use crate::error::{AppError, Result};
use crate::models::EncryptedKeys;

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::RngCore;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey as X25519Public, StaticSecret as X25519Secret};

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

    /// Derive a peer ID from an Ed25519 signing key
    /// Uses libp2p's actual PeerId derivation for compatibility with the network layer
    pub fn derive_peer_id_from_signing_key(signing_key: &SigningKey) -> Result<String> {
        // Convert our ed25519_dalek SigningKey to libp2p's format
        let secret =
            libp2p::identity::ed25519::SecretKey::try_from_bytes(signing_key.to_bytes().to_vec())
                .map_err(|e| {
                AppError::Crypto(format!("Failed to convert Ed25519 signing key: {}", e))
            })?;
        let libp2p_keypair = libp2p::identity::ed25519::Keypair::from(secret);
        let libp2p_keypair = libp2p::identity::Keypair::from(libp2p_keypair);
        let peer_id = libp2p::PeerId::from(libp2p_keypair.public());
        Ok(peer_id.to_string())
    }

    /// Derive a peer ID from an Ed25519 verifying (public) key
    ///
    /// This is used during identity verification to confirm that the public key
    /// included in an identity response actually derives the claimed peer ID.
    /// Uses libp2p's actual PeerId derivation for network compatibility.
    pub fn derive_peer_id_from_verifying_key(verifying_key: &VerifyingKey) -> Result<String> {
        let libp2p_public_key =
            libp2p::identity::ed25519::PublicKey::try_from_bytes(verifying_key.to_bytes().as_ref())
                .map_err(|e| {
                    AppError::Crypto(format!("Failed to convert Ed25519 public key: {}", e))
                })?;
        let libp2p_public = libp2p::identity::PublicKey::from(libp2p_public_key);
        let peer_id = libp2p::PeerId::from(libp2p_public);
        Ok(peer_id.to_string())
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
            .map_err(|e| AppError::Crypto(format!("Failed to hash password: {}", e)))?;

        let hash_bytes = password_hash
            .hash
            .ok_or_else(|| AppError::Crypto("Failed to get hash bytes".to_string()))?;

        // Use first 32 bytes of hash as AES key
        let key_bytes: [u8; 32] = hash_bytes.as_bytes()[..32]
            .try_into()
            .map_err(|_| AppError::Crypto("Invalid key length".to_string()))?;

        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| AppError::CryptoEncryption(format!("Failed to create cipher: {}", e)))?;

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
            .map_err(|e| AppError::CryptoEncryption(format!("Encryption failed: {}", e)))?;

        // Combine: salt (22 bytes as string) + nonce (12 bytes) + ciphertext
        let salt_bytes = salt.as_str().as_bytes();
        let mut result = Vec::with_capacity(salt_bytes.len() + 1 + 12 + ciphertext.len());
        result.push(salt_bytes.len() as u8);
        result.extend_from_slice(salt_bytes);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Validate the password-independent structure of an encrypted key blob.
    /// This lets startup distinguish a locked identity from a key envelope that
    /// cannot possibly be decrypted, without attempting or weakening password
    /// verification.
    pub fn validate_encrypted_key_envelope(encrypted: &[u8]) -> Result<()> {
        if encrypted.is_empty() {
            return Err(AppError::CryptoDecryption(
                "Empty encrypted key data".to_string(),
            ));
        }

        let salt_len = encrypted[0] as usize;
        let nonce_start = 1usize.checked_add(salt_len).ok_or_else(|| {
            AppError::CryptoDecryption("Invalid encrypted key salt length".to_string())
        })?;
        let ciphertext_start = nonce_start.checked_add(12).ok_or_else(|| {
            AppError::CryptoDecryption("Invalid encrypted key nonce length".to_string())
        })?;
        // AES-GCM always appends a 16-byte authentication tag. Anything shorter
        // is structurally corrupt regardless of the password supplied later.
        if encrypted.len() < ciphertext_start + 16 {
            return Err(AppError::CryptoDecryption(
                "Invalid encrypted key data format".to_string(),
            ));
        }

        let salt_str = std::str::from_utf8(&encrypted[1..nonce_start])
            .map_err(|e| AppError::CryptoDecryption(format!("Invalid key salt: {e}")))?;
        SaltString::from_b64(salt_str)
            .map_err(|e| AppError::CryptoDecryption(format!("Invalid key salt format: {e}")))?;
        Ok(())
    }

    /// Decrypt private keys using a passphrase
    pub fn decrypt_keys(encrypted: &[u8], passphrase: &str) -> Result<EncryptedKeys> {
        if encrypted.is_empty() {
            return Err(AppError::CryptoDecryption(
                "Empty encrypted data".to_string(),
            ));
        }

        // Parse: salt_len (1 byte) + salt + nonce (12 bytes) + ciphertext
        let salt_len = encrypted[0] as usize;
        if encrypted.len() < 1 + salt_len + 12 {
            return Err(AppError::CryptoDecryption(
                "Invalid encrypted data format".to_string(),
            ));
        }

        let salt_str = std::str::from_utf8(&encrypted[1..1 + salt_len])
            .map_err(|e| AppError::CryptoDecryption(format!("Invalid salt: {}", e)))?;

        let salt = SaltString::from_b64(salt_str)
            .map_err(|e| AppError::CryptoDecryption(format!("Invalid salt format: {}", e)))?;

        let nonce_start = 1 + salt_len;
        let nonce_bytes: [u8; 12] = encrypted[nonce_start..nonce_start + 12]
            .try_into()
            .map_err(|_| AppError::CryptoDecryption("Invalid nonce length".to_string()))?;

        let ciphertext = &encrypted[nonce_start + 12..];

        // Derive key from passphrase
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(passphrase.as_bytes(), &salt)
            .map_err(|e| AppError::CryptoDecryption(format!("Failed to hash password: {}", e)))?;

        let hash_bytes = password_hash
            .hash
            .ok_or_else(|| AppError::CryptoDecryption("Failed to get hash bytes".to_string()))?;

        let key_bytes: [u8; 32] = hash_bytes.as_bytes()[..32]
            .try_into()
            .map_err(|_| AppError::CryptoDecryption("Invalid key length".to_string()))?;

        // Decrypt
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| AppError::CryptoDecryption(format!("Failed to create cipher: {}", e)))?;

        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|_| {
            AppError::IdentityInvalidPassphrase("Decryption failed - invalid password".to_string())
        })?;

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

    /// Derive a versioned key for an ephemeral, domain-specific envelope.
    ///
    /// This is intentionally separate from direct-message traffic keys. Callers must provide the
    /// exact application protocol domain and version so a shared X25519 secret cannot silently be
    /// reused across incompatible envelope types.
    pub fn derive_ephemeral_envelope_key(
        shared_secret: &[u8; 32],
        protocol_domain: &str,
        protocol_version: u16,
    ) -> Result<[u8; 32]> {
        use hkdf::Hkdf;

        if shared_secret.iter().all(|byte| *byte == 0) {
            return Err(AppError::Crypto(
                "Rejected an invalid all-zero X25519 shared secret".to_string(),
            ));
        }
        if protocol_domain.is_empty() {
            return Err(AppError::Validation(
                "Ephemeral envelope protocol domain must not be empty".to_string(),
            ));
        }

        let mut salt_hasher = Sha256::new();
        salt_hasher.update(b"harbor/ephemeral-envelope/kdf-salt");
        salt_hasher.update(protocol_version.to_be_bytes());
        salt_hasher.update((protocol_domain.len() as u64).to_be_bytes());
        salt_hasher.update(protocol_domain.as_bytes());
        let salt = salt_hasher.finalize();
        let hk = Hkdf::<Sha256>::new(Some(&salt), shared_secret);
        let mut key = [0u8; 32];
        hk.expand(b"harbor/ephemeral-envelope/key", &mut key)
            .map_err(|_| AppError::Crypto("Failed to derive ephemeral envelope key".to_string()))?;
        Ok(key)
    }

    /// Encrypt a message using AES-256-GCM
    pub fn encrypt_message(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| AppError::CryptoEncryption(format!("Failed to create cipher: {}", e)))?;

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| AppError::CryptoEncryption(format!("Encryption failed: {}", e)))?;

        // Combine nonce + ciphertext
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt a message using AES-256-GCM
    pub fn decrypt_message(key: &[u8; 32], encrypted: &[u8]) -> Result<Vec<u8>> {
        if encrypted.len() < 12 {
            return Err(AppError::CryptoDecryption(
                "Invalid encrypted message".to_string(),
            ));
        }

        let nonce = Nonce::from_slice(&encrypted[..12]);
        let ciphertext = &encrypted[12..];

        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| AppError::CryptoDecryption(format!("Failed to create cipher: {}", e)))?;

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| AppError::CryptoDecryption("Decryption failed".to_string()))?;

        Ok(plaintext)
    }

    /// Hash data using SHA-256
    pub fn sha256(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
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
        assert!(!CryptoService::verify(
            &verifying_key,
            b"Wrong message",
            &signature
        ));
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
    fn ephemeral_envelope_keys_are_versioned_and_domain_separated() {
        let shared = [0x42; 32];
        let mention_v2 =
            CryptoService::derive_ephemeral_envelope_key(&shared, "harbor/mention", 2).unwrap();
        assert_eq!(
            mention_v2,
            CryptoService::derive_ephemeral_envelope_key(&shared, "harbor/mention", 2).unwrap()
        );
        assert_ne!(
            mention_v2,
            CryptoService::derive_ephemeral_envelope_key(&shared, "harbor/mention", 3).unwrap()
        );
        assert_ne!(
            mention_v2,
            CryptoService::derive_ephemeral_envelope_key(&shared, "harbor/other", 2).unwrap()
        );
        assert!(
            CryptoService::derive_ephemeral_envelope_key(&[0; 32], "harbor/mention", 2).is_err()
        );
        assert!(CryptoService::derive_ephemeral_envelope_key(&shared, "", 2).is_err());
    }

    #[test]
    fn test_key_encryption_decryption() {
        let ed25519_private = [1u8; 32];
        let x25519_private = [2u8; 32];
        let passphrase = "test-passphrase-123";

        let encrypted =
            CryptoService::encrypt_keys(&ed25519_private, &x25519_private, passphrase).unwrap();

        let decrypted = CryptoService::decrypt_keys(&encrypted, passphrase).unwrap();

        assert_eq!(decrypted.ed25519_private, ed25519_private);
        assert_eq!(decrypted.x25519_private, x25519_private);
    }

    #[test]
    fn test_key_decryption_wrong_passphrase() {
        let ed25519_private = [1u8; 32];
        let x25519_private = [2u8; 32];

        let encrypted =
            CryptoService::encrypt_keys(&ed25519_private, &x25519_private, "correct-passphrase")
                .unwrap();

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
    fn test_peer_id_derivation_libp2p() {
        let (signing_key, _) = CryptoService::generate_ed25519_keypair();
        let peer_id = CryptoService::derive_peer_id_from_signing_key(&signing_key).unwrap();

        // libp2p peer IDs start with "12D3KooW" and are longer (base58 encoded)
        assert!(peer_id.starts_with("12D3KooW"));
        // Full libp2p peer ID is typically 52 characters
        assert!(
            peer_id.len() >= 50,
            "Peer ID should be a full libp2p PeerId: {}",
            peer_id
        );
    }

    #[test]
    fn test_peer_id_matches_network_keypair() {
        // This test verifies that derive_peer_id_from_signing_key produces the same
        // peer ID as the network layer would when using ed25519_to_libp2p_keypair
        let (signing_key, _) = CryptoService::generate_ed25519_keypair();

        // Method 1: Our derive_peer_id_from_signing_key function
        let derived_peer_id = CryptoService::derive_peer_id_from_signing_key(&signing_key).unwrap();

        // Method 2: The same way the network layer derives it
        let bytes = signing_key.to_bytes();
        let secret = libp2p::identity::ed25519::SecretKey::try_from_bytes(bytes.to_vec())
            .expect("Valid Ed25519 key");
        let keypair = libp2p::identity::ed25519::Keypair::from(secret);
        let libp2p_keypair = libp2p::identity::Keypair::from(keypair);
        let network_peer_id = libp2p::PeerId::from(libp2p_keypair.public()).to_string();

        assert_eq!(
            derived_peer_id, network_peer_id,
            "Peer ID mismatch! Identity service: {} vs Network: {}",
            derived_peer_id, network_peer_id
        );
    }

    #[test]
    fn test_peer_id_from_verifying_key_matches_signing_key() {
        // Verify that derive_peer_id_from_verifying_key produces the same peer ID
        // as derive_peer_id_from_signing_key for the corresponding keypair.
        // This is critical for the identity exchange signature verification.
        let (signing_key, verifying_key) = CryptoService::generate_ed25519_keypair();

        let from_signing = CryptoService::derive_peer_id_from_signing_key(&signing_key).unwrap();
        let from_verifying =
            CryptoService::derive_peer_id_from_verifying_key(&verifying_key).unwrap();

        assert_eq!(
            from_signing, from_verifying,
            "Peer ID from signing key ({}) must match peer ID from verifying key ({})",
            from_signing, from_verifying
        );

        // Also verify it starts with the expected libp2p prefix
        assert!(from_verifying.starts_with("12D3KooW"));
        assert!(
            from_verifying.len() >= 50,
            "Peer ID should be a full libp2p PeerId: {}",
            from_verifying
        );
    }
}
