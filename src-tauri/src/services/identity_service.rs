use crate::db::repositories::IdentityRepository;
use crate::db::Database;
use crate::error::{AppError, Result};
use crate::models::{CreateIdentityRequest, IdentityInfo, LocalIdentity};
use crate::services::{sign as signing_sign, CryptoService, Signable};

use ed25519_dalek::SigningKey;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::{error, info};
use x25519_dalek::StaticSecret as X25519Secret;

/// Service for managing the local user's identity
pub struct IdentityService {
    db: Arc<Database>,
    /// Cached unlocked keys (only available after unlock)
    unlocked_keys: Arc<RwLock<Option<UnlockedKeys>>>,
}

/// Keys that are available after unlocking with passphrase
#[derive(Clone)]
pub struct UnlockedKeys {
    pub ed25519_signing: SigningKey,
    pub x25519_secret: X25519Secret,
}

impl IdentityService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            unlocked_keys: Arc::new(RwLock::new(None)),
        }
    }

    /// Acquire a read lock on the unlocked keys, recovering from poisoned state.
    fn read_keys(&self) -> RwLockReadGuard<'_, Option<UnlockedKeys>> {
        self.unlocked_keys.read().unwrap_or_else(|poisoned| {
            error!(
                "Identity keys RwLock was poisoned (a thread panicked while holding it). \
                 Recovering read access."
            );
            poisoned.into_inner()
        })
    }

    /// Acquire a write lock on the unlocked keys, recovering from poisoned state.
    fn write_keys(&self) -> RwLockWriteGuard<'_, Option<UnlockedKeys>> {
        self.unlocked_keys.write().unwrap_or_else(|poisoned| {
            error!(
                "Identity keys RwLock was poisoned (a thread panicked while holding it). \
                 Recovering write access."
            );
            poisoned.into_inner()
        })
    }

    /// Check if an identity has been created
    pub fn has_identity(&self) -> Result<bool> {
        let repo = IdentityRepository::new(&self.db);
        repo.exists().map_err(Into::into)
    }

    /// Check if the identity is currently unlocked
    pub fn is_unlocked(&self) -> bool {
        self.read_keys().is_some()
    }

    /// Get identity info (public data only)
    pub fn get_identity_info(&self) -> Result<Option<IdentityInfo>> {
        let repo = IdentityRepository::new(&self.db);
        match repo.get()? {
            Some(identity) => Ok(Some(identity.into())),
            None => Ok(None),
        }
    }

    /// Create a new identity with the given display name and passphrase
    pub fn create_identity(&self, request: CreateIdentityRequest) -> Result<IdentityInfo> {
        let repo = IdentityRepository::new(&self.db);

        // Check if identity already exists
        if repo.exists()? {
            return Err(AppError::AlreadyExists(
                "Identity already exists".to_string(),
            ));
        }

        // Generate Ed25519 keypair for signing
        let (ed25519_signing, ed25519_verifying) = CryptoService::generate_ed25519_keypair();

        // Generate X25519 keypair for key agreement
        let (x25519_secret, x25519_public) = CryptoService::generate_x25519_keypair();

        // Derive peer ID using libp2p's format for network compatibility
        let peer_id = CryptoService::derive_peer_id_from_signing_key(&ed25519_signing);
        info!(
            "Derived peer ID from signing key: {} (length: {})",
            peer_id,
            peer_id.len()
        );

        // Encrypt private keys
        let encrypted_keys = CryptoService::encrypt_keys(
            ed25519_signing.to_bytes().as_ref(),
            x25519_secret.as_bytes(),
            &request.passphrase,
        )?;

        let now = chrono::Utc::now().timestamp();

        let identity = LocalIdentity {
            peer_id: peer_id.clone(),
            public_key: ed25519_verifying.to_bytes().to_vec(),
            x25519_public: x25519_public.to_bytes().to_vec(),
            private_key_encrypted: encrypted_keys,
            display_name: request.display_name,
            avatar_hash: None,
            bio: request.bio,
            passphrase_hint: request.passphrase_hint,
            created_at: now,
            updated_at: now,
        };

        repo.create(&identity)?;

        // Auto-unlock after creation
        {
            let mut unlocked = self.write_keys();
            *unlocked = Some(UnlockedKeys {
                ed25519_signing,
                x25519_secret,
            });
        }

        info!("Created new identity: {}", peer_id);
        Ok(identity.into())
    }

    /// Unlock the identity with the passphrase
    pub fn unlock(&self, passphrase: &str) -> Result<IdentityInfo> {
        let repo = IdentityRepository::new(&self.db);

        let identity = repo
            .get()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;

        // Decrypt private keys
        let keys = CryptoService::decrypt_keys(&identity.private_key_encrypted, passphrase)?;

        // Reconstruct signing key
        let ed25519_bytes: [u8; 32] = keys
            .ed25519_private
            .try_into()
            .map_err(|_| AppError::Crypto("Invalid Ed25519 key length".to_string()))?;
        let ed25519_signing = SigningKey::from_bytes(&ed25519_bytes);

        // Reconstruct X25519 secret
        let x25519_bytes: [u8; 32] = keys
            .x25519_private
            .try_into()
            .map_err(|_| AppError::Crypto("Invalid X25519 key length".to_string()))?;
        let x25519_secret = X25519Secret::from(x25519_bytes);

        // Store unlocked keys
        {
            let mut unlocked = self.write_keys();
            *unlocked = Some(UnlockedKeys {
                ed25519_signing,
                x25519_secret,
            });
        }

        info!("Identity unlocked: {}", identity.peer_id);
        Ok(identity.into())
    }

    /// Lock the identity (clear unlocked keys from memory)
    pub fn lock(&self) {
        let mut unlocked = self.write_keys();
        *unlocked = None;
        info!("Identity locked");
    }

    /// Get the unlocked keys (for signing/encryption operations)
    pub fn get_unlocked_keys(&self) -> Result<UnlockedKeys> {
        let unlocked = self.read_keys();
        unlocked
            .clone()
            .ok_or_else(|| AppError::IdentityLocked("Identity is locked".to_string()))
    }

    /// Sign raw data using the unlocked Ed25519 key
    pub fn sign_raw(&self, data: &[u8]) -> Result<Vec<u8>> {
        let keys = self.get_unlocked_keys()?;
        let signature = CryptoService::sign(&keys.ed25519_signing, data);
        Ok(signature.to_bytes().to_vec())
    }

    /// Sign a Signable object using canonical CBOR encoding
    pub fn sign<T: Signable>(&self, signable: &T) -> Result<Vec<u8>> {
        let keys = self.get_unlocked_keys()?;
        signing_sign(&keys.ed25519_signing, signable)
    }

    /// Get the full identity (for internal use)
    pub fn get_identity(&self) -> Result<Option<LocalIdentity>> {
        let repo = IdentityRepository::new(&self.db);
        repo.get().map_err(Into::into)
    }

    /// Update display name
    pub fn update_display_name(&self, display_name: &str) -> Result<()> {
        let repo = IdentityRepository::new(&self.db);
        repo.update_display_name(display_name)?;
        Ok(())
    }

    /// Update bio
    pub fn update_bio(&self, bio: Option<&str>) -> Result<()> {
        let repo = IdentityRepository::new(&self.db);
        repo.update_bio(bio)?;
        Ok(())
    }

    /// Update passphrase hint
    pub fn update_passphrase_hint(&self, hint: Option<&str>) -> Result<()> {
        let repo = IdentityRepository::new(&self.db);
        repo.update_passphrase_hint(hint)?;
        Ok(())
    }

    /// Get the local peer ID
    pub fn get_peer_id(&self) -> Result<String> {
        let repo = IdentityRepository::new(&self.db);
        let identity = repo
            .get()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;
        Ok(identity.peer_id)
    }
}

impl Clone for IdentityService {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            unlocked_keys: Arc::clone(&self.unlocked_keys),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> IdentityService {
        let db = Arc::new(Database::in_memory().unwrap());
        IdentityService::new(db)
    }

    #[test]
    fn test_no_identity_initially() {
        let service = create_test_service();
        assert!(!service.has_identity().unwrap());
        assert!(!service.is_unlocked());
    }

    #[test]
    fn test_create_identity() {
        let service = create_test_service();

        let request = CreateIdentityRequest {
            display_name: "Test User".to_string(),
            passphrase: "test-passphrase".to_string(),
            bio: Some("Test bio".to_string()),
            passphrase_hint: Some("Test hint".to_string()),
        };

        let info = service.create_identity(request).unwrap();

        assert!(info.peer_id.starts_with("12D3KooW"));
        assert_eq!(info.display_name, "Test User");
        assert_eq!(info.bio, Some("Test bio".to_string()));
        assert_eq!(info.passphrase_hint, Some("Test hint".to_string()));

        // Should be auto-unlocked after creation
        assert!(service.is_unlocked());
        assert!(service.has_identity().unwrap());
    }

    #[test]
    fn test_lock_unlock() {
        let service = create_test_service();

        let request = CreateIdentityRequest {
            display_name: "Test User".to_string(),
            passphrase: "test-passphrase".to_string(),
            bio: None,
            passphrase_hint: None,
        };

        service.create_identity(request).unwrap();
        assert!(service.is_unlocked());

        // Lock
        service.lock();
        assert!(!service.is_unlocked());

        // Unlock with correct passphrase
        service.unlock("test-passphrase").unwrap();
        assert!(service.is_unlocked());
    }

    #[test]
    fn test_wrong_passphrase() {
        let service = create_test_service();

        let request = CreateIdentityRequest {
            display_name: "Test User".to_string(),
            passphrase: "correct-passphrase".to_string(),
            bio: None,
            passphrase_hint: None,
        };

        service.create_identity(request).unwrap();
        service.lock();

        // Wrong passphrase should fail
        let result = service.unlock("wrong-passphrase");
        assert!(result.is_err());
    }

    #[test]
    fn test_sign_requires_unlock() {
        let service = create_test_service();

        let request = CreateIdentityRequest {
            display_name: "Test User".to_string(),
            passphrase: "test-passphrase".to_string(),
            bio: None,
            passphrase_hint: None,
        };

        service.create_identity(request).unwrap();

        // Can sign when unlocked
        let signature = service.sign_raw(b"test data").unwrap();
        assert!(!signature.is_empty());

        // Lock
        service.lock();

        // Cannot sign when locked
        let result = service.sign_raw(b"test data");
        assert!(result.is_err());
    }
}
