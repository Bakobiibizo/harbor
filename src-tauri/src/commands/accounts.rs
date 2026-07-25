use crate::commands::network::NetworkState;
use crate::error::AppError;
use crate::services::accounts_service::AccountInfo;
use crate::services::{
    AccountBackupService, AccountsService, BackupExportResult, BackupRestoreResult,
    DeleteAccountProfileResult, IdentityService,
};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tauri::State;
use tokio::sync::Mutex;

fn account_switch_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

async fn switch_active_account(
    accounts_service: &AccountsService,
    identity_service: &IdentityService,
    network: &NetworkState,
    account_id: &str,
) -> Result<AccountInfo, AppError> {
    let _switch_guard = account_switch_lock().lock().await;

    // Resolve the target before disrupting the current profile. A missing target
    // must leave its network and unlocked keys untouched.
    if let Some(active) = accounts_service
        .get_active_account()?
        .filter(|account| account.id == account_id)
    {
        return Ok(active);
    }
    let target = accounts_service.validate_account_runtime(account_id)?;

    // The lifecycle mutex stays held until all network workers have exited and
    // the private keys have been cleared. Only then is the registry commit made.
    // A failed commit therefore leaves the old profile stopped and locked rather
    // than falsely reporting a completed switch while it is still serving.
    network.stop_and_lock_identity(identity_service).await?;
    accounts_service.set_active_account(&target.id)
}

async fn activate_restored_account(
    accounts_service: &AccountsService,
    identity_service: &IdentityService,
    network: &NetworkState,
    restored_account_id: &str,
) -> Result<AccountInfo, AppError> {
    network.stop_and_lock_identity(identity_service).await?;
    accounts_service.set_active_account(restored_account_id)
}

/// List all registered accounts
#[tauri::command]
pub async fn list_accounts(
    accounts_service: State<'_, Arc<AccountsService>>,
) -> Result<Vec<AccountInfo>, AppError> {
    accounts_service.list_accounts()
}

/// Get a specific account by ID
#[tauri::command]
pub async fn get_account(
    accounts_service: State<'_, Arc<AccountsService>>,
    account_id: String,
) -> Result<Option<AccountInfo>, AppError> {
    accounts_service.get_account(&account_id)
}

/// Get the currently active account
#[tauri::command]
pub async fn get_active_account(
    accounts_service: State<'_, Arc<AccountsService>>,
) -> Result<Option<AccountInfo>, AppError> {
    accounts_service.get_active_account()
}

/// Check if any accounts exist
#[tauri::command]
pub async fn has_accounts(
    accounts_service: State<'_, Arc<AccountsService>>,
) -> Result<bool, AppError> {
    accounts_service.has_accounts()
}

/// Set the active account (for switching between accounts)
#[tauri::command]
pub async fn set_active_account(
    accounts_service: State<'_, Arc<AccountsService>>,
    identity_service: State<'_, Arc<IdentityService>>,
    network: State<'_, NetworkState>,
    account_id: String,
) -> Result<AccountInfo, AppError> {
    switch_active_account(&accounts_service, &identity_service, &network, &account_id).await
}

/// Remove an account from the registry
#[tauri::command]
pub async fn remove_account(
    accounts_service: State<'_, Arc<AccountsService>>,
    account_id: String,
    delete_data: bool,
) -> Result<(), AppError> {
    accounts_service.remove_account(&account_id, delete_data)
}

/// Export the active identity as a versioned authenticated encrypted archive.
#[tauri::command]
pub async fn export_identity_backup(
    backup_service: State<'_, Arc<AccountBackupService>>,
    path: String,
    password: String,
) -> Result<BackupExportResult, AppError> {
    let service = backup_service.inner().clone();
    tokio::task::spawn_blocking(move || {
        service.export_identity_backup(PathBuf::from(path), &password)
    })
    .await
    .map_err(|error| AppError::Internal(format!("Identity backup worker failed: {error}")))?
}

/// Restore an identity into an isolated profile and select it for next startup.
#[tauri::command]
pub async fn restore_identity_backup(
    backup_service: State<'_, Arc<AccountBackupService>>,
    accounts_service: State<'_, Arc<AccountsService>>,
    identity_service: State<'_, Arc<IdentityService>>,
    network: State<'_, NetworkState>,
    path: String,
    password: String,
) -> Result<BackupRestoreResult, AppError> {
    let _guard = account_switch_lock().lock().await;
    let previous_active = accounts_service.get_active_account()?;
    let service = backup_service.inner().clone();
    let restored = tokio::task::spawn_blocking(move || {
        service.restore_identity_backup(PathBuf::from(path).as_path(), &password)
    })
    .await
    .map_err(|error| AppError::Internal(format!("Identity restore worker failed: {error}")))??;

    if previous_active.is_some() {
        // All password, archive and key-binding validation completed before the
        // current runtime is disturbed. The restored account is still inactive
        // at this point, so the registry can never claim it is active while the
        // old profile continues serving.
        activate_restored_account(
            &accounts_service,
            &identity_service,
            &network,
            &restored.account.id,
        )
        .await?;
    }
    Ok(restored)
}

/// Authenticate and durably schedule deletion of exactly one contained profile.
#[tauri::command]
pub async fn delete_account_profile(
    backup_service: State<'_, Arc<AccountBackupService>>,
    accounts_service: State<'_, Arc<AccountsService>>,
    identity_service: State<'_, Arc<IdentityService>>,
    network: State<'_, NetworkState>,
    account_id: String,
    password: String,
) -> Result<DeleteAccountProfileResult, AppError> {
    let _guard = account_switch_lock().lock().await;
    let service = backup_service.inner().clone();
    let authenticated = tokio::task::spawn_blocking({
        let account_id = account_id.clone();
        move || service.authenticate_account(&account_id, &password)
    })
    .await
    .map_err(|error| {
        AppError::Internal(format!("Account authentication worker failed: {error}"))
    })??;

    if accounts_service
        .get_active_account()?
        .is_some_and(|active| active.id == authenticated.id)
    {
        network.stop_and_lock_identity(&identity_service).await?;
    }

    let service = backup_service.inner().clone();
    tokio::task::spawn_blocking(move || service.schedule_profile_deletion(&authenticated))
        .await
        .map_err(|error| AppError::Internal(format!("Account deletion worker failed: {error}")))?
}

/// Update account metadata in the registry
#[tauri::command]
pub async fn update_account_metadata(
    accounts_service: State<'_, Arc<AccountsService>>,
    account_id: String,
    display_name: Option<String>,
    bio: Option<Option<String>>,
    avatar_hash: Option<Option<String>>,
) -> Result<AccountInfo, AppError> {
    accounts_service.update_account(&account_id, display_name, bio, avatar_hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::CreateIdentityRequest;

    fn unlocked_identity() -> IdentityService {
        let database = Arc::new(Database::in_memory().unwrap());
        let service = IdentityService::new(database);
        service
            .create_identity(CreateIdentityRequest {
                display_name: "Current account".into(),
                passphrase: "test-password".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        service
    }

    #[tokio::test]
    async fn invalid_target_does_not_disrupt_the_current_session() {
        let temp = tempfile::tempdir().unwrap();
        let accounts = AccountsService::new(temp.path().to_path_buf());
        accounts
            .register_account("current-peer".into(), "Current".into(), None, None)
            .unwrap();
        let identity = unlocked_identity();
        let network = NetworkState::new();

        let error = switch_active_account(&accounts, &identity, &network, "missing-peer")
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::NotFound(_)));
        assert!(identity.is_unlocked());
        assert_eq!(
            accounts.get_active_account().unwrap().unwrap().id,
            "current-peer"
        );
    }

    #[tokio::test]
    async fn unusable_registered_target_does_not_disrupt_the_current_session() {
        let temp = tempfile::tempdir().unwrap();
        let accounts = AccountsService::new(temp.path().to_path_buf());
        accounts
            .register_account("current-peer".into(), "Current".into(), None, None)
            .unwrap();
        accounts
            .register_account("target-peer".into(), "Target".into(), None, None)
            .unwrap();
        accounts.set_active_account("current-peer").unwrap();
        let identity = unlocked_identity();
        let network = NetworkState::new();

        let error = switch_active_account(&accounts, &identity, &network, "target-peer")
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::InvalidData(_)));
        assert!(identity.is_unlocked());
        assert_eq!(
            accounts.get_active_account().unwrap().unwrap().id,
            "current-peer"
        );
    }

    #[tokio::test]
    async fn switch_locks_keys_before_committing_the_target() {
        let temp = tempfile::tempdir().unwrap();
        let target_identity = IdentityService::new(Arc::new(
            Database::new(temp.path().join("harbor.db")).unwrap(),
        ));
        let target_peer = target_identity
            .create_identity(CreateIdentityRequest {
                display_name: "Target".into(),
                passphrase: "target-password".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap()
            .peer_id;
        let accounts = AccountsService::new(temp.path().to_path_buf());
        accounts
            .register_account("current-peer".into(), "Current".into(), None, None)
            .unwrap();
        accounts
            .register_account(target_peer.clone(), "Target".into(), None, None)
            .unwrap();
        accounts.set_active_account("current-peer").unwrap();
        let identity = unlocked_identity();
        let network = NetworkState::new();

        let selected = switch_active_account(&accounts, &identity, &network, &target_peer)
            .await
            .unwrap();

        assert_eq!(selected.id, target_peer);
        assert!(!identity.is_unlocked());
        assert_eq!(
            accounts.get_active_account().unwrap().unwrap().id,
            selected.id
        );
    }

    #[tokio::test]
    async fn restored_activation_locks_old_runtime_before_registry_selection() {
        let temp = tempfile::tempdir().unwrap();
        let accounts = AccountsService::new(temp.path().to_path_buf());
        accounts
            .register_account("current-peer".into(), "Current".into(), None, None)
            .unwrap();
        accounts
            .register_account("restored-peer".into(), "Restored".into(), None, None)
            .unwrap();
        accounts.set_active_account("current-peer").unwrap();
        let identity = unlocked_identity();
        let network = NetworkState::new();

        let selected = activate_restored_account(&accounts, &identity, &network, "restored-peer")
            .await
            .unwrap();

        assert_eq!(selected.id, "restored-peer");
        assert!(!identity.is_unlocked());
        assert_eq!(
            accounts.get_active_account().unwrap().unwrap().id,
            "restored-peer"
        );
    }
}
