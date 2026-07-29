use crate::db::repositories::IdentityRepository;
use crate::db::Database;
use crate::error::{AppError, Result};
use crate::models::{CreateIdentityRequest, IdentityInfo, LocalIdentity};
use crate::services::{sign as signing_sign, CryptoService, Signable};

use ed25519_dalek::{SigningKey, VerifyingKey};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvatarProfileUpdate {
    pub old_avatar_hash: Option<String>,
    pub old_avatar_mime_type: Option<String>,
    pub avatar_hash: Option<String>,
    pub avatar_mime_type: Option<String>,
    pub revision: u64,
}

/// Coherent backend snapshot used by application initialization. Repository
/// absence is represented separately from locked and unlocked identities;
/// failures remain errors for the command layer to classify explicitly.
pub enum IdentityInitializationSnapshot {
    Absent,
    Locked(IdentityInfo),
    Unlocked(IdentityInfo),
}

impl IdentityService {
    const MIN_PASSWORD_CHARACTERS: usize = 8;
    const MAX_PASSWORD_BYTES: usize = 1_024;

    fn validate_new_password(password: &str) -> Result<()> {
        if password.chars().count() < Self::MIN_PASSWORD_CHARACTERS {
            return Err(AppError::Validation(format!(
                "Password must be at least {} characters",
                Self::MIN_PASSWORD_CHARACTERS
            )));
        }
        if password.len() > Self::MAX_PASSWORD_BYTES {
            return Err(AppError::Validation(format!(
                "Password must be at most {} bytes",
                Self::MAX_PASSWORD_BYTES
            )));
        }
        if password.chars().all(char::is_whitespace) {
            return Err(AppError::Validation(
                "Password cannot contain only whitespace".into(),
            ));
        }
        Ok(())
    }

    fn validate_key_material(identity: &LocalIdentity, keys: &UnlockedKeys) -> Result<()> {
        let derived_public = keys.ed25519_signing.verifying_key().to_bytes();
        if identity.public_key.as_slice() != derived_public.as_slice() {
            return Err(AppError::Crypto(
                "Stored Ed25519 public key does not match the decrypted signing key".into(),
            ));
        }

        let derived_peer_id =
            CryptoService::derive_peer_id_from_signing_key(&keys.ed25519_signing)?;
        if identity.peer_id != derived_peer_id {
            return Err(AppError::Crypto(
                "Stored PeerId does not match the decrypted signing key".into(),
            ));
        }

        let derived_x25519 = x25519_dalek::PublicKey::from(&keys.x25519_secret).to_bytes();
        if identity.x25519_public.as_slice() != derived_x25519.as_slice() {
            return Err(AppError::Crypto(
                "Stored X25519 public key does not match the decrypted agreement key".into(),
            ));
        }
        Ok(())
    }

    fn validate_stored_identity(identity: &LocalIdentity) -> Result<()> {
        let public_key: [u8; 32] = identity.public_key.as_slice().try_into().map_err(|_| {
            AppError::Crypto("Stored Ed25519 public key has an invalid length".into())
        })?;
        let verifying_key = VerifyingKey::from_bytes(&public_key).map_err(|error| {
            AppError::Crypto(format!("Stored Ed25519 public key is invalid: {error}"))
        })?;
        let derived_peer_id = CryptoService::derive_peer_id_from_verifying_key(&verifying_key)?;
        if identity.peer_id != derived_peer_id {
            return Err(AppError::Crypto(
                "Stored PeerId does not match the public signing key".into(),
            ));
        }
        let _: [u8; 32] = identity.x25519_public.as_slice().try_into().map_err(|_| {
            AppError::Crypto("Stored X25519 public key has an invalid length".into())
        })?;
        CryptoService::validate_encrypted_key_envelope(&identity.private_key_encrypted)
    }

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

    /// Return one authoritative identity-entry snapshot from a single repository
    /// read while the key-cache read lock is held. A stale unlocked cache without
    /// its persisted identity is corruption, never a signal to create a new one.
    pub fn initialization_snapshot(&self) -> Result<IdentityInitializationSnapshot> {
        let unlocked = self.read_keys();
        let identity = IdentityRepository::new(&self.db).get()?;
        let Some(identity) = identity else {
            if unlocked.is_some() {
                return Err(AppError::InvalidData(
                    "Unlocked identity keys exist without a persisted identity".into(),
                ));
            }
            return Ok(IdentityInitializationSnapshot::Absent);
        };

        Self::validate_stored_identity(&identity)?;
        let is_unlocked = if let Some(keys) = unlocked.as_ref() {
            Self::validate_key_material(&identity, keys)?;
            true
        } else {
            false
        };
        let identity = IdentityInfo::from(identity);
        if is_unlocked {
            Ok(IdentityInitializationSnapshot::Unlocked(identity))
        } else {
            Ok(IdentityInitializationSnapshot::Locked(identity))
        }
    }

    /// Create a new identity with the given display name and passphrase
    pub fn create_identity(&self, request: CreateIdentityRequest) -> Result<IdentityInfo> {
        Self::validate_new_password(&request.passphrase)?;
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
        let peer_id = CryptoService::derive_peer_id_from_signing_key(&ed25519_signing)?;
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

        let unlocked_keys = UnlockedKeys {
            ed25519_signing,
            x25519_secret,
        };
        Self::validate_key_material(&identity, &unlocked_keys)?;
        repo.create(&identity)?;

        // Auto-unlock after creation
        {
            let mut unlocked = self.write_keys();
            *unlocked = Some(unlocked_keys);
        }

        info!("Created new identity: {}", peer_id);
        Ok(identity.into())
    }

    /// Roll back a newly-created identity when the account registry cannot commit.
    /// The expected peer guard prevents a stale caller from deleting another identity.
    pub fn rollback_created_identity(&self, expected_peer_id: &str) -> Result<()> {
        let deleted = IdentityRepository::new(&self.db).delete_if_peer_id(expected_peer_id)?;
        if !deleted {
            return Err(AppError::Internal(format!(
                "Refused to roll back identity because peer {} is not the stored identity",
                expected_peer_id
            )));
        }
        self.lock();
        Ok(())
    }

    /// Unlock the identity with the passphrase
    pub fn unlock(&self, passphrase: &str) -> Result<IdentityInfo> {
        // A failed unlock must never leave a stale session usable.
        self.lock();
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

        let unlocked_keys = UnlockedKeys {
            ed25519_signing,
            x25519_secret,
        };
        Self::validate_key_material(&identity, &unlocked_keys)?;

        // Store unlocked keys
        {
            let mut unlocked = self.write_keys();
            *unlocked = Some(unlocked_keys);
        }

        info!("Identity unlocked: {}", identity.peer_id);
        Ok(identity.into())
    }

    /// Re-encrypt the existing private keys under a new password. The public
    /// identity and unlocked key material do not change.
    pub fn change_password(&self, current_password: &str, new_password: &str) -> Result<()> {
        Self::validate_new_password(new_password)?;
        if current_password == new_password {
            return Err(AppError::Validation(
                "New password must be different from the current password".into(),
            ));
        }

        let repo = IdentityRepository::new(&self.db);
        let identity = repo
            .get()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;
        let decrypted =
            CryptoService::decrypt_keys(&identity.private_key_encrypted, current_password)?;

        let ed25519_bytes: [u8; 32] = decrypted
            .ed25519_private
            .as_slice()
            .try_into()
            .map_err(|_| AppError::Crypto("Invalid Ed25519 key length".to_string()))?;
        let x25519_bytes: [u8; 32] = decrypted
            .x25519_private
            .as_slice()
            .try_into()
            .map_err(|_| AppError::Crypto("Invalid X25519 key length".to_string()))?;
        let keys = UnlockedKeys {
            ed25519_signing: SigningKey::from_bytes(&ed25519_bytes),
            x25519_secret: X25519Secret::from(x25519_bytes),
        };
        Self::validate_key_material(&identity, &keys)?;

        let replacement = CryptoService::encrypt_keys(
            &decrypted.ed25519_private,
            &decrypted.x25519_private,
            new_password,
        )?;
        if !repo.replace_encrypted_private_key(&identity.private_key_encrypted, &replacement)? {
            return Err(AppError::DatabaseString(
                "Identity changed while password rotation was in progress; no changes were saved"
                    .into(),
            ));
        }

        info!("Identity password changed");
        Ok(())
    }

    /// Lock the identity (clear unlocked keys from memory)
    pub fn lock(&self) {
        let mut unlocked = self.write_keys();
        *unlocked = None;
        info!("Identity locked");
    }

    /// Get the unlocked keys (for signing/encryption operations)
    pub fn get_unlocked_keys(&self) -> Result<UnlockedKeys> {
        self.read_keys()
            .clone()
            .ok_or_else(|| AppError::IdentityLocked("Identity is locked".to_string()))
    }

    /// Reload persisted identity metadata and validate it against the cached keys.
    /// Network startup uses this immediately before constructing or publishing a
    /// network service, without adding a database read to every signing operation.
    pub fn get_validated_unlocked_keys(&self) -> Result<UnlockedKeys> {
        let keys = self.get_unlocked_keys()?;
        let identity = IdentityRepository::new(&self.db)
            .get()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;
        if let Err(error) = Self::validate_key_material(&identity, &keys) {
            self.lock();
            return Err(error);
        }
        Ok(keys)
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

    /// Commit a local avatar hash and strictly monotonic signed profile
    /// revision in one SQLite transaction.
    pub fn replace_avatar(
        &self,
        avatar_hash: Option<&str>,
        avatar_mime_type: Option<&str>,
    ) -> Result<AvatarProfileUpdate> {
        self.get_unlocked_keys()?;
        if avatar_hash
            .is_some_and(|hash| hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()))
            || avatar_mime_type.is_some_and(|mime| !mime.starts_with("image/") || mime.len() > 128)
        {
            return Err(AppError::Validation(
                "Invalid profile avatar metadata".into(),
            ));
        }
        self.db
            .with_connection(|connection| {
                let transaction = connection.unchecked_transaction()?;
                let old_avatar_hash: Option<String> = transaction.query_row(
                    "SELECT avatar_hash FROM local_identity WHERE id = 1",
                    [],
                    |row| row.get(0),
                )?;
                let current: i64 = transaction.query_row(
                    "SELECT revision FROM local_profile_state WHERE id = 1",
                    [],
                    |row| row.get(0),
                )?;
                let old_avatar_mime_type: Option<String> = transaction.query_row(
                    "SELECT avatar_mime_type FROM local_profile_state WHERE id = 1",
                    [],
                    |row| row.get(0),
                )?;
                let revision = current.saturating_add(1);
                let now = chrono::Utc::now().timestamp();
                transaction.execute(
                    "UPDATE local_identity SET avatar_hash = ?, updated_at = ? WHERE id = 1",
                    rusqlite::params![avatar_hash, now],
                )?;
                transaction.execute(
                    "UPDATE local_profile_state SET revision = ?, avatar_mime_type = ?, updated_at = ? WHERE id = 1",
                    rusqlite::params![revision, avatar_mime_type, now],
                )?;
                transaction.commit()?;
                Ok(AvatarProfileUpdate {
                    old_avatar_hash,
                    old_avatar_mime_type,
                    avatar_hash: avatar_hash.map(str::to_owned),
                    avatar_mime_type: avatar_mime_type.map(str::to_owned),
                    revision: revision as u64,
                })
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))
    }

    pub fn profile_revision(&self) -> Result<(u64, Option<String>)> {
        self.db
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT revision, avatar_mime_type FROM local_profile_state WHERE id = 1",
                    [],
                    |row| Ok((row.get::<_, i64>(0)? as u64, row.get(1)?)),
                )
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))
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

    const TEST_PASSPHRASE: &str = "test-passphrase";

    #[derive(Debug, Clone, Copy)]
    enum IdentityCorruption {
        PeerId,
        Ed25519Public,
        X25519Public,
        Ed25519Private,
        X25519Private,
    }

    fn create_test_service() -> IdentityService {
        let db = Arc::new(Database::in_memory().unwrap());
        IdentityService::new(db)
    }

    fn create_locked_service() -> IdentityService {
        let service = create_test_service();
        service
            .create_identity(CreateIdentityRequest {
                display_name: "Test User".into(),
                passphrase: TEST_PASSPHRASE.into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        service.lock();
        service
    }

    fn corrupt_identity(service: &IdentityService, corruption: IdentityCorruption) {
        let stored = IdentityRepository::new(&service.db).get().unwrap().unwrap();
        match corruption {
            IdentityCorruption::PeerId => {
                let (other_key, _) = CryptoService::generate_ed25519_keypair();
                let other_peer =
                    CryptoService::derive_peer_id_from_signing_key(&other_key).unwrap();
                service
                    .db
                    .with_connection(|connection| {
                        connection.execute(
                            "UPDATE local_identity SET peer_id=? WHERE id=1",
                            [other_peer],
                        )?;
                        Ok(())
                    })
                    .unwrap();
            }
            IdentityCorruption::Ed25519Public => {
                let (_, public) = CryptoService::generate_ed25519_keypair();
                service
                    .db
                    .with_connection(|connection| {
                        connection.execute(
                            "UPDATE local_identity SET public_key=? WHERE id=1",
                            [public.to_bytes().to_vec()],
                        )?;
                        Ok(())
                    })
                    .unwrap();
            }
            IdentityCorruption::X25519Public => {
                let (_, public) = CryptoService::generate_x25519_keypair();
                service
                    .db
                    .with_connection(|connection| {
                        connection.execute(
                            "UPDATE local_identity SET x25519_public=? WHERE id=1",
                            [public.to_bytes().to_vec()],
                        )?;
                        Ok(())
                    })
                    .unwrap();
            }
            IdentityCorruption::Ed25519Private | IdentityCorruption::X25519Private => {
                let mut keys =
                    CryptoService::decrypt_keys(&stored.private_key_encrypted, TEST_PASSPHRASE)
                        .unwrap();
                match corruption {
                    IdentityCorruption::Ed25519Private => {
                        keys.ed25519_private = CryptoService::generate_ed25519_keypair()
                            .0
                            .to_bytes()
                            .to_vec()
                    }
                    IdentityCorruption::X25519Private => {
                        keys.x25519_private = CryptoService::generate_x25519_keypair()
                            .0
                            .as_bytes()
                            .to_vec()
                    }
                    _ => unreachable!(),
                }
                let encrypted = CryptoService::encrypt_keys(
                    &keys.ed25519_private,
                    &keys.x25519_private,
                    TEST_PASSPHRASE,
                )
                .unwrap();
                service
                    .db
                    .with_connection(|connection| {
                        connection.execute(
                            "UPDATE local_identity SET private_key_encrypted=? WHERE id=1",
                            [encrypted],
                        )?;
                        Ok(())
                    })
                    .unwrap();
            }
        }
    }

    fn assert_unlock_rejects(corruption: IdentityCorruption) {
        let service = create_locked_service();
        corrupt_identity(&service, corruption);
        let error = service.unlock(TEST_PASSPHRASE).unwrap_err();
        assert!(
            matches!(error, AppError::Crypto(_)),
            "{corruption:?}: {error}"
        );
        assert!(
            !service.is_unlocked(),
            "{corruption:?} published unlocked keys"
        );
    }

    #[test]
    fn unlock_rejects_inconsistent_peer_id() {
        assert_unlock_rejects(IdentityCorruption::PeerId);
    }

    #[test]
    fn unlock_rejects_inconsistent_ed25519_public_key() {
        assert_unlock_rejects(IdentityCorruption::Ed25519Public);
    }

    #[test]
    fn unlock_rejects_inconsistent_x25519_public_key() {
        assert_unlock_rejects(IdentityCorruption::X25519Public);
    }

    #[test]
    fn unlock_rejects_inconsistent_ed25519_private_key() {
        assert_unlock_rejects(IdentityCorruption::Ed25519Private);
    }

    #[test]
    fn unlock_rejects_inconsistent_x25519_private_key() {
        assert_unlock_rejects(IdentityCorruption::X25519Private);
    }

    #[test]
    fn cached_keys_are_revalidated_before_network_use() {
        let service = create_test_service();
        service
            .create_identity(CreateIdentityRequest {
                display_name: "Test User".into(),
                passphrase: TEST_PASSPHRASE.into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        corrupt_identity(&service, IdentityCorruption::Ed25519Public);

        assert!(matches!(
            service.get_validated_unlocked_keys(),
            Err(AppError::Crypto(_))
        ));
        assert!(!service.is_unlocked());
        assert!(matches!(
            service.sign_raw(b"must not sign"),
            Err(AppError::IdentityLocked(_))
        ));
    }

    #[test]
    fn failed_integrity_unlock_clears_an_existing_session() {
        let service = create_test_service();
        service
            .create_identity(CreateIdentityRequest {
                display_name: "Test User".into(),
                passphrase: TEST_PASSPHRASE.into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        assert!(service.is_unlocked());

        corrupt_identity(&service, IdentityCorruption::PeerId);
        assert!(matches!(
            service.unlock(TEST_PASSPHRASE),
            Err(AppError::Crypto(_))
        ));
        assert!(!service.is_unlocked());
    }

    #[test]
    fn weak_password_is_rejected_at_the_service_boundary() {
        for password in [
            String::new(),
            "1234567".into(),
            "        ".into(),
            "x".repeat(IdentityService::MAX_PASSWORD_BYTES + 1),
        ] {
            let service = create_test_service();
            let result = service.create_identity(CreateIdentityRequest {
                display_name: "Test User".into(),
                passphrase: password,
                bio: None,
                passphrase_hint: None,
            });

            assert!(matches!(result, Err(AppError::Validation(_))));
            assert!(!service.has_identity().unwrap());
        }
    }

    #[test]
    fn password_rotation_survives_service_restart_and_invalidates_old_password() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("identity.db");
        let original = {
            let db = Arc::new(Database::new(path.clone()).unwrap());
            let service = IdentityService::new(db);
            let original = service
                .create_identity(CreateIdentityRequest {
                    display_name: "Test User".into(),
                    passphrase: "old-password".into(),
                    bio: None,
                    passphrase_hint: None,
                })
                .unwrap();
            service
                .change_password("old-password", "new-password")
                .unwrap();
            assert!(service.is_unlocked());
            original
        };

        let restarted = IdentityService::new(Arc::new(Database::new(path).unwrap()));
        assert!(matches!(
            restarted.unlock("old-password"),
            Err(AppError::IdentityInvalidPassphrase(_))
        ));
        let unlocked = restarted.unlock("new-password").unwrap();
        assert_eq!(unlocked.peer_id, original.peer_id);
        assert_eq!(unlocked.public_key, original.public_key);
        assert_eq!(unlocked.x25519_public, original.x25519_public);
    }

    #[test]
    fn failed_password_rotation_preserves_the_previous_encrypted_keys() {
        let db = Arc::new(Database::in_memory().unwrap());
        let service = IdentityService::new(db.clone());
        service
            .create_identity(CreateIdentityRequest {
                display_name: "Test User".into(),
                passphrase: "old-password".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        db.with_connection(|connection| {
            connection.execute_batch(
                "CREATE TRIGGER interrupt_password_rotation
                 BEFORE UPDATE OF private_key_encrypted ON local_identity
                 BEGIN
                   SELECT RAISE(ABORT, 'simulated interruption');
                 END;",
            )
        })
        .unwrap();

        assert!(matches!(
            service.change_password("old-password", "new-password"),
            Err(AppError::Database(_))
        ));
        db.with_connection(|connection| {
            connection.execute_batch("DROP TRIGGER interrupt_password_rotation")
        })
        .unwrap();

        let old_password_service = IdentityService::new(db.clone());
        assert!(old_password_service.unlock("old-password").is_ok());
        let new_password_service = IdentityService::new(db);
        assert!(matches!(
            new_password_service.unlock("new-password"),
            Err(AppError::IdentityInvalidPassphrase(_))
        ));
    }

    #[test]
    fn weak_rotation_password_is_rejected_without_changing_the_identity() {
        let db = Arc::new(Database::in_memory().unwrap());
        let service = IdentityService::new(db.clone());
        service
            .create_identity(CreateIdentityRequest {
                display_name: "Test User".into(),
                passphrase: "old-password".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();

        assert!(matches!(
            service.change_password("old-password", "short"),
            Err(AppError::Validation(_))
        ));
        let restarted = IdentityService::new(db);
        assert!(restarted.unlock("old-password").is_ok());
    }

    #[test]
    fn wrong_current_password_does_not_rotate_or_change_lock_state() {
        let db = Arc::new(Database::in_memory().unwrap());
        let service = IdentityService::new(db.clone());
        service
            .create_identity(CreateIdentityRequest {
                display_name: "Test User".into(),
                passphrase: "old-password".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        service.lock();

        assert!(matches!(
            service.change_password("wrong-password", "new-password"),
            Err(AppError::IdentityInvalidPassphrase(_))
        ));
        assert!(!service.is_unlocked());
        let restarted = IdentityService::new(db);
        assert!(restarted.unlock("old-password").is_ok());
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

    #[test]
    fn avatar_revision_is_monotonic_and_failed_replacement_preserves_old_hash() {
        let service = create_test_service();
        service
            .create_identity(CreateIdentityRequest {
                display_name: "Avatar owner".into(),
                passphrase: "test-passphrase".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        let first_hash = "1".repeat(64);
        let first = service
            .replace_avatar(Some(&first_hash), Some("image/png"))
            .unwrap();
        let second_hash = "2".repeat(64);
        let second = service
            .replace_avatar(Some(&second_hash), Some("image/webp"))
            .unwrap();
        assert!(second.revision > first.revision);

        assert!(service
            .replace_avatar(Some("invalid"), Some("image/png"))
            .is_err());
        let identity = service.get_identity().unwrap().unwrap();
        assert_eq!(identity.avatar_hash.as_deref(), Some(second_hash.as_str()));
        assert_eq!(service.profile_revision().unwrap().0, second.revision);
    }
}
