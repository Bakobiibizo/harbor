use crate::db::repositories::{IdentityRepository, RelayNamesRepository};
use crate::db::Database;
use crate::error::{AppError, Result};
use crate::models::{EncryptedKeys, LocalIdentity, NameClaim};
use crate::services::{AccountsService, CryptoService, IdentityService};
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use ed25519_dalek::SigningKey;
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use x25519_dalek::{PublicKey as X25519Public, StaticSecret as X25519Secret};

use super::accounts_service::AccountInfo;
use super::name_claim_service::verified_name_claim;

const ARCHIVE_MAGIC: &[u8; 8] = b"HRBRIDB\0";
const ARCHIVE_VERSION: u16 = 1;
const KDF_ARGON2ID: u8 = 1;
const CIPHER_AES_256_GCM: u8 = 1;
const ARGON_MEMORY_KIB: u32 = 64 * 1024;
const ARGON_ITERATIONS: u32 = 3;
const ARGON_LANES: u8 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const HEADER_LEN: usize = 8 + 2 + 1 + 1 + 4 + 4 + 1 + SALT_LEN + NONCE_LEN + 8;
const MAX_ARCHIVE_BYTES: usize = 2 * 1024 * 1024;
const MAX_PLAINTEXT_BYTES: usize = 1024 * 1024;
const MAX_PASSWORD_BYTES: usize = 1024;
const DELETE_JOURNAL_DIR: &str = ".account-deletions";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupExportResult {
    pub peer_id: String,
    pub path: String,
    pub created_at: i64,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreResult {
    pub account: AccountInfo,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccountProfileResult {
    pub restart_required: bool,
    pub next_account_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityArchiveV1 {
    archive_domain: String,
    archive_version: u16,
    identity: ArchivedIdentity,
    keys: EncryptedKeys,
    publishing_state: Option<PublishingState>,
    relay_keys: Vec<RelayKey>,
    relay_claims: Vec<RelayClaim>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchivedIdentity {
    peer_id: String,
    public_key: Vec<u8>,
    x25519_public: Vec<u8>,
    display_name: String,
    avatar_hash: Option<String>,
    bio: Option<String>,
    passphrase_hint: Option<String>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishingState {
    mode: String,
    updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelayKey {
    relay: String,
    key_id: String,
    public_key: Vec<u8>,
    not_before: i64,
    not_after: Option<i64>,
    retired_at: Option<i64>,
    sequence: i64,
    compromise_from: Option<i64>,
    rotation_cbor: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelayClaim {
    qualified_name: String,
    local_name: String,
    relay: String,
    peer_id: String,
    sequence: i64,
    claim_cbor: Vec<u8>,
    not_before: i64,
    not_after: i64,
    relay_key_id: String,
    status: String,
    verified_at: i64,
    retired_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
struct DeletionJournal {
    version: u16,
    account_id: String,
    data_path: String,
    scheduled_at: i64,
}

pub struct AccountBackupService {
    accounts: Arc<AccountsService>,
    identity: Arc<IdentityService>,
    #[cfg(test)]
    fail_restore_after_staging: AtomicBool,
    #[cfg(test)]
    fail_delete_after_journal: AtomicBool,
}

impl AccountBackupService {
    pub fn new(accounts: Arc<AccountsService>, identity: Arc<IdentityService>) -> Self {
        Self {
            accounts,
            identity,
            #[cfg(test)]
            fail_restore_after_staging: AtomicBool::new(false),
            #[cfg(test)]
            fail_delete_after_journal: AtomicBool::new(false),
        }
    }

    #[cfg(test)]
    fn inject_restore_failure_after_staging(&self) {
        self.fail_restore_after_staging
            .store(true, Ordering::SeqCst);
    }

    #[cfg(test)]
    fn inject_delete_failure_after_journal(&self) {
        self.fail_delete_after_journal.store(true, Ordering::SeqCst);
    }

    fn validate_password(password: &str) -> Result<()> {
        if password.chars().count() < 8 || password.len() > MAX_PASSWORD_BYTES {
            return Err(AppError::Validation(
                "Password must be between 8 and 1024 bytes".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_identity(identity: &ArchivedIdentity, keys: &EncryptedKeys) -> Result<()> {
        let ed25519: [u8; 32] = keys
            .ed25519_private
            .as_slice()
            .try_into()
            .map_err(|_| AppError::InvalidData("Backup has an invalid signing key".into()))?;
        let x25519: [u8; 32] =
            keys.x25519_private.as_slice().try_into().map_err(|_| {
                AppError::InvalidData("Backup has an invalid encryption key".into())
            })?;
        let signing = SigningKey::from_bytes(&ed25519);
        if signing.verifying_key().to_bytes().as_slice() != identity.public_key.as_slice() {
            return Err(AppError::InvalidData(
                "Backup signing key does not match its public identity".into(),
            ));
        }
        if CryptoService::derive_peer_id_from_signing_key(&signing)? != identity.peer_id {
            return Err(AppError::InvalidData(
                "Backup signing key does not derive its peer ID".into(),
            ));
        }
        let x25519_public = X25519Public::from(&X25519Secret::from(x25519)).to_bytes();
        if x25519_public.as_slice() != identity.x25519_public.as_slice() {
            return Err(AppError::InvalidData(
                "Backup encryption key does not match its public identity".into(),
            ));
        }
        libp2p::PeerId::from_str(&identity.peer_id)
            .map_err(|_| AppError::InvalidData("Backup contains an invalid peer ID".into()))?;
        if identity.display_name.trim().is_empty() || identity.display_name.len() > 256 {
            return Err(AppError::InvalidData(
                "Backup contains an invalid account name".into(),
            ));
        }
        Ok(())
    }

    fn archive_header(salt: &[u8; SALT_LEN], nonce: &[u8; NONCE_LEN], length: u64) -> Vec<u8> {
        let mut header = Vec::with_capacity(HEADER_LEN);
        header.extend_from_slice(ARCHIVE_MAGIC);
        header.extend_from_slice(&ARCHIVE_VERSION.to_be_bytes());
        header.push(KDF_ARGON2ID);
        header.push(CIPHER_AES_256_GCM);
        header.extend_from_slice(&ARGON_MEMORY_KIB.to_be_bytes());
        header.extend_from_slice(&ARGON_ITERATIONS.to_be_bytes());
        header.push(ARGON_LANES);
        header.extend_from_slice(salt);
        header.extend_from_slice(nonce);
        header.extend_from_slice(&length.to_be_bytes());
        header
    }

    fn derive_archive_key(password: &str, salt: &[u8; SALT_LEN]) -> Result<[u8; 32]> {
        Self::validate_password(password)?;
        let params = Params::new(
            ARGON_MEMORY_KIB,
            ARGON_ITERATIONS,
            ARGON_LANES as u32,
            Some(32),
        )
        .map_err(|error| AppError::Crypto(format!("Invalid backup KDF parameters: {error}")))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = [0u8; 32];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .map_err(|error| AppError::Crypto(format!("Backup key derivation failed: {error}")))?;
        Ok(key)
    }

    fn encrypt_archive(archive: &IdentityArchiveV1, password: &str) -> Result<Vec<u8>> {
        let mut plaintext = Vec::new();
        ciborium::ser::into_writer(archive, &mut plaintext).map_err(|error| {
            AppError::Serialization(format!("Could not encode identity backup: {error}"))
        })?;
        if plaintext.len() > MAX_PLAINTEXT_BYTES {
            return Err(AppError::InvalidData("Identity backup is too large".into()));
        }
        let mut salt = [0u8; SALT_LEN];
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut nonce);
        let ciphertext_length = plaintext
            .len()
            .checked_add(16)
            .ok_or_else(|| AppError::InvalidData("Identity backup length overflow".into()))?;
        let header = Self::archive_header(&salt, &nonce, ciphertext_length as u64);
        let key = Self::derive_archive_key(password, &salt)?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|error| AppError::CryptoEncryption(error.to_string()))?;
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &plaintext,
                    aad: &header,
                },
            )
            .map_err(|_| AppError::CryptoEncryption("Could not encrypt identity backup".into()))?;
        let mut bytes = header;
        bytes.extend_from_slice(&ciphertext);
        Ok(bytes)
    }

    fn decrypt_archive(bytes: &[u8], password: &str) -> Result<IdentityArchiveV1> {
        if bytes.len() < HEADER_LEN + 16 || bytes.len() > MAX_ARCHIVE_BYTES {
            return Err(AppError::InvalidData(
                "Identity backup has an invalid size".into(),
            ));
        }
        if &bytes[..8] != ARCHIVE_MAGIC {
            return Err(AppError::InvalidData(
                "This is not a Harbor identity backup".into(),
            ));
        }
        let version = u16::from_be_bytes([bytes[8], bytes[9]]);
        if version != ARCHIVE_VERSION
            || bytes[10] != KDF_ARGON2ID
            || bytes[11] != CIPHER_AES_256_GCM
        {
            return Err(AppError::InvalidData(
                "Unsupported identity backup version or cryptography".into(),
            ));
        }
        let memory = u32::from_be_bytes(bytes[12..16].try_into().unwrap());
        let iterations = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let lanes = bytes[20];
        if memory != ARGON_MEMORY_KIB || iterations != ARGON_ITERATIONS || lanes != ARGON_LANES {
            return Err(AppError::InvalidData(
                "Identity backup uses unsupported KDF parameters".into(),
            ));
        }
        let salt: [u8; SALT_LEN] = bytes[21..37].try_into().unwrap();
        let nonce: [u8; NONCE_LEN] = bytes[37..49].try_into().unwrap();
        let declared = u64::from_be_bytes(bytes[49..57].try_into().unwrap());
        if declared > MAX_ARCHIVE_BYTES as u64
            || declared as usize != bytes.len().saturating_sub(HEADER_LEN)
        {
            return Err(AppError::InvalidData(
                "Identity backup length does not match its header".into(),
            ));
        }
        let key = Self::derive_archive_key(password, &salt)?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|error| AppError::CryptoDecryption(error.to_string()))?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &bytes[HEADER_LEN..],
                    aad: &bytes[..HEADER_LEN],
                },
            )
            .map_err(|_| {
                AppError::IdentityInvalidPassphrase(
                    "Backup password is wrong or the backup was modified".into(),
                )
            })?;
        if plaintext.len() > MAX_PLAINTEXT_BYTES {
            return Err(AppError::InvalidData("Identity backup is too large".into()));
        }
        let mut payload = Cursor::new(plaintext.as_slice());
        let archive: IdentityArchiveV1 = ciborium::de::from_reader(&mut payload)
            .map_err(|_| AppError::InvalidData("Identity backup payload is invalid".into()))?;
        if payload.position() != plaintext.len() as u64 {
            return Err(AppError::InvalidData(
                "Identity backup payload has trailing data".into(),
            ));
        }
        if archive.archive_domain != "harbor/identity-backup"
            || archive.archive_version != ARCHIVE_VERSION
        {
            return Err(AppError::InvalidData(
                "Identity backup payload has an unsupported version".into(),
            ));
        }
        Self::validate_identity(&archive.identity, &archive.keys)?;
        for claim in &archive.relay_claims {
            if claim.peer_id != archive.identity.peer_id {
                return Err(AppError::InvalidData(
                    "Identity backup contains a relay name for another identity".into(),
                ));
            }
            let decoded: NameClaim = ciborium::de::from_reader(claim.claim_cbor.as_slice())
                .map_err(|_| AppError::InvalidData("Backup relay name is malformed".into()))?;
            if decoded.request.peer_id != archive.identity.peer_id
                || decoded.request.ed25519_public_key != archive.identity.public_key
                || decoded.request.x25519_public_key != archive.identity.x25519_public
                || decoded.request.local_name != claim.local_name
                || decoded.request.relay != claim.relay
                || i64::try_from(decoded.request.sequence).ok() != Some(claim.sequence)
                || decoded.not_before != claim.not_before
                || decoded.not_after != claim.not_after
                || decoded.relay_key_id != claim.relay_key_id
                || decoded.status != claim.status
                || format!("@{}@{}", claim.local_name, claim.relay) != claim.qualified_name
            {
                return Err(AppError::InvalidData(
                    "Backup relay name is not bound to the archived identity".into(),
                ));
            }
        }
        Ok(archive)
    }

    fn continuity(
        db: &Database,
        peer_id: &str,
    ) -> Result<(Option<PublishingState>, Vec<RelayKey>, Vec<RelayClaim>)> {
        db.with_connection(|connection| {
            let publishing_state = connection
                .query_row(
                    "SELECT mode, updated_at FROM identity_publishing_state WHERE peer_id = ?1",
                    [peer_id],
                    |row| {
                        Ok(PublishingState {
                            mode: row.get(0)?,
                            updated_at: row.get(1)?,
                        })
                    },
                )
                .optional()?;
            let mut claim_statement = connection.prepare(
                "SELECT qualified_name, local_name, relay, peer_id, sequence, claim_cbor,
                        not_before, not_after, relay_key_id, status, verified_at, retired_at
                 FROM relay_name_claims WHERE peer_id = ?1",
            )?;
            let relay_claims = claim_statement
                .query_map([peer_id], |row| {
                    Ok(RelayClaim {
                        qualified_name: row.get(0)?,
                        local_name: row.get(1)?,
                        relay: row.get(2)?,
                        peer_id: row.get(3)?,
                        sequence: row.get(4)?,
                        claim_cbor: row.get(5)?,
                        not_before: row.get(6)?,
                        not_after: row.get(7)?,
                        relay_key_id: row.get(8)?,
                        status: row.get(9)?,
                        verified_at: row.get(10)?,
                        retired_at: row.get(11)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let mut keys = Vec::new();
            for claim in &relay_claims {
                let key = connection
                    .query_row(
                        "SELECT relay, key_id, public_key, not_before, not_after, retired_at,
                                sequence, compromise_from, rotation_cbor
                         FROM relay_trust_keys WHERE relay = ?1 AND key_id = ?2",
                        rusqlite::params![claim.relay, claim.relay_key_id],
                        |row| {
                            Ok(RelayKey {
                                relay: row.get(0)?,
                                key_id: row.get(1)?,
                                public_key: row.get(2)?,
                                not_before: row.get(3)?,
                                not_after: row.get(4)?,
                                retired_at: row.get(5)?,
                                sequence: row.get(6)?,
                                compromise_from: row.get(7)?,
                                rotation_cbor: row.get(8)?,
                            })
                        },
                    )
                    .optional()?;
                if let Some(key) = key {
                    if !keys.iter().any(|existing: &RelayKey| {
                        existing.relay == key.relay && existing.key_id == key.key_id
                    }) {
                        keys.push(key);
                    }
                }
            }
            Ok((publishing_state, keys, relay_claims))
        })
        .map_err(AppError::Database)
    }

    pub fn export_identity_backup(
        &self,
        path: PathBuf,
        password: &str,
    ) -> Result<BackupExportResult> {
        Self::validate_password(password)?;
        if path.exists() || path.file_name().is_none() {
            return Err(AppError::Validation(
                "Choose a new file path for the identity backup".into(),
            ));
        }
        let stored = self.identity.get_identity()?.ok_or_else(|| {
            AppError::IdentityNotFound("No identity is available to back up".into())
        })?;
        let keys = CryptoService::decrypt_keys(&stored.private_key_encrypted, password)?;
        let archived_identity = ArchivedIdentity {
            peer_id: stored.peer_id.clone(),
            public_key: stored.public_key.clone(),
            x25519_public: stored.x25519_public.clone(),
            display_name: stored.display_name.clone(),
            avatar_hash: stored.avatar_hash.clone(),
            bio: stored.bio.clone(),
            passphrase_hint: stored.passphrase_hint.clone(),
            created_at: stored.created_at,
            updated_at: stored.updated_at,
        };
        Self::validate_identity(&archived_identity, &keys)?;
        let active = self
            .accounts
            .get_active_account()?
            .ok_or_else(|| AppError::InvalidData("No active account is registered".into()))?;
        if active.peer_id != stored.peer_id {
            return Err(AppError::InvalidData(
                "The active account does not match the open identity".into(),
            ));
        }
        let db_path = self
            .accounts
            .get_account_data_path(&active.id)?
            .join("harbor.db");
        let db = Database::new(db_path)?;
        let (publishing_state, relay_keys, relay_claims) = Self::continuity(&db, &stored.peer_id)?;
        let archive = IdentityArchiveV1 {
            archive_domain: "harbor/identity-backup".into(),
            archive_version: ARCHIVE_VERSION,
            identity: archived_identity,
            keys,
            publishing_state,
            relay_keys,
            relay_claims,
        };
        let bytes = Self::encrypt_archive(&archive, password)?;
        Self::atomic_write_new(&path, &bytes)?;
        Ok(BackupExportResult {
            peer_id: stored.peer_id,
            path: path.to_string_lossy().into_owned(),
            created_at: chrono::Utc::now().timestamp(),
            bytes_written: bytes.len() as u64,
        })
    }

    pub fn restore_identity_backup(
        &self,
        path: &Path,
        password: &str,
    ) -> Result<BackupRestoreResult> {
        let bytes = Self::read_bounded(path)?;
        let archive = Self::decrypt_archive(&bytes, password)?;
        if self
            .accounts
            .get_account(&archive.identity.peer_id)?
            .is_some()
        {
            return Err(AppError::AlreadyExists(
                "This identity is already registered on this device".into(),
            ));
        }
        let relative = format!("profiles/{}", archive.identity.peer_id);
        let final_dir = self.accounts.installation_root().join(&relative);
        if final_dir.exists() {
            return Err(AppError::AlreadyExists(
                "A profile directory already exists for this identity".into(),
            ));
        }
        let staging = self
            .accounts
            .installation_root()
            .join(format!(".restore-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&staging)?;
        let restore = (|| -> Result<AccountInfo> {
            let db = Database::new(staging.join("harbor.db"))?;
            let encrypted_keys = CryptoService::encrypt_keys(
                &archive.keys.ed25519_private,
                &archive.keys.x25519_private,
                password,
            )?;
            let identity = LocalIdentity {
                peer_id: archive.identity.peer_id.clone(),
                public_key: archive.identity.public_key.clone(),
                x25519_public: archive.identity.x25519_public.clone(),
                private_key_encrypted: encrypted_keys,
                display_name: archive.identity.display_name.clone(),
                avatar_hash: archive.identity.avatar_hash.clone(),
                bio: archive.identity.bio.clone(),
                passphrase_hint: archive.identity.passphrase_hint.clone(),
                created_at: archive.identity.created_at,
                updated_at: archive.identity.updated_at,
            };
            IdentityRepository::new(&db).create(&identity)?;
            Self::restore_continuity(&db, &archive)?;
            let verifier =
                IdentityService::new(Arc::new(Database::new(staging.join("harbor.db"))?));
            verifier.unlock(password)?;
            let verified = verified_name_claim(
                &RelayNamesRepository::new(&db),
                &identity.peer_id,
                chrono::Utc::now().timestamp(),
            )
            .map_err(|error| {
                AppError::InvalidData(format!("Restored relay name is invalid: {error}"))
            })?;
            drop(verifier);
            drop(db);
            #[cfg(test)]
            if self
                .fail_restore_after_staging
                .swap(false, Ordering::SeqCst)
            {
                return Err(AppError::Internal(
                    "injected restore failure after staging writes".into(),
                ));
            }
            fs::create_dir_all(final_dir.parent().unwrap())?;
            fs::rename(&staging, &final_dir)?;
            let now = chrono::Utc::now().timestamp();
            let account = AccountInfo {
                id: identity.peer_id.clone(),
                display_name: identity.display_name,
                verified_qualified_name: verified
                    .as_ref()
                    .map(|(_, claim)| claim.qualified_name.to_string()),
                verified_name_not_after: verified.as_ref().map(|(_, claim)| claim.not_after),
                avatar_hash: identity.avatar_hash,
                bio: identity.bio,
                peer_id: identity.peer_id,
                created_at: identity.created_at,
                last_accessed_at: Some(now),
                data_path: relative,
            };
            match self.accounts.register_restored_account(account) {
                Ok(account) => Ok(account),
                Err(error) => {
                    let _ = fs::remove_dir_all(&final_dir);
                    Err(error)
                }
            }
        })();
        if restore.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        Ok(BackupRestoreResult {
            account: restore?,
            restart_required: true,
        })
    }

    fn restore_continuity(db: &Database, archive: &IdentityArchiveV1) -> Result<()> {
        db.with_connection_mut(|connection| {
            let transaction = connection.transaction()?;
            if let Some(state) = &archive.publishing_state {
                transaction.execute(
                    "INSERT OR REPLACE INTO identity_publishing_state(peer_id, mode, updated_at) VALUES (?1, ?2, ?3)",
                    rusqlite::params![archive.identity.peer_id, state.mode, state.updated_at],
                )?;
            }
            for key in &archive.relay_keys {
                transaction.execute(
                    "INSERT INTO relay_trust_keys(
                        relay, key_id, public_key, not_before, not_after, retired_at,
                        sequence, compromise_from, rotation_cbor
                     ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                    rusqlite::params![key.relay,key.key_id,key.public_key,key.not_before,key.not_after,key.retired_at,key.sequence,key.compromise_from,key.rotation_cbor],
                )?;
            }
            for claim in &archive.relay_claims {
                transaction.execute(
                    "INSERT INTO relay_name_claims(qualified_name,local_name,relay,peer_id,sequence,claim_cbor,not_before,not_after,relay_key_id,status,verified_at,retired_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                    rusqlite::params![claim.qualified_name,claim.local_name,claim.relay,claim.peer_id,claim.sequence,claim.claim_cbor,claim.not_before,claim.not_after,claim.relay_key_id,claim.status,claim.verified_at,claim.retired_at],
                )?;
            }
            transaction.commit()
        })?;
        Ok(())
    }

    pub fn authenticate_account(&self, account_id: &str, password: &str) -> Result<AccountInfo> {
        Self::validate_password(password)?;
        let account = self
            .accounts
            .get_account(account_id)?
            .ok_or_else(|| AppError::NotFound(format!("Account {account_id} not found")))?;
        let database_path = self
            .accounts
            .get_account_data_path(account_id)?
            .join("harbor.db");
        let connection = rusqlite::Connection::open_with_flags(
            &database_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let stored = connection.query_row(
            "SELECT peer_id, public_key, x25519_public, private_key_encrypted,
                    display_name, avatar_hash, bio, passphrase_hint, created_at, updated_at
             FROM local_identity WHERE id = 1",
            [],
            |row| {
                Ok(LocalIdentity {
                    peer_id: row.get(0)?,
                    public_key: row.get(1)?,
                    x25519_public: row.get(2)?,
                    private_key_encrypted: row.get(3)?,
                    display_name: row.get(4)?,
                    avatar_hash: row.get(5)?,
                    bio: row.get(6)?,
                    passphrase_hint: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )?;
        let keys = CryptoService::decrypt_keys(&stored.private_key_encrypted, password)?;
        let archived = ArchivedIdentity {
            peer_id: stored.peer_id,
            public_key: stored.public_key,
            x25519_public: stored.x25519_public,
            display_name: stored.display_name,
            avatar_hash: stored.avatar_hash,
            bio: stored.bio,
            passphrase_hint: stored.passphrase_hint,
            created_at: stored.created_at,
            updated_at: stored.updated_at,
        };
        Self::validate_identity(&archived, &keys)?;
        if archived.peer_id != account.peer_id {
            return Err(AppError::InvalidData(
                "Account registry does not match the authenticated identity".into(),
            ));
        }
        Ok(account)
    }

    pub fn schedule_profile_deletion(
        &self,
        account: &AccountInfo,
    ) -> Result<DeleteAccountProfileResult> {
        let journal_dir = self.accounts.installation_root().join(DELETE_JOURNAL_DIR);
        fs::create_dir_all(&journal_dir)?;
        let journal_path = journal_dir.join(format!("{}.json", uuid::Uuid::new_v4()));
        let journal = DeletionJournal {
            version: 1,
            account_id: account.id.clone(),
            data_path: account.data_path.clone(),
            scheduled_at: chrono::Utc::now().timestamp(),
        };
        let encoded = serde_json::to_vec(&journal)
            .map_err(|error| AppError::Serialization(error.to_string()))?;
        Self::atomic_write_new(&journal_path, &encoded)?;
        #[cfg(test)]
        if self.fail_delete_after_journal.swap(false, Ordering::SeqCst) {
            return Err(AppError::Internal(
                "injected account deletion registry failure".into(),
            ));
        }
        let next_account_id = self.accounts.unregister_for_profile_deletion(&account.id)?;
        Ok(DeleteAccountProfileResult {
            restart_required: true,
            next_account_id,
        })
    }

    pub fn reconcile_pending_deletions(accounts: &AccountsService) -> Result<()> {
        let journal_dir = accounts.installation_root().join(DELETE_JOURNAL_DIR);
        if !journal_dir.exists() {
            return Ok(());
        }
        let registry = accounts.load_registry()?;
        for entry in fs::read_dir(&journal_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let bytes = Self::read_bounded(&entry.path())?;
            let journal: DeletionJournal = serde_json::from_slice(&bytes)
                .map_err(|_| AppError::InvalidData("Account deletion journal is invalid".into()))?;
            if journal.version != 1 {
                return Err(AppError::InvalidData(
                    "Unsupported account deletion journal".into(),
                ));
            }
            if registry.accounts.contains_key(&journal.account_id) {
                // The registry commit did not happen. The account remains authoritative.
                fs::remove_file(entry.path())?;
                continue;
            }
            Self::delete_contained_profile(accounts.installation_root(), &journal.data_path)?;
            fs::remove_file(entry.path())?;
        }
        if fs::read_dir(&journal_dir)?.next().is_none() {
            fs::remove_dir(&journal_dir)?;
        }
        Ok(())
    }

    fn delete_contained_profile(root: &Path, data_path: &str) -> Result<()> {
        if data_path == "default" {
            for name in ["harbor.db", "harbor.db-wal", "harbor.db-shm"] {
                let path = root.join(name);
                if path.exists() {
                    fs::remove_file(path)?;
                }
            }
            for name in ["media", "webview", "logs"] {
                let path = root.join(name);
                if path.exists() {
                    fs::remove_dir_all(path)?;
                }
            }
            return Ok(());
        }
        let relative = Path::new(data_path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(AppError::InvalidData(
                "Deletion profile path is not contained".into(),
            ));
        }
        let path = root.join(relative);
        fs::create_dir_all(root)?;
        let canonical_root = root.canonicalize()?;
        if path.exists() && !path.canonicalize()?.starts_with(&canonical_root) {
            return Err(AppError::InvalidData(
                "Deletion profile path escapes the installation root".into(),
            ));
        }
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
        Ok(())
    }

    fn read_bounded(path: &Path) -> Result<Vec<u8>> {
        let metadata = fs::metadata(path)?;
        if !metadata.is_file() || metadata.len() > MAX_ARCHIVE_BYTES as u64 {
            return Err(AppError::InvalidData(
                "Identity backup has an invalid size".into(),
            ));
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        File::open(path)?
            .take((MAX_ARCHIVE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_ARCHIVE_BYTES {
            return Err(AppError::InvalidData("Identity backup is too large".into()));
        }
        Ok(bytes)
    }

    fn atomic_write_new(path: &Path, bytes: &[u8]) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| AppError::Validation("File path has no parent directory".into()))?;
        fs::create_dir_all(parent)?;
        let temp = parent.join(format!(".harbor-write-{}", uuid::Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&temp);
            return Err(error.into());
        }
        drop(file);
        if path.exists() {
            let _ = fs::remove_file(&temp);
            return Err(AppError::AlreadyExists(
                "Destination file already exists".into(),
            ));
        }
        if let Err(error) = fs::rename(&temp, path) {
            let _ = fs::remove_file(&temp);
            return Err(error.into());
        }
        #[cfg(unix)]
        {
            File::open(parent)?.sync_all()?;
        }
        Ok(())
    }
}

use rusqlite::OptionalExtension;
use std::str::FromStr;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateIdentityRequest;

    const PASSWORD: &str = "correct-password";

    struct TestProfile {
        root: tempfile::TempDir,
        accounts: Arc<AccountsService>,
        identity: Arc<IdentityService>,
        backup: AccountBackupService,
        peer_id: String,
    }

    fn populated_profile() -> TestProfile {
        let root = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::new(root.path().join("harbor.db")).unwrap());
        let identity = Arc::new(IdentityService::new(db));
        let created = identity
            .create_identity(CreateIdentityRequest {
                display_name: "Alice".into(),
                passphrase: PASSWORD.into(),
                bio: Some("Backup test".into()),
                passphrase_hint: Some("test hint".into()),
            })
            .unwrap();
        let accounts = Arc::new(AccountsService::new(root.path().to_path_buf()));
        accounts
            .register_account(
                created.peer_id.clone(),
                created.display_name.clone(),
                created.bio.clone(),
                created.avatar_hash.clone(),
            )
            .unwrap();
        let backup = AccountBackupService::new(accounts.clone(), identity.clone());
        TestProfile {
            root,
            accounts,
            identity,
            backup,
            peer_id: created.peer_id,
        }
    }

    fn empty_target() -> (
        tempfile::TempDir,
        Arc<AccountsService>,
        AccountBackupService,
    ) {
        let root = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::new(root.path().join("harbor.db")).unwrap());
        let identity = Arc::new(IdentityService::new(db));
        let accounts = Arc::new(AccountsService::new(root.path().to_path_buf()));
        let backup = AccountBackupService::new(accounts.clone(), identity);
        (root, accounts, backup)
    }

    #[test]
    fn encrypted_backup_roundtrip_restores_key_and_registry_continuity() {
        let source = populated_profile();
        let export_path = source.root.path().join("alice.harbor-identity");
        let exported = source
            .backup
            .export_identity_backup(export_path.clone(), PASSWORD)
            .unwrap();
        assert_eq!(exported.peer_id, source.peer_id);
        assert_eq!(
            exported.bytes_written,
            fs::metadata(&export_path).unwrap().len()
        );

        let (target_root, target_accounts, target) = empty_target();
        let restored = target
            .restore_identity_backup(&export_path, PASSWORD)
            .unwrap();
        assert!(restored.restart_required);
        assert_eq!(restored.account.peer_id, source.peer_id);
        assert_eq!(
            target_accounts.get_active_account().unwrap().unwrap().id,
            source.peer_id
        );
        let restored_db = Arc::new(
            Database::new(
                target_root
                    .path()
                    .join(&restored.account.data_path)
                    .join("harbor.db"),
            )
            .unwrap(),
        );
        let restored_identity = IdentityService::new(restored_db);
        let info = restored_identity.unlock(PASSWORD).unwrap();
        assert_eq!(info.peer_id, source.peer_id);
        assert_eq!(info.display_name, "Alice");
    }

    #[test]
    fn wrong_password_and_tamper_are_rejected_before_restore_writes() {
        let source = populated_profile();
        let export_path = source.root.path().join("alice.harbor-identity");
        source
            .backup
            .export_identity_backup(export_path.clone(), PASSWORD)
            .unwrap();
        let (target_root, target_accounts, target) = empty_target();

        let wrong = target
            .restore_identity_backup(&export_path, "incorrect-password")
            .unwrap_err();
        assert!(matches!(wrong, AppError::IdentityInvalidPassphrase(_)));
        assert!(target_accounts.list_accounts().unwrap().is_empty());

        let mut tampered = fs::read(&export_path).unwrap();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x40;
        let tampered_path = source.root.path().join("tampered.harbor-identity");
        fs::write(&tampered_path, tampered).unwrap();
        assert!(target
            .restore_identity_backup(&tampered_path, PASSWORD)
            .is_err());
        assert!(target_accounts.list_accounts().unwrap().is_empty());
        assert!(!fs::read_dir(target_root.path()).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".restore-")));
    }

    #[test]
    fn duplicate_restore_is_rejected_without_replacing_the_profile() {
        let source = populated_profile();
        let export_path = source.root.path().join("alice.harbor-identity");
        source
            .backup
            .export_identity_backup(export_path.clone(), PASSWORD)
            .unwrap();
        let (_target_root, target_accounts, target) = empty_target();
        target
            .restore_identity_backup(&export_path, PASSWORD)
            .unwrap();
        let duplicate = target
            .restore_identity_backup(&export_path, PASSWORD)
            .unwrap_err();
        assert!(matches!(duplicate, AppError::AlreadyExists(_)));
        assert_eq!(target_accounts.list_accounts().unwrap().len(), 1);
    }

    #[test]
    fn restore_into_existing_installation_stays_inactive_until_runtime_handoff() {
        let source = populated_profile();
        let export_path = source.root.path().join("alice.harbor-identity");
        source
            .backup
            .export_identity_backup(export_path.clone(), PASSWORD)
            .unwrap();

        let existing = populated_profile();
        let previous_id = existing.peer_id.clone();
        let restored = existing
            .backup
            .restore_identity_backup(&export_path, PASSWORD)
            .unwrap();

        assert_eq!(
            existing.accounts.get_active_account().unwrap().unwrap().id,
            previous_id
        );
        assert!(existing.identity.is_unlocked());
        assert!(existing
            .accounts
            .get_account(&restored.account.id)
            .unwrap()
            .is_some());

        // This is the ordering enforced by the command after all archive work
        // succeeds: stop/lock first, then commit the active registry selection.
        existing.identity.lock();
        existing
            .accounts
            .set_active_account(&restored.account.id)
            .unwrap();
        assert!(!existing.identity.is_unlocked());
        assert_eq!(
            existing.accounts.get_active_account().unwrap().unwrap().id,
            restored.account.id
        );
    }

    #[test]
    fn failed_restore_preserves_existing_active_registry_and_runtime() {
        let source = populated_profile();
        let export_path = source.root.path().join("alice.harbor-identity");
        source
            .backup
            .export_identity_backup(export_path.clone(), PASSWORD)
            .unwrap();
        let existing = populated_profile();
        let previous_id = existing.peer_id.clone();

        assert!(existing
            .backup
            .restore_identity_backup(&export_path, "incorrect-password")
            .is_err());
        assert!(existing.identity.is_unlocked());
        assert_eq!(
            existing.accounts.get_active_account().unwrap().unwrap().id,
            previous_id
        );
        assert_eq!(existing.accounts.list_accounts().unwrap().len(), 1);
    }

    #[test]
    fn injected_restore_failure_after_staging_rolls_back_every_write() {
        let source = populated_profile();
        let export_path = source.root.path().join("alice.harbor-identity");
        source
            .backup
            .export_identity_backup(export_path.clone(), PASSWORD)
            .unwrap();
        let (target_root, target_accounts, target) = empty_target();
        target.inject_restore_failure_after_staging();

        let error = target
            .restore_identity_backup(&export_path, PASSWORD)
            .unwrap_err();
        assert!(error.to_string().contains("injected restore failure"));
        assert!(target_accounts.list_accounts().unwrap().is_empty());
        assert!(!target_root
            .path()
            .join("profiles")
            .join(&source.peer_id)
            .exists());
        assert!(!fs::read_dir(target_root.path()).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".restore-")));
    }

    #[test]
    fn interrupted_deletion_before_registry_commit_rolls_back() {
        let profile = populated_profile();
        let account = profile.accounts.get_active_account().unwrap().unwrap();
        let directory = profile.root.path().join(DELETE_JOURNAL_DIR);
        fs::create_dir_all(&directory).unwrap();
        let journal = DeletionJournal {
            version: 1,
            account_id: account.id,
            data_path: account.data_path,
            scheduled_at: 1,
        };
        AccountBackupService::atomic_write_new(
            &directory.join("interrupted.json"),
            &serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();

        AccountBackupService::reconcile_pending_deletions(&profile.accounts).unwrap();
        assert!(profile.root.path().join("harbor.db").exists());
        assert_eq!(profile.accounts.list_accounts().unwrap().len(), 1);
        assert!(!directory.exists());
    }

    #[test]
    fn injected_delete_registry_failure_reconciles_to_preserve_account_and_data() {
        let profile = populated_profile();
        let account = profile.accounts.get_active_account().unwrap().unwrap();
        profile.backup.inject_delete_failure_after_journal();

        let error = profile
            .backup
            .schedule_profile_deletion(&account)
            .unwrap_err();
        assert!(error.to_string().contains("injected account deletion"));
        assert!(profile.accounts.get_account(&account.id).unwrap().is_some());
        assert!(profile.root.path().join("harbor.db").exists());

        AccountBackupService::reconcile_pending_deletions(&profile.accounts).unwrap();
        assert!(profile.accounts.get_account(&account.id).unwrap().is_some());
        assert!(profile.root.path().join("harbor.db").exists());
        assert!(!profile.root.path().join(DELETE_JOURNAL_DIR).exists());
    }

    #[test]
    fn deleting_default_profile_preserves_registry_and_sibling_profiles() {
        let TestProfile {
            root,
            accounts,
            identity,
            backup,
            ..
        } = populated_profile();
        let sibling = root.path().join("profiles/sibling");
        fs::create_dir_all(&sibling).unwrap();
        fs::write(sibling.join("keep.txt"), b"sibling data").unwrap();
        fs::write(root.path().join("keep-root.txt"), b"installation data").unwrap();
        let account = accounts.get_active_account().unwrap().unwrap();

        let result = backup.schedule_profile_deletion(&account).unwrap();
        assert!(result.restart_required);
        drop(backup);
        drop(identity);
        AccountBackupService::reconcile_pending_deletions(&accounts).unwrap();

        assert!(!root.path().join("harbor.db").exists());
        assert!(root.path().join("accounts.json").exists());
        assert!(root.path().join("keep-root.txt").exists());
        assert!(sibling.join("keep.txt").exists());
    }

    #[test]
    fn deleting_one_isolated_profile_selects_and_preserves_the_survivor() {
        let source = populated_profile();
        let export_path = source.root.path().join("alice.harbor-identity");
        source
            .backup
            .export_identity_backup(export_path.clone(), PASSWORD)
            .unwrap();
        let existing = populated_profile();
        let survivor_id = existing.peer_id.clone();
        let restored = existing
            .backup
            .restore_identity_backup(&export_path, PASSWORD)
            .unwrap();
        let restored_path = existing.root.path().join(&restored.account.data_path);
        let untouched_sibling = existing.root.path().join("profiles/untouched-profile");
        fs::create_dir_all(&untouched_sibling).unwrap();
        fs::write(untouched_sibling.join("keep.txt"), b"keep").unwrap();
        existing
            .accounts
            .set_active_account(&restored.account.id)
            .unwrap();

        let scheduled = existing
            .backup
            .schedule_profile_deletion(&restored.account)
            .unwrap();
        assert_eq!(
            scheduled.next_account_id.as_deref(),
            Some(survivor_id.as_str())
        );
        AccountBackupService::reconcile_pending_deletions(&existing.accounts).unwrap();

        assert!(!restored_path.exists());
        assert!(untouched_sibling.join("keep.txt").exists());
        assert!(existing.root.path().join("harbor.db").exists());
        assert_eq!(
            existing.accounts.get_active_account().unwrap().unwrap().id,
            survivor_id
        );
        assert!(existing
            .accounts
            .get_account(&restored.account.id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn legacy_remove_account_cannot_delete_profile_data() {
        let profile = populated_profile();
        let error = profile
            .accounts
            .remove_account(&profile.peer_id, true)
            .unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
        assert!(profile.root.path().join("harbor.db").exists());
        assert!(profile
            .accounts
            .get_account(&profile.peer_id)
            .unwrap()
            .is_some());
    }

    #[test]
    fn authentication_rejects_wrong_password_without_changing_lock_state() {
        let profile = populated_profile();
        assert!(profile.identity.is_unlocked());
        let error = profile
            .backup
            .authenticate_account(&profile.peer_id, "incorrect-password")
            .unwrap_err();
        assert!(matches!(error, AppError::IdentityInvalidPassphrase(_)));
        assert!(profile.identity.is_unlocked());
        assert!(profile
            .accounts
            .get_account(&profile.peer_id)
            .unwrap()
            .is_some());
    }
}
