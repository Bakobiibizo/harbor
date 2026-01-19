use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::info;

/// Account metadata stored in the accounts registry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    /// Unique identifier for the account (derived from peer_id)
    pub id: String,
    /// User's display name
    pub display_name: String,
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
}

impl AccountsService {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let registry_path = app_data_dir.join("accounts.json");
        Self {
            registry_path,
            base_data_dir: app_data_dir,
        }
    }

    /// Load the accounts registry from disk
    pub fn load_registry(&self) -> Result<AccountsRegistry> {
        if !self.registry_path.exists() {
            return Ok(AccountsRegistry::default());
        }

        let content = fs::read_to_string(&self.registry_path)?;

        serde_json::from_str(&content).map_err(|e| {
            AppError::Serialization(format!("Failed to parse accounts registry: {}", e))
        })
    }

    /// Save the accounts registry to disk
    pub fn save_registry(&self, registry: &AccountsRegistry) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.registry_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(registry)
            .map_err(|e| AppError::Serialization(format!("Failed to serialize registry: {}", e)))?;

        fs::write(&self.registry_path, content)?;

        Ok(())
    }

    /// List all registered accounts
    pub fn list_accounts(&self) -> Result<Vec<AccountInfo>> {
        let registry = self.load_registry()?;
        let mut accounts: Vec<AccountInfo> = registry.accounts.values().cloned().collect();

        // Sort by last accessed (most recent first), then by created_at
        accounts.sort_by(|a, b| match (a.last_accessed_at, b.last_accessed_at) {
            (Some(a_time), Some(b_time)) => b_time.cmp(&a_time),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.created_at.cmp(&a.created_at),
        });

        Ok(accounts)
    }

    /// Get account by ID
    pub fn get_account(&self, account_id: &str) -> Result<Option<AccountInfo>> {
        let registry = self.load_registry()?;
        Ok(registry.accounts.get(account_id).cloned())
    }

    /// Get the currently active account
    pub fn get_active_account(&self) -> Result<Option<AccountInfo>> {
        let registry = self.load_registry()?;
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
        let mut registry = self.load_registry()?;

        // Use full peer_id as account ID for uniqueness
        let account_id = peer_id.clone();

        // Check if already exists
        if registry.accounts.contains_key(&account_id) {
            return Err(AppError::AlreadyExists(format!(
                "Account with ID {} already exists",
                account_id
            )));
        }

        let now = chrono::Utc::now().timestamp();

        // Data path is relative to base_data_dir
        let data_path = format!("profile-{}", account_id);

        let account = AccountInfo {
            id: account_id.clone(),
            display_name,
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

        self.save_registry(&registry)?;

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
        let mut registry = self.load_registry()?;

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
        self.save_registry(&registry)?;

        Ok(updated)
    }

    /// Set the active account and update last_accessed_at
    pub fn set_active_account(&self, account_id: &str) -> Result<AccountInfo> {
        let mut registry = self.load_registry()?;

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
        self.save_registry(&registry)?;

        Ok(registry.accounts.get(account_id).cloned().unwrap())
    }

    /// Remove an account from the registry
    pub fn remove_account(&self, account_id: &str, delete_data: bool) -> Result<()> {
        let mut registry = self.load_registry()?;

        let account = registry
            .accounts
            .remove(account_id)
            .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))?;

        // Clear active account if it was this one
        if registry.active_account_id.as_deref() == Some(account_id) {
            registry.active_account_id = None;
        }

        self.save_registry(&registry)?;

        // Optionally delete the account's data directory
        if delete_data {
            let data_dir = self.base_data_dir.join(&account.data_path);
            if data_dir.exists() {
                fs::remove_dir_all(&data_dir)?;
                info!("Deleted account data at {:?}", data_dir);
            }
        }

        info!("Removed account: {}", account_id);
        Ok(())
    }

    /// Get the data directory path for an account
    pub fn get_account_data_path(&self, account_id: &str) -> Result<PathBuf> {
        let registry = self.load_registry()?;

        let account = registry
            .accounts
            .get(account_id)
            .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))?;

        Ok(self.base_data_dir.join(&account.data_path))
    }

    /// Check if any accounts exist
    pub fn has_accounts(&self) -> Result<bool> {
        let registry = self.load_registry()?;
        Ok(!registry.accounts.is_empty())
    }

    /// Migrate existing single-account setup to multi-account registry
    /// This is called on app startup to handle existing users
    pub fn migrate_legacy_account(&self, legacy_db_path: &PathBuf) -> Result<Option<AccountInfo>> {
        // Check if legacy database exists and we don't have any accounts yet
        if !legacy_db_path.exists() {
            return Ok(None);
        }

        let registry = self.load_registry()?;
        if !registry.accounts.is_empty() {
            return Ok(None);
        }

        // Try to read identity info from the legacy database
        use rusqlite::Connection;

        let conn = Connection::open(legacy_db_path).map_err(AppError::Database)?;

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
                self.save_registry(&registry)?;

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
        }
    }
}

// Tests require tempfile crate - run with cargo test
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn create_temp_dir() -> PathBuf {
        let dir = env::temp_dir().join(format!("harbor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup_temp_dir(dir: &PathBuf) {
        let _ = std::fs::remove_dir_all(dir);
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

        // Most recently accessed should be first
        assert_eq!(accounts[0].display_name, "Bob");
        assert_eq!(accounts[1].display_name, "Alice");

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
