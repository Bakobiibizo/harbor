use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, Weak};
use tracing::{info, warn};

static REGISTRY_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> = OnceLock::new();

fn shared_registry_lock(path: &PathBuf) -> Arc<Mutex<()>> {
    let locks = REGISTRY_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = locks.lock().unwrap_or_else(|poisoned| {
        warn!("Accounts registry lock map was poisoned; recovering it");
        poisoned.into_inner()
    });
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(path).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(path.clone(), Arc::downgrade(&lock));
    lock
}

/// Account metadata stored in the accounts registry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    /// Unique identifier for the account (derived from peer_id)
    pub id: String,
    /// User's display name
    pub display_name: String,
    /// Relay-qualified name after cryptographic verification.
    #[serde(default)]
    pub verified_qualified_name: Option<String>,
    /// Expiry of the verified relay claim cached for the locked chooser.
    #[serde(default)]
    pub verified_name_not_after: Option<i64>,
    /// Avatar hash if set
    pub avatar_hash: Option<String>,
    /// Short bio
    pub bio: Option<String>,
    /// Peer ID for this account
    pub peer_id: String,
    /// When the account was created (timestamp)
    pub created_at: i64,
    /// When the account was last accessed (timestamp)
    pub last_accessed_at: Option<i64>,
    /// Path to the account's data directory (relative to app data)
    pub data_path: String,
}

/// Accounts registry stored as JSON
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountsRegistry {
    /// Map of account ID to account info
    pub accounts: HashMap<String, AccountInfo>,
    /// Currently active account ID (if any)
    pub active_account_id: Option<String>,
}

/// Service for managing multiple accounts
pub struct AccountsService {
    /// Path to the registry file
    registry_path: PathBuf,
    /// Base data directory for all accounts
    base_data_dir: PathBuf,
    /// Registry-relative path of the profile whose services are currently open.
    runtime_data_path: String,
    /// Serializes every registry read-modify-write sequence across service clones.
    registry_lock: Arc<Mutex<()>>,
}

impl AccountsService {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let registry_path = app_data_dir.join("accounts.json");
        let registry_lock = shared_registry_lock(&registry_path);
        Self {
            registry_path,
            base_data_dir: app_data_dir,
            runtime_data_path: "default".to_string(),
            registry_lock,
        }
    }

    /// Bind identity registration to the profile whose database is open. This
    /// prevents an identity created in a selected non-default profile from being
    /// recorded as if it lived in the installation root.
    pub fn with_runtime_data_dir(mut self, runtime_data_dir: &Path) -> Result<Self> {
        fs::create_dir_all(&self.base_data_dir)?;
        fs::create_dir_all(runtime_data_dir)?;
        let canonical_base = self.base_data_dir.canonicalize()?;
        let canonical_runtime = runtime_data_dir.canonicalize()?;
        let relative = canonical_runtime
            .strip_prefix(&canonical_base)
            .map_err(|_| {
                AppError::InvalidData(format!(
                    "Runtime profile {} is outside the Harbor data directory",
                    runtime_data_dir.display()
                ))
            })?;

        self.runtime_data_path = if relative.as_os_str().is_empty() {
            "default".to_string()
        } else {
            let components = relative
                .components()
                .map(|component| match component {
                    Component::Normal(value) => value.to_str().ok_or_else(|| {
                        AppError::InvalidData(
                            "Runtime profile path must contain valid Unicode".to_string(),
                        )
                    }),
                    _ => Err(AppError::InvalidData(
                        "Runtime profile path is not contained".to_string(),
                    )),
                })
                .collect::<Result<Vec<_>>>()?;
            components.join("/")
        };
        // Apply the same lexical and symlink containment checks used when the
        // registry is consumed at startup.
        self.resolve_data_path(&self.runtime_data_path)?;
        Ok(self)
    }

    fn lock_registry(&self) -> Result<MutexGuard<'_, ()>> {
        self.registry_lock
            .lock()
            .map_err(|_| AppError::Internal("Accounts registry lock is poisoned".to_string()))
    }

    fn backup_path(&self) -> PathBuf {
        self.registry_path.with_extension("json.backup")
    }

    fn resolve_data_path(&self, data_path: &str) -> Result<PathBuf> {
        if data_path == "default" {
            return Ok(self.base_data_dir.clone());
        }

        let relative = Path::new(data_path);
        if data_path.is_empty()
            || data_path.contains('\\')
            || data_path.contains(':')
            || relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(AppError::InvalidData(format!(
                "Account data path must be a contained relative path: {data_path}"
            )));
        }

        fs::create_dir_all(&self.base_data_dir)?;
        let canonical_base = self.base_data_dir.canonicalize()?;
        let candidate = self.base_data_dir.join(relative);

        // Reject an existing symlink (or a symlink in an existing parent) that
        // resolves outside the installation data root. Missing leaf directories
        // are safe because every lexical component was validated above.
        let mut existing = self.base_data_dir.clone();
        for component in relative.components() {
            let Component::Normal(component) = component else {
                unreachable!("components were validated above")
            };
            existing.push(component);
            if existing.exists() && !existing.canonicalize()?.starts_with(&canonical_base) {
                return Err(AppError::InvalidData(format!(
                    "Account data path escapes the Harbor data directory: {data_path}"
                )));
            }
        }

        Ok(candidate)
    }

    fn identity_peer_id(data_dir: &Path) -> Result<String> {
        let database_path = data_dir.join("harbor.db");
        let connection = rusqlite::Connection::open_with_flags(
            &database_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|error| {
            AppError::InvalidData(format!(
                "Could not open the selected account database {}: {error}",
                database_path.display()
            ))
        })?;
        connection
            .query_row(
                "SELECT peer_id FROM local_identity WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| {
                AppError::InvalidData(format!(
                    "The selected account database {} has no usable identity: {error}",
                    database_path.display()
                ))
            })
    }

    fn validate_identity_owner(account: &AccountInfo, data_dir: &Path) -> Result<()> {
        let actual_peer_id = Self::identity_peer_id(data_dir)?;
        if actual_peer_id != account.peer_id {
            return Err(AppError::InvalidData(format!(
                "Selected account {} does not own database identity {}",
                account.peer_id, actual_peer_id
            )));
        }
        Ok(())
    }

    fn resolve_account_data_dir_unlocked(
        &self,
        registry: &mut AccountsRegistry,
        account_id: &str,
    ) -> Result<(AccountInfo, PathBuf)> {
        let account = registry
            .accounts
            .get(account_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Account {account_id} not found")))?;
        let selected_dir = self.resolve_data_path(&account.data_path)?;
        if selected_dir.join("harbor.db").exists() {
            Self::validate_identity_owner(&account, &selected_dir)?;
            return Ok((account, selected_dir));
        }

        // Earlier builds sometimes registered the root identity under a
        // profile-<peer> path that was never populated. The peer ID stored in
        // the root database is authoritative, so it remains safe to repair the
        // matching account even if stale test/profile entries also exist.
        if account.data_path != "default" {
            let legacy_dir = self.base_data_dir.clone();
            if legacy_dir.join("harbor.db").exists()
                && Self::validate_identity_owner(&account, &legacy_dir).is_ok()
            {
                if let Some(conflicting_account) = registry.accounts.values().find(|candidate| {
                    candidate.id != account.id && candidate.data_path == "default"
                }) {
                    return Err(AppError::InvalidData(format!(
                        "The root account database matches {}, but is already assigned to account {}",
                        account.id, conflicting_account.id
                    )));
                }

                let repaired = registry
                    .accounts
                    .get_mut(account_id)
                    .expect("account was cloned from this registry");
                repaired.data_path = "default".to_string();
                let repaired = repaired.clone();
                self.save_registry_unlocked(registry)?;
                info!(
                    "Repaired legacy account data path for account {}",
                    account_id
                );
                return Ok((repaired, legacy_dir));
            }
        }

        Err(AppError::InvalidData(format!(
            "Selected account {} has no database at {}",
            account.peer_id,
            selected_dir.join("harbor.db").display()
        )))
    }

    /// Resolve the active account's runtime directory before opening any profile
    /// database or service. A registry entry can never select data outside this
    /// service's installation root, and an existing database must belong to the
    /// peer recorded in the registry.
    pub fn resolve_active_data_dir(&self) -> Result<PathBuf> {
        let _guard = self.lock_registry()?;
        let mut registry = self.load_registry_unlocked()?;
        let Some(active_id) = registry.active_account_id.clone() else {
            return Ok(self.base_data_dir.clone());
        };
        if !registry.accounts.contains_key(&active_id) {
            return Err(AppError::InvalidData(format!(
                "Active account {active_id} does not exist"
            )));
        }
        self.resolve_account_data_dir_unlocked(&mut registry, &active_id)
            .map(|(_, data_dir)| data_dir)
    }

    fn load_registry_unlocked(&self) -> Result<AccountsRegistry> {
        let path = if self.registry_path.exists() {
            &self.registry_path
        } else {
            let backup_path = self.backup_path();
            if !backup_path.exists() {
                return Ok(AccountsRegistry::default());
            }
            return Self::read_registry_file(&backup_path);
        };

        Self::read_registry_file(path)
    }

    fn read_registry_file(path: &PathBuf) -> Result<AccountsRegistry> {
        let content = fs::read_to_string(path)?;
        let registry: AccountsRegistry = serde_json::from_str(&content).map_err(|e| {
            AppError::Serialization(format!(
                "Failed to parse accounts registry {}: {}",
                path.display(),
                e
            ))
        })?;
        if registry
            .active_account_id
            .as_ref()
            .is_some_and(|id| !registry.accounts.contains_key(id))
        {
            return Err(AppError::InvalidData(
                "Accounts registry references an active account that does not exist".to_string(),
            ));
        }
        Ok(registry)
    }

    /// Load the accounts registry from disk
    pub fn load_registry(&self) -> Result<AccountsRegistry> {
        let _guard = self.lock_registry()?;
        self.load_registry_unlocked()
    }

    /// Save the accounts registry to disk
    pub fn save_registry(&self, registry: &AccountsRegistry) -> Result<()> {
        let _guard = self.lock_registry()?;
        self.save_registry_unlocked(registry)
    }

    fn save_registry_unlocked(&self, registry: &AccountsRegistry) -> Result<()> {
        self.save_registry_unlocked_with(registry, |file, content| file.write_all(content))
    }

    fn save_registry_unlocked_with<F>(&self, registry: &AccountsRegistry, writer: F) -> Result<()>
    where
        F: FnOnce(&mut File, &[u8]) -> std::io::Result<()>,
    {
        // Ensure parent directory exists
        if let Some(parent) = self.registry_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(registry)
            .map_err(|e| AppError::Serialization(format!("Failed to serialize registry: {}", e)))?;

        let temp_path = self.registry_path.with_extension(format!(
            "json.tmp-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let mut temp = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;

        if let Err(error) = writer(&mut temp, content.as_bytes()).and_then(|_| temp.sync_all()) {
            drop(temp);
            let _ = fs::remove_file(&temp_path);
            return Err(error.into());
        }
        drop(temp);

        if let Err(error) = self.replace_registry_file(&temp_path) {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }

        #[cfg(unix)]
        if let Some(parent) = self.registry_path.parent() {
            // The rename above is the commit point. A directory sync failure cannot
            // be rolled back safely, so report it operationally without telling the
            // caller that the registry mutation failed.
            if let Err(error) = File::open(parent).and_then(|directory| directory.sync_all()) {
                warn!(
                    "Accounts registry committed but parent directory sync failed: {}",
                    error
                );
            }
        }

        Ok(())
    }

    #[cfg(not(windows))]
    fn replace_registry_file(&self, temp_path: &PathBuf) -> Result<()> {
        fs::rename(temp_path, &self.registry_path)?;
        Ok(())
    }

    /// Windows does not replace an existing destination with `std::fs::rename`.
    /// Keep a recoverable previous generation while swapping the new registry in.
    #[cfg(windows)]
    fn replace_registry_file(&self, temp_path: &PathBuf) -> Result<()> {
        let backup_path = self.backup_path();
        if backup_path.exists() {
            fs::remove_file(&backup_path)?;
        }
        let had_registry = self.registry_path.exists();
        if had_registry {
            fs::rename(&self.registry_path, &backup_path)?;
        }
        if let Err(error) = fs::rename(temp_path, &self.registry_path) {
            if had_registry {
                let _ = fs::rename(&backup_path, &self.registry_path);
            }
            return Err(error.into());
        }
        if had_registry {
            if let Err(error) = fs::remove_file(backup_path) {
                warn!(
                    "Accounts registry committed but the previous backup could not be removed: {}",
                    error
                );
            }
        }
        Ok(())
    }

    /// List all registered accounts
    pub fn list_accounts(&self) -> Result<Vec<AccountInfo>> {
        let _guard = self.lock_registry()?;
        let registry = self.load_registry_unlocked()?;
        let mut accounts: Vec<AccountInfo> = registry.accounts.values().cloned().collect();

        // Sort by last accessed (most recent first), then by created_at, then by id for stability
        accounts.sort_by(|a, b| {
            match (a.last_accessed_at, b.last_accessed_at) {
                (Some(a_time), Some(b_time)) => b_time.cmp(&a_time),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => b.created_at.cmp(&a.created_at),
            }
            .then_with(|| a.id.cmp(&b.id))
        });

        Ok(accounts)
    }

    /// Get account by ID
    pub fn get_account(&self, account_id: &str) -> Result<Option<AccountInfo>> {
        let _guard = self.lock_registry()?;
        let registry = self.load_registry_unlocked()?;
        Ok(registry.accounts.get(account_id).cloned())
    }

    /// Get the currently active account
    pub fn get_active_account(&self) -> Result<Option<AccountInfo>> {
        let _guard = self.lock_registry()?;
        let registry = self.load_registry_unlocked()?;
        if let Some(active_id) = &registry.active_account_id {
            Ok(registry.accounts.get(active_id).cloned())
        } else {
            Ok(None)
        }
    }

    /// Register a new account in the registry
    pub fn register_account(
        &self,
        peer_id: String,
        display_name: String,
        bio: Option<String>,
        avatar_hash: Option<String>,
    ) -> Result<AccountInfo> {
        let _guard = self.lock_registry()?;
        let mut registry = self.load_registry_unlocked()?;

        // Use full peer_id as account ID for uniqueness
        let account_id = peer_id.clone();

        // A matching entry can remain after an interrupted earlier identity creation.
        // Treat it as an idempotent recovery, while still persisting the active account.
        if let Some(account) = registry.accounts.get_mut(&account_id) {
            account.display_name = display_name;
            account.bio = bio;
            account.avatar_hash = avatar_hash;
            account.last_accessed_at = Some(chrono::Utc::now().timestamp());
            let recovered = account.clone();
            registry.active_account_id = Some(account_id);
            self.save_registry_unlocked(&registry)?;
            return Ok(recovered);
        }

        let now = chrono::Utc::now().timestamp();

        // Identity creation happens inside the currently opened runtime profile.
        // The first/default profile is the installation root, represented by this
        // stable sentinel rather than a directory that has not been populated.
        let data_path = self.runtime_data_path.clone();

        let account = AccountInfo {
            id: account_id.clone(),
            display_name,
            verified_qualified_name: None,
            verified_name_not_after: None,
            avatar_hash,
            bio,
            peer_id,
            created_at: now,
            last_accessed_at: Some(now),
            data_path,
        };

        registry
            .accounts
            .insert(account_id.clone(), account.clone());
        registry.active_account_id = Some(account_id);

        self.save_registry_unlocked(&registry)?;

        info!("Registered new account: {}", account.id);
        Ok(account)
    }

    /// Update account metadata (display name, bio, avatar)
    pub fn update_account(
        &self,
        account_id: &str,
        display_name: Option<String>,
        bio: Option<Option<String>>,
        avatar_hash: Option<Option<String>>,
    ) -> Result<AccountInfo> {
        let _guard = self.lock_registry()?;
        let mut registry = self.load_registry_unlocked()?;

        let account = registry
            .accounts
            .get_mut(account_id)
            .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))?;

        if let Some(name) = display_name {
            account.display_name = name;
        }
        if let Some(new_bio) = bio {
            account.bio = new_bio;
        }
        if let Some(new_avatar) = avatar_hash {
            account.avatar_hash = new_avatar;
        }

        let updated = account.clone();
        self.save_registry_unlocked(&registry)?;

        Ok(updated)
    }

    /// Persist the public relay-qualified name used by the locked account chooser.
    /// Missing registry entries are valid for legacy/single-profile installations.
    pub fn update_verified_qualified_name(
        &self,
        peer_id: &str,
        qualified_name: &str,
        not_after: i64,
    ) -> Result<()> {
        let _guard = self.lock_registry()?;
        let mut registry = self.load_registry_unlocked()?;
        let Some(account) = registry.accounts.get_mut(peer_id) else {
            return Ok(());
        };
        account.verified_qualified_name = Some(qualified_name.to_string());
        account.verified_name_not_after = Some(not_after);
        self.save_registry_unlocked(&registry)
    }

    /// Set the active account and update last_accessed_at
    pub fn set_active_account(&self, account_id: &str) -> Result<AccountInfo> {
        let _guard = self.lock_registry()?;
        let mut registry = self.load_registry_unlocked()?;

        if !registry.accounts.contains_key(account_id) {
            return Err(AppError::NotFound(format!(
                "Account {} not found",
                account_id
            )));
        }

        // Update last accessed
        if let Some(account) = registry.accounts.get_mut(account_id) {
            account.last_accessed_at = Some(chrono::Utc::now().timestamp());
        }

        registry.active_account_id = Some(account_id.to_string());
        self.save_registry_unlocked(&registry)?;

        registry.accounts.get(account_id).cloned().ok_or_else(|| {
            AppError::Internal(format!(
                "Account {} disappeared from registry after existence check",
                account_id
            ))
        })
    }

    /// Remove an account from the registry
    pub fn remove_account(&self, account_id: &str, delete_data: bool) -> Result<()> {
        if delete_data {
            return Err(AppError::Validation(
                "Direct profile deletion is disabled; use delete_account_profile so the deletion is authenticated and recoverable"
                    .to_string(),
            ));
        }
        let _guard = self.lock_registry()?;
        let mut registry = self.load_registry_unlocked()?;

        registry
            .accounts
            .remove(account_id)
            .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))?;

        // Clear active account if it was this one
        if registry.active_account_id.as_deref() == Some(account_id) {
            registry.active_account_id = None;
        }

        self.save_registry_unlocked(&registry)?;

        info!("Removed account: {}", account_id);
        Ok(())
    }

    /// Get the data directory path for an account
    pub fn get_account_data_path(&self, account_id: &str) -> Result<PathBuf> {
        let _guard = self.lock_registry()?;
        let registry = self.load_registry_unlocked()?;

        let account = registry
            .accounts
            .get(account_id)
            .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))?;

        self.resolve_data_path(&account.data_path)
    }

    /// Validate that an account can be opened before the current runtime is
    /// stopped for a profile switch. This includes both path containment and
    /// proof that the selected database belongs to the registry peer.
    pub fn validate_account_runtime(&self, account_id: &str) -> Result<AccountInfo> {
        let _guard = self.lock_registry()?;
        let mut registry = self.load_registry_unlocked()?;
        self.resolve_account_data_dir_unlocked(&mut registry, account_id)
            .map(|(account, _)| account)
    }

    /// Check if any accounts exist
    pub fn has_accounts(&self) -> Result<bool> {
        let _guard = self.lock_registry()?;
        let registry = self.load_registry_unlocked()?;
        Ok(!registry.accounts.is_empty())
    }

    /// Installation root containing the durable account registry and all
    /// contained profile directories. Backup/recovery code uses this rather
    /// than accepting a renderer-provided root.
    pub(crate) fn installation_root(&self) -> &Path {
        &self.base_data_dir
    }

    /// Register a restored profile at an explicitly contained path. Unlike
    /// ordinary identity creation, restoration builds the profile before it is
    /// selected, so it must not inherit the currently-open runtime path.
    pub(crate) fn register_restored_account(&self, account: AccountInfo) -> Result<AccountInfo> {
        let _guard = self.lock_registry()?;
        let mut registry = self.load_registry_unlocked()?;
        if registry.accounts.contains_key(&account.id) {
            return Err(AppError::AlreadyExists(format!(
                "Account {} is already registered",
                account.id
            )));
        }
        self.resolve_data_path(&account.data_path)?;
        registry
            .accounts
            .insert(account.id.clone(), account.clone());
        // A restore into an installation that is already serving a profile is
        // registered inactive. The command layer stops that runtime and locks
        // its keys before selecting the restored profile. A fresh installation
        // has no runtime identity to stop, so it can select immediately.
        if registry.active_account_id.is_none() {
            registry.active_account_id = Some(account.id.clone());
        }
        self.save_registry_unlocked(&registry)?;
        Ok(account)
    }

    /// Remove only the registry record and select a deterministic surviving
    /// account. Physical profile deletion is handled by the durable deletion
    /// journal and never by this registry primitive.
    pub(crate) fn unregister_for_profile_deletion(
        &self,
        account_id: &str,
    ) -> Result<Option<String>> {
        let _guard = self.lock_registry()?;
        let mut registry = self.load_registry_unlocked()?;
        if registry.accounts.remove(account_id).is_none() {
            return Err(AppError::NotFound(format!(
                "Account {} not found",
                account_id
            )));
        }
        let next_account_id = registry
            .accounts
            .values()
            .max_by(|left, right| {
                left.last_accessed_at
                    .cmp(&right.last_accessed_at)
                    .then_with(|| right.id.cmp(&left.id))
            })
            .map(|account| account.id.clone());
        registry.active_account_id = next_account_id.clone();
        self.save_registry_unlocked(&registry)?;
        Ok(next_account_id)
    }

    /// Migrate existing single-account setup to multi-account registry
    /// This is called on app startup to handle existing users
    pub fn migrate_legacy_account(&self, legacy_db_path: &PathBuf) -> Result<Option<AccountInfo>> {
        let _guard = self.lock_registry()?;
        // Check if legacy database exists and we don't have any accounts yet
        if !legacy_db_path.exists() {
            return Ok(None);
        }

        let registry = self.load_registry_unlocked()?;
        if !registry.accounts.is_empty() {
            return Ok(None);
        }

        // Try to read identity info from the legacy database
        use rusqlite::Connection;

        let conn = Connection::open(legacy_db_path).map_err(AppError::Database)?;

        #[allow(clippy::type_complexity)]
        let result: rusqlite::Result<(String, String, Option<String>, Option<String>, i64)> = conn.query_row(
            "SELECT peer_id, display_name, bio, avatar_hash, created_at FROM local_identity WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        );

        match result {
            Ok((peer_id, display_name, bio, avatar_hash, created_at)) => {
                // Use full peer_id as account ID for uniqueness
                let account_id = peer_id.clone();

                let account = AccountInfo {
                    id: account_id.clone(),
                    display_name,
                    verified_qualified_name: None,
                    verified_name_not_after: None,
                    avatar_hash,
                    bio,
                    peer_id,
                    created_at,
                    last_accessed_at: Some(chrono::Utc::now().timestamp()),
                    data_path: "default".to_string(), // Legacy data stays in default location
                };

                let mut registry = AccountsRegistry::default();
                registry
                    .accounts
                    .insert(account_id.clone(), account.clone());
                registry.active_account_id = Some(account_id);
                self.save_registry_unlocked(&registry)?;

                info!("Migrated legacy account to multi-account registry");
                Ok(Some(account))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

impl Clone for AccountsService {
    fn clone(&self) -> Self {
        Self {
            registry_path: self.registry_path.clone(),
            base_data_dir: self.base_data_dir.clone(),
            runtime_data_path: self.runtime_data_path.clone(),
            registry_lock: Arc::clone(&self.registry_lock),
        }
    }
}

// Tests require tempfile crate - run with cargo test
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::io::ErrorKind;
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn create_temp_dir() -> PathBuf {
        let dir = env::temp_dir().join(format!("harbor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup_temp_dir(dir: &PathBuf) {
        let _ = std::fs::remove_dir_all(dir);
    }

    fn write_identity_database(data_dir: &Path, peer_id: &str) {
        std::fs::create_dir_all(data_dir).unwrap();
        let connection = rusqlite::Connection::open(data_dir.join("harbor.db")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE local_identity (id INTEGER PRIMARY KEY, peer_id TEXT NOT NULL);",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO local_identity (id, peer_id) VALUES (1, ?1)",
                [peer_id],
            )
            .unwrap();
    }

    fn account(peer_id: &str, data_path: &str) -> AccountInfo {
        AccountInfo {
            id: peer_id.to_string(),
            display_name: peer_id.to_string(),
            verified_qualified_name: None,
            verified_name_not_after: None,
            avatar_hash: None,
            bio: None,
            peer_id: peer_id.to_string(),
            created_at: 1,
            last_accessed_at: Some(1),
            data_path: data_path.to_string(),
        }
    }

    #[test]
    fn test_empty_registry() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());

        let accounts = service.list_accounts().unwrap();
        assert!(accounts.is_empty());
        assert!(!service.has_accounts().unwrap());

        cleanup_temp_dir(&temp);
    }

    #[test]
    fn test_register_and_list_accounts() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());

        let _account1 = service
            .register_account(
                "12D3KooWTestPeer1".to_string(),
                "Alice".to_string(),
                Some("Hello!".to_string()),
                None,
            )
            .unwrap();

        let _account2 = service
            .register_account(
                "12D3KooWTestPeer2".to_string(),
                "Bob".to_string(),
                None,
                None,
            )
            .unwrap();

        assert!(service.has_accounts().unwrap());

        let accounts = service.list_accounts().unwrap();
        assert_eq!(accounts.len(), 2);

        // When timestamps are equal (created in same second), sorted alphabetically by ID
        // 12D3KooWTestPeer1 < 12D3KooWTestPeer2, so Alice comes first
        assert_eq!(accounts[0].display_name, "Alice");
        assert_eq!(accounts[1].display_name, "Bob");

        cleanup_temp_dir(&temp);
    }

    #[test]
    fn first_registered_account_uses_the_open_default_profile() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());
        let registered = service
            .register_account("peer-a".into(), "Alice".into(), None, None)
            .unwrap();

        assert_eq!(registered.data_path, "default");
        assert_eq!(service.get_account_data_path(&registered.id).unwrap(), temp);
        cleanup_temp_dir(&temp);
    }

    #[test]
    fn identity_registered_from_non_default_runtime_resolves_to_that_profile() {
        let temp = create_temp_dir();
        write_identity_database(&temp, "peer-a");
        let default_service = AccountsService::new(temp.clone());
        let account_a = default_service
            .register_account("peer-a".into(), "Alice".into(), None, None)
            .unwrap();

        let profile_b = temp.join("profile-peer-b");
        write_identity_database(&profile_b, "peer-b");
        let profile_service = AccountsService::new(temp.clone())
            .with_runtime_data_dir(&profile_b)
            .unwrap();
        let account_b = profile_service
            .register_account("peer-b".into(), "Bob".into(), None, None)
            .unwrap();

        assert_eq!(account_a.data_path, "default");
        assert_eq!(account_b.data_path, "profile-peer-b");
        profile_service.set_active_account(&account_a.id).unwrap();
        assert_eq!(profile_service.resolve_active_data_dir().unwrap(), temp);
        profile_service.set_active_account(&account_b.id).unwrap();
        assert_eq!(
            profile_service.resolve_active_data_dir().unwrap(),
            profile_b
        );
        cleanup_temp_dir(&temp);
    }

    #[test]
    fn active_profile_is_contained_and_owned_by_the_registered_peer() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());
        let selected = temp.join("profiles/alice");
        write_identity_database(&selected, "peer-a");
        let mut registry = AccountsRegistry::default();
        registry
            .accounts
            .insert("peer-a".into(), account("peer-a", "profiles/alice"));
        registry.active_account_id = Some("peer-a".into());
        service.save_registry(&registry).unwrap();

        assert_eq!(service.resolve_active_data_dir().unwrap(), selected);

        registry.accounts.get_mut("peer-a").unwrap().data_path = "../outside".into();
        service.save_registry(&registry).unwrap();
        assert!(service
            .resolve_active_data_dir()
            .unwrap_err()
            .to_string()
            .contains("contained relative path"));
        cleanup_temp_dir(&temp);
    }

    #[test]
    fn active_profile_rejects_a_database_owned_by_another_peer() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());
        let selected = temp.join("profile-a");
        write_identity_database(&selected, "peer-b");
        let mut registry = AccountsRegistry::default();
        registry
            .accounts
            .insert("peer-a".into(), account("peer-a", "profile-a"));
        registry.active_account_id = Some("peer-a".into());
        service.save_registry(&registry).unwrap();

        let error = service.resolve_active_data_dir().unwrap_err();
        assert!(error.to_string().contains("does not own database identity"));
        cleanup_temp_dir(&temp);
    }

    #[test]
    fn legacy_single_account_path_is_repaired_when_root_identity_matches() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());
        write_identity_database(&temp, "peer-a");
        let mut registry = AccountsRegistry::default();
        registry
            .accounts
            .insert("peer-a".into(), account("peer-a", "profile-peer-a"));
        registry.active_account_id = Some("peer-a".into());
        service.save_registry(&registry).unwrap();

        assert_eq!(service.resolve_active_data_dir().unwrap(), temp);
        assert_eq!(
            service.get_active_account().unwrap().unwrap().data_path,
            "default"
        );
        cleanup_temp_dir(&temp);
    }

    #[test]
    fn legacy_root_account_is_repaired_among_stale_registry_entries() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());
        write_identity_database(&temp, "peer-real");
        let mut registry = AccountsRegistry::default();
        registry.accounts.insert(
            "peer-real".into(),
            account("peer-real", "profile-peer-real"),
        );
        registry.accounts.insert(
            "peer-stale".into(),
            account("peer-stale", "profile-peer-stale"),
        );
        registry.active_account_id = Some("peer-stale".into());
        service.save_registry(&registry).unwrap();

        let repaired = service.validate_account_runtime("peer-real").unwrap();
        assert_eq!(repaired.data_path, "default");
        assert_eq!(
            service.get_account("peer-real").unwrap().unwrap().data_path,
            "default"
        );
        service.set_active_account("peer-real").unwrap();
        assert_eq!(service.resolve_active_data_dir().unwrap(), temp);
        cleanup_temp_dir(&temp);
    }

    #[test]
    fn legacy_root_repair_rejects_conflicting_default_assignment() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());
        write_identity_database(&temp, "peer-real");
        let mut registry = AccountsRegistry::default();
        registry.accounts.insert(
            "peer-real".into(),
            account("peer-real", "profile-peer-real"),
        );
        registry
            .accounts
            .insert("peer-other".into(), account("peer-other", "default"));
        service.save_registry(&registry).unwrap();

        let error = service.validate_account_runtime("peer-real").unwrap_err();
        assert!(error.to_string().contains("already assigned"));
        assert_eq!(
            service.get_account("peer-real").unwrap().unwrap().data_path,
            "profile-peer-real"
        );
        cleanup_temp_dir(&temp);
    }

    #[test]
    fn concurrent_registry_updates_do_not_lose_accounts() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());
        let workers = 24;
        let barrier = Arc::new(Barrier::new(workers));
        let mut joins = Vec::new();

        for index in 0..workers {
            let temp = temp.clone();
            let barrier = Arc::clone(&barrier);
            joins.push(thread::spawn(move || {
                // Exercise separately constructed services, not only clones.
                let service = AccountsService::new(temp);
                barrier.wait();
                service.register_account(
                    format!("peer-{index:02}"),
                    format!("Account {index:02}"),
                    None,
                    None,
                )
            }));
        }

        for join in joins {
            join.join().unwrap().unwrap();
        }

        let accounts = service.list_accounts().unwrap();
        assert_eq!(accounts.len(), workers);
        for index in 0..workers {
            assert!(accounts
                .iter()
                .any(|account| account.id == format!("peer-{index:02}")));
        }
        cleanup_temp_dir(&temp);
    }

    #[test]
    fn failed_atomic_write_preserves_the_previous_registry() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());
        service
            .register_account("peer-original".into(), "Original".into(), None, None)
            .unwrap();

        let _guard = service.lock_registry().unwrap();
        let mut changed = service.load_registry_unlocked().unwrap();
        changed.accounts.insert(
            "peer-uncommitted".into(),
            AccountInfo {
                id: "peer-uncommitted".into(),
                display_name: "Uncommitted".into(),
                verified_qualified_name: None,
                verified_name_not_after: None,
                avatar_hash: None,
                bio: None,
                peer_id: "peer-uncommitted".into(),
                created_at: 2,
                last_accessed_at: Some(2),
                data_path: "profile-peer-uncommitted".into(),
            },
        );

        let error = service
            .save_registry_unlocked_with(&changed, |file, content| {
                file.write_all(&content[..content.len() / 2])?;
                Err(std::io::Error::new(
                    ErrorKind::WriteZero,
                    "injected short write",
                ))
            })
            .unwrap_err();
        assert!(error.to_string().contains("injected short write"));
        drop(_guard);

        let persisted = service.load_registry().unwrap();
        assert!(persisted.accounts.contains_key("peer-original"));
        assert!(!persisted.accounts.contains_key("peer-uncommitted"));
        assert_eq!(
            persisted.active_account_id.as_deref(),
            Some("peer-original")
        );
        cleanup_temp_dir(&temp);
    }

    #[test]
    fn permission_or_disk_failure_preserves_the_previous_registry() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());
        service
            .register_account("peer-original".into(), "Original".into(), None, None)
            .unwrap();

        for kind in [ErrorKind::PermissionDenied, ErrorKind::StorageFull] {
            let _guard = service.lock_registry().unwrap();
            let mut changed = service.load_registry_unlocked().unwrap();
            changed.active_account_id = None;
            let error = service
                .save_registry_unlocked_with(&changed, |_file, _content| {
                    Err(std::io::Error::new(kind, "injected registry failure"))
                })
                .unwrap_err();
            assert!(error.to_string().contains("injected registry failure"));
            drop(_guard);

            let persisted = service.load_registry().unwrap();
            assert_eq!(
                persisted.active_account_id.as_deref(),
                Some("peer-original")
            );
        }
        cleanup_temp_dir(&temp);
    }

    #[test]
    fn test_set_active_account() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());

        let account = service
            .register_account(
                "12D3KooWTestPeer1".to_string(),
                "Alice".to_string(),
                None,
                None,
            )
            .unwrap();

        let active = service.get_active_account().unwrap();
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, account.id);

        cleanup_temp_dir(&temp);
    }

    #[test]
    fn verified_qualified_name_is_persisted_for_locked_account_selection() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());
        let account = service
            .register_account(
                "12D3KooWTestPeer1".to_string(),
                "Untrusted legacy alias".to_string(),
                None,
                None,
            )
            .unwrap();

        service
            .update_verified_qualified_name(&account.peer_id, "@alice@relay.test", 4_000_000_000)
            .unwrap();

        let loaded = service.get_account(&account.id).unwrap().unwrap();
        assert_eq!(
            loaded.verified_qualified_name,
            Some("@alice@relay.test".to_string())
        );
        assert_eq!(loaded.verified_name_not_after, Some(4_000_000_000));
        cleanup_temp_dir(&temp);
    }

    #[test]
    fn legacy_account_registry_without_verified_name_still_loads() {
        let account: AccountInfo = serde_json::from_value(serde_json::json!({
            "id": "peer-old",
            "displayName": "Old alias",
            "avatarHash": null,
            "bio": null,
            "peerId": "peer-old",
            "createdAt": 1,
            "lastAccessedAt": 1,
            "dataPath": "default"
        }))
        .unwrap();
        assert_eq!(account.verified_qualified_name, None);
    }

    #[test]
    fn test_remove_account() {
        let temp = create_temp_dir();
        let service = AccountsService::new(temp.clone());

        let account = service
            .register_account(
                "12D3KooWTestPeer1".to_string(),
                "Alice".to_string(),
                None,
                None,
            )
            .unwrap();

        service.remove_account(&account.id, false).unwrap();

        let accounts = service.list_accounts().unwrap();
        assert!(accounts.is_empty());

        cleanup_temp_dir(&temp);
    }
}
