use super::NetworkState;
use crate::error::{AppError, ErrorResponse};
use crate::models::{CreateIdentityRequest, IdentityInfo};
use crate::services::identity_service::IdentityInitializationSnapshot;
use crate::services::{AccountsService, ContactsService, IdentityService, MediaStorageService};
use crate::PendingDeepLink;
use serde::Serialize;
use std::sync::{Arc, OnceLock};
use tauri::{Emitter, Manager, State};
use tokio::sync::Semaphore;
use tracing::info;

const MAX_CONCURRENT_PASSWORD_WORK: usize = 2;

fn password_work_limit() -> Arc<Semaphore> {
    static LIMIT: OnceLock<Arc<Semaphore>> = OnceLock::new();
    LIMIT
        .get_or_init(|| Arc::new(Semaphore::new(MAX_CONCURRENT_PASSWORD_WORK)))
        .clone()
}

async fn run_password_work_with_limit<T, F>(limit: Arc<Semaphore>, work: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
{
    let permit = limit
        .acquire_owned()
        .await
        .map_err(|_| AppError::Internal("Password worker limit closed".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work()
    })
    .await
    .map_err(|error| AppError::Internal(format!("Password worker failed: {error}")))?
}

async fn run_password_work<T, F>(work: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
{
    run_password_work_with_limit(password_work_limit(), work).await
}

/// Drain any deep-link contact strings that arrived while the identity was locked,
/// emitting each one to the frontend for confirmation.
fn drain_pending_deep_links(app: &tauri::AppHandle) {
    if let Ok(mut queue) = app.state::<PendingDeepLink>().0.lock() {
        for contact_string in queue.drain(..) {
            let _ = app.emit("deep_link_contact", &contact_string);
        }
    }
}

fn register_created_identity(
    identity_service: &IdentityService,
    accounts_service: &AccountsService,
    identity: &IdentityInfo,
    display_name: String,
    bio: Option<String>,
) -> Result<(), AppError> {
    if let Err(registry_error) = accounts_service.register_account(
        identity.peer_id.clone(),
        display_name,
        bio,
        identity.avatar_hash.clone(),
    ) {
        if let Err(rollback_error) = identity_service.rollback_created_identity(&identity.peer_id) {
            return Err(AppError::Internal(format!(
                "Account registry failed ({registry_error}); identity rollback also failed ({rollback_error})"
            )));
        }
        return Err(registry_error);
    }
    Ok(())
}

/// Check if an identity has been created
#[tauri::command]
pub async fn has_identity(
    identity_service: State<'_, Arc<IdentityService>>,
) -> Result<bool, AppError> {
    identity_service.has_identity()
}

/// Check if the identity is currently unlocked
#[tauri::command]
pub async fn is_identity_unlocked(
    identity_service: State<'_, Arc<IdentityService>>,
) -> Result<bool, AppError> {
    Ok(identity_service.is_unlocked())
}

/// Get identity info (public data only)
#[tauri::command]
pub async fn get_identity_info(
    identity_service: State<'_, Arc<IdentityService>>,
) -> Result<Option<IdentityInfo>, AppError> {
    identity_service.get_identity_info()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IdentityInitializationFailureSource {
    IdentityDatabase,
    IdentityCorruption,
    AccountRegistry,
}

/// Authoritative application-entry result. Failure outcomes are values rather
/// than rejected commands so the frontend can never reinterpret a successful
/// absence lookup and a failed lookup as the same state.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum IdentityInitializationState {
    Absent,
    Locked {
        identity: IdentityInfo,
    },
    Unlocked {
        identity: IdentityInfo,
    },
    RecoverableError {
        source: IdentityInitializationFailureSource,
        error: ErrorResponse,
    },
    FatalError {
        source: IdentityInitializationFailureSource,
        error: ErrorResponse,
    },
}

fn initialization_error_is_recoverable(error: &AppError) -> bool {
    match error {
        AppError::Database(rusqlite::Error::SqliteFailure(sqlite, _)) => matches!(
            sqlite.code,
            rusqlite::ErrorCode::DatabaseBusy
                | rusqlite::ErrorCode::DatabaseLocked
                | rusqlite::ErrorCode::SystemIoFailure
                | rusqlite::ErrorCode::CannotOpen
        ),
        AppError::Io(error) => matches!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied
                | std::io::ErrorKind::Interrupted
                | std::io::ErrorKind::TimedOut
                | std::io::ErrorKind::WouldBlock
        ),
        AppError::PermissionDenied(_) | AppError::Internal(_) => true,
        _ => false,
    }
}

pub(crate) fn initialization_failure(
    source: IdentityInitializationFailureSource,
    error: AppError,
) -> IdentityInitializationState {
    let recoverable = initialization_error_is_recoverable(&error);
    let details = error.to_string();
    let error = error.to_response().with_details(details);
    if recoverable {
        IdentityInitializationState::RecoverableError { source, error }
    } else {
        IdentityInitializationState::FatalError { source, error }
    }
}

/// A failure before profile services exist cannot be retried in-process because
/// doing so would require rebinding all managed Tauri state. Mark transient
/// startup failures as fatal for this process and give an honest restart action.
pub(crate) fn startup_initialization_failure(
    source: IdentityInitializationFailureSource,
    error: AppError,
) -> IdentityInitializationState {
    match initialization_failure(source, error) {
        IdentityInitializationState::RecoverableError { source, mut error } => {
            error.recovery = Some(
                "Correct the local access problem, then restart Harbor to reopen this account."
                    .into(),
            );
            IdentityInitializationState::FatalError { source, error }
        }
        state => state,
    }
}

/// Captures a failure that occurred before the selected profile services could
/// be opened. Recovery services keep the window and typed initialization
/// command available, but the failure remains authoritative for the session.
pub struct StartupInitializationState(pub Option<IdentityInitializationState>);

fn effective_initialization_state(
    identity_service: &IdentityService,
    accounts_service: &AccountsService,
    startup_failure: Option<&IdentityInitializationState>,
) -> IdentityInitializationState {
    startup_failure
        .cloned()
        .unwrap_or_else(|| identity_initialization_state(identity_service, accounts_service))
}

fn identity_initialization_state(
    identity_service: &IdentityService,
    accounts_service: &AccountsService,
) -> IdentityInitializationState {
    let registry = match accounts_service.load_registry() {
        Ok(registry) => registry,
        Err(error) => {
            return initialization_failure(
                IdentityInitializationFailureSource::AccountRegistry,
                error,
            )
        }
    };
    let active_account = registry
        .active_account_id
        .as_ref()
        .and_then(|account_id| registry.accounts.get(account_id))
        .cloned();

    if active_account.is_none() && !registry.accounts.is_empty() {
        return initialization_failure(
            IdentityInitializationFailureSource::AccountRegistry,
            AppError::InvalidData(
                "The account registry contains accounts but has no active account".into(),
            ),
        );
    }

    if let Some(account) = active_account.as_ref() {
        if let Err(error) = accounts_service.validate_account_runtime(&account.id) {
            return initialization_failure(
                IdentityInitializationFailureSource::AccountRegistry,
                error,
            );
        }
    }

    let snapshot = match identity_service.initialization_snapshot() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let source = match &error {
                AppError::Crypto(_)
                | AppError::CryptoEncryption(_)
                | AppError::CryptoDecryption(_)
                | AppError::InvalidData(_)
                | AppError::Serialization(_) => {
                    IdentityInitializationFailureSource::IdentityCorruption
                }
                _ => IdentityInitializationFailureSource::IdentityDatabase,
            };
            return initialization_failure(source, error);
        }
    };

    match (snapshot, active_account) {
        (IdentityInitializationSnapshot::Absent, None) => IdentityInitializationState::Absent,
        (IdentityInitializationSnapshot::Absent, Some(account)) => initialization_failure(
            IdentityInitializationFailureSource::AccountRegistry,
            AppError::InvalidData(format!(
                "Active account {} has no persisted identity",
                account.id
            )),
        ),
        (IdentityInitializationSnapshot::Locked(identity), Some(account))
            if account.peer_id == identity.peer_id =>
        {
            if account.avatar_hash != identity.avatar_hash {
                if let Err(error) = accounts_service.update_account(
                    &account.id,
                    None,
                    None,
                    Some(identity.avatar_hash.clone()),
                ) {
                    return initialization_failure(
                        IdentityInitializationFailureSource::AccountRegistry,
                        error,
                    );
                }
            }
            IdentityInitializationState::Locked { identity }
        }
        (IdentityInitializationSnapshot::Unlocked(identity), Some(account))
            if account.peer_id == identity.peer_id =>
        {
            if account.avatar_hash != identity.avatar_hash {
                if let Err(error) = accounts_service.update_account(
                    &account.id,
                    None,
                    None,
                    Some(identity.avatar_hash.clone()),
                ) {
                    return initialization_failure(
                        IdentityInitializationFailureSource::AccountRegistry,
                        error,
                    );
                }
            }
            IdentityInitializationState::Unlocked { identity }
        }
        (IdentityInitializationSnapshot::Locked(identity), None)
        | (IdentityInitializationSnapshot::Unlocked(identity), None) => initialization_failure(
            IdentityInitializationFailureSource::AccountRegistry,
            AppError::InvalidData(format!(
                "Identity {} is missing from the account registry",
                identity.peer_id
            )),
        ),
        (IdentityInitializationSnapshot::Locked(identity), Some(account))
        | (IdentityInitializationSnapshot::Unlocked(identity), Some(account)) => {
            initialization_failure(
                IdentityInitializationFailureSource::AccountRegistry,
                AppError::InvalidData(format!(
                    "Active account {} does not match identity {}",
                    account.peer_id, identity.peer_id
                )),
            )
        }
    }
}

#[tauri::command]
pub fn get_identity_initialization_state(
    identity_service: State<'_, Arc<IdentityService>>,
    accounts_service: State<'_, Arc<AccountsService>>,
    startup_state: State<'_, StartupInitializationState>,
) -> IdentityInitializationState {
    effective_initialization_state(
        &identity_service,
        &accounts_service,
        startup_state.0.as_ref(),
    )
}

/// Create a new identity
#[tauri::command]
pub async fn create_identity(
    app: tauri::AppHandle,
    identity_service: State<'_, Arc<IdentityService>>,
    accounts_service: State<'_, Arc<AccountsService>>,
    request: CreateIdentityRequest,
) -> Result<IdentityInfo, AppError> {
    let display_name = request.display_name.clone();
    let bio = request.bio.clone();

    let worker_identity_service = identity_service.inner().clone();
    let identity =
        run_password_work(move || worker_identity_service.create_identity(request)).await?;

    // The database identity and file-backed account registry form one user-visible
    // creation operation. Never report success with only one side committed.
    register_created_identity(
        &identity_service,
        &accounts_service,
        &identity,
        display_name,
        bio,
    )?;
    info!("Registered new account in registry: {}", identity.peer_id);

    // create_identity auto-unlocks the identity, so replay any queued deep links
    drain_pending_deep_links(&app);

    Ok(identity)
}

/// Unlock the identity with passphrase
#[tauri::command]
pub async fn unlock_identity(
    app: tauri::AppHandle,
    identity_service: State<'_, Arc<IdentityService>>,
    passphrase: String,
) -> Result<IdentityInfo, AppError> {
    let worker_identity_service = identity_service.inner().clone();
    let identity = run_password_work(move || worker_identity_service.unlock(&passphrase)).await?;
    drain_pending_deep_links(&app);
    Ok(identity)
}

/// Atomically re-encrypt the local private keys under a new password.
#[tauri::command]
pub async fn change_identity_password(
    identity_service: State<'_, Arc<IdentityService>>,
    current_password: String,
    new_password: String,
) -> Result<(), AppError> {
    let worker_identity_service = identity_service.inner().clone();
    run_password_work(move || {
        worker_identity_service.change_password(&current_password, &new_password)
    })
    .await
}

/// Lock the identity
#[tauri::command]
pub async fn lock_identity(
    identity_service: State<'_, Arc<IdentityService>>,
    network: State<'_, NetworkState>,
) -> Result<(), AppError> {
    network.stop_and_lock_identity(&identity_service).await
}

/// Update display name
#[tauri::command]
pub async fn update_display_name(
    identity_service: State<'_, Arc<IdentityService>>,
    display_name: String,
) -> Result<(), AppError> {
    identity_service.update_display_name(&display_name)
}

/// Update bio
#[tauri::command]
pub async fn update_bio(
    identity_service: State<'_, Arc<IdentityService>>,
    bio: Option<String>,
) -> Result<(), AppError> {
    identity_service.update_bio(bio.as_deref())
}

/// Import, commit, and distribute a profile avatar. The registry and local
/// identity continue to reference the old avatar if any replacement step fails.
#[tauri::command]
pub async fn update_profile_avatar(
    file_path: Option<String>,
    identity_service: State<'_, Arc<IdentityService>>,
    accounts_service: State<'_, Arc<AccountsService>>,
    contacts_service: State<'_, Arc<ContactsService>>,
    media_service: State<'_, Arc<MediaStorageService>>,
    network: State<'_, NetworkState>,
) -> Result<IdentityInfo, AppError> {
    let before = identity_service
        .get_identity_info()?
        .ok_or_else(|| AppError::IdentityNotFound("No identity found".into()))?;
    let imported = if let Some(file_path) = file_path {
        let path = std::path::PathBuf::from(file_path);
        let mime_type = match path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref()
        {
            Some("jpg" | "jpeg") => "image/jpeg",
            Some("png") => "image/png",
            Some("gif") => "image/gif",
            Some("webp") => "image/webp",
            _ => {
                return Err(AppError::Validation(
                    "Choose a JPG, PNG, GIF, or WebP image".into(),
                ))
            }
        };
        let service = media_service.inner().clone();
        let mime_type = mime_type.to_string();
        Some(
            tokio::task::spawn_blocking(move || service.store_avatar_path(&path, &mime_type))
                .await
                .map_err(|error| {
                    AppError::Internal(format!("Avatar import worker failed: {error}"))
                })??,
        )
    } else {
        None
    };
    let new_hash = imported.as_ref().map(|media| media.media_hash.as_str());
    let new_mime = imported.as_ref().map(|media| media.mime_type.as_str());
    let update = match identity_service.replace_avatar(new_hash, new_mime) {
        Ok(update) => update,
        Err(error) => {
            if let Some(media) = imported.as_ref() {
                let _ = media_service.release_local_media(&media.media_hash);
            }
            return Err(error);
        }
    };
    if let Err(registry_error) = accounts_service.update_account(
        &before.peer_id,
        None,
        None,
        Some(imported.as_ref().map(|media| media.media_hash.clone())),
    ) {
        let rollback = identity_service.replace_avatar(
            update.old_avatar_hash.as_deref(),
            update.old_avatar_mime_type.as_deref(),
        );
        if let Some(media) = imported.as_ref() {
            let _ = media_service.release_local_media(&media.media_hash);
        }
        rollback.map_err(|rollback_error| {
            AppError::Internal(format!(
                "Account registry failed ({registry_error}); avatar rollback also failed ({rollback_error})"
            ))
        })?;
        return Err(registry_error);
    }

    if let Ok(handle) = network.get_handle().await {
        for contact in contacts_service.get_active_contacts()? {
            let Ok(peer_id) = contact.peer_id.parse() else {
                continue;
            };
            let _ = handle
                .request_identity_action(
                    peer_id,
                    uuid::Uuid::new_v4().to_string(),
                    "profile".into(),
                )
                .await;
        }
    }
    if let Some(old_hash) = update.old_avatar_hash.as_deref() {
        if Some(old_hash) != new_hash {
            if let Err(error) = media_service.release_local_media(old_hash) {
                tracing::warn!("Avatar updated but old media cleanup failed: {error}");
            }
        }
    }
    identity_service.get_identity_info()?.ok_or_else(|| {
        AppError::IdentityNotFound("Identity disappeared after avatar update".into())
    })
}

/// Update passphrase hint
#[tauri::command]
pub async fn update_passphrase_hint(
    identity_service: State<'_, Arc<IdentityService>>,
    hint: Option<String>,
) -> Result<(), AppError> {
    identity_service.update_passphrase_hint(hint.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn registered_identity() -> (
        tempfile::TempDir,
        Arc<Database>,
        IdentityService,
        AccountsService,
        IdentityInfo,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let database = Arc::new(Database::new(temp.path().join("harbor.db")).unwrap());
        let identity_service = IdentityService::new(database.clone());
        let identity = identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Alice".into(),
                passphrase: "test-password".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        let accounts_service = AccountsService::new(temp.path().to_path_buf());
        accounts_service
            .register_account(
                identity.peer_id.clone(),
                identity.display_name.clone(),
                identity.bio.clone(),
                identity.avatar_hash.clone(),
            )
            .unwrap();
        (temp, database, identity_service, accounts_service, identity)
    }

    #[test]
    fn successful_repository_absence_is_the_only_absent_state() {
        let temp = tempfile::tempdir().unwrap();
        let identity = IdentityService::new(Arc::new(
            Database::new(temp.path().join("harbor.db")).unwrap(),
        ));
        let accounts = AccountsService::new(temp.path().to_path_buf());

        assert!(matches!(
            identity_initialization_state(&identity, &accounts),
            IdentityInitializationState::Absent
        ));
    }

    #[test]
    fn initialization_repairs_registry_after_interrupted_avatar_commit() {
        let (_temp, _database, identity_service, accounts_service, identity) =
            registered_identity();
        let hash = "a".repeat(64);
        identity_service
            .replace_avatar(Some(&hash), Some("image/png"))
            .unwrap();
        assert_ne!(
            accounts_service
                .get_account(&identity.peer_id)
                .unwrap()
                .unwrap()
                .avatar_hash
                .as_deref(),
            Some(hash.as_str())
        );

        assert!(matches!(
            identity_initialization_state(&identity_service, &accounts_service),
            IdentityInitializationState::Unlocked { .. }
        ));
        assert_eq!(
            accounts_service
                .get_account(&identity.peer_id)
                .unwrap()
                .unwrap()
                .avatar_hash
                .as_deref(),
            Some(hash.as_str())
        );
    }

    #[test]
    fn startup_failure_overrides_empty_recovery_services() {
        let temp = tempfile::tempdir().unwrap();
        let identity = IdentityService::new(Arc::new(Database::in_memory().unwrap()));
        let accounts = AccountsService::new(temp.path().to_path_buf());
        let startup_failure = initialization_failure(
            IdentityInitializationFailureSource::AccountRegistry,
            AppError::InvalidData("active profile registry is corrupt".into()),
        );

        match effective_initialization_state(&identity, &accounts, Some(&startup_failure)) {
            IdentityInitializationState::FatalError { source, error } => {
                assert_eq!(source, IdentityInitializationFailureSource::AccountRegistry);
                assert!(error
                    .details
                    .as_deref()
                    .is_some_and(|details| details.contains("registry is corrupt")));
            }
            state => panic!("expected authoritative startup failure, got {state:?}"),
        }
    }

    #[test]
    fn registered_account_without_an_active_selection_is_not_absence() {
        let temp = tempfile::tempdir().unwrap();
        let identity = IdentityService::new(Arc::new(
            Database::new(temp.path().join("harbor.db")).unwrap(),
        ));
        let accounts = AccountsService::new(temp.path().to_path_buf());
        accounts
            .register_account("orphan-peer".into(), "Orphan".into(), None, None)
            .unwrap();
        let mut registry = accounts.load_registry().unwrap();
        registry.active_account_id = None;
        accounts.save_registry(&registry).unwrap();

        match identity_initialization_state(&identity, &accounts) {
            IdentityInitializationState::FatalError { source, .. } => {
                assert_eq!(source, IdentityInitializationFailureSource::AccountRegistry)
            }
            state => panic!("expected fatal registry state, got {state:?}"),
        }
    }

    #[test]
    fn initialization_reports_locked_and_unlocked_identity_explicitly() {
        let (_temp, _database, identity, accounts, expected) = registered_identity();

        match identity_initialization_state(&identity, &accounts) {
            IdentityInitializationState::Unlocked { identity } => {
                assert_eq!(identity.peer_id, expected.peer_id)
            }
            state => panic!("expected unlocked state, got {state:?}"),
        }

        identity.lock();
        match identity_initialization_state(&identity, &accounts) {
            IdentityInitializationState::Locked { identity } => {
                assert_eq!(identity.peer_id, expected.peer_id)
            }
            state => panic!("expected locked state, got {state:?}"),
        }
    }

    #[test]
    fn corrupt_identity_metadata_is_fatal_not_absent() {
        let (_temp, database, identity, accounts, _expected) = registered_identity();
        identity.lock();
        database
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE local_identity SET public_key = ?1 WHERE id = 1",
                    [vec![0u8; 31]],
                )?;
                Ok(())
            })
            .unwrap();

        match identity_initialization_state(&identity, &accounts) {
            IdentityInitializationState::FatalError { source, .. } => assert_eq!(
                source,
                IdentityInitializationFailureSource::IdentityCorruption
            ),
            state => panic!("expected fatal corruption state, got {state:?}"),
        }
    }

    #[test]
    fn registry_parse_failure_is_fatal_not_absent() {
        let (temp, _database, identity, accounts, _expected) = registered_identity();
        std::fs::write(temp.path().join("accounts.json"), b"not valid json").unwrap();

        match identity_initialization_state(&identity, &accounts) {
            IdentityInitializationState::FatalError { source, .. } => {
                assert_eq!(source, IdentityInitializationFailureSource::AccountRegistry)
            }
            state => panic!("expected fatal registry state, got {state:?}"),
        }
    }

    #[test]
    fn permission_and_transient_database_failures_are_recoverable() {
        assert!(initialization_error_is_recoverable(&AppError::Io(
            std::io::Error::from(std::io::ErrorKind::PermissionDenied)
        )));
        assert!(initialization_error_is_recoverable(
            &AppError::PermissionDenied("registry is temporarily unreadable".into())
        ));
        assert!(!initialization_error_is_recoverable(
            &AppError::InvalidData("corrupt identity".into())
        ));

        match startup_initialization_failure(
            IdentityInitializationFailureSource::AccountRegistry,
            AppError::Io(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        ) {
            IdentityInitializationState::FatalError { error, .. } => assert!(error
                .recovery
                .as_deref()
                .is_some_and(|recovery| recovery.contains("restart Harbor"))),
            state => panic!("startup failure must require restart, got {state:?}"),
        }
    }

    #[test]
    fn initialization_response_uses_stable_camel_case_tags() {
        let state = initialization_failure(
            IdentityInitializationFailureSource::AccountRegistry,
            AppError::InvalidData("corrupt registry".into()),
        );
        let json = serde_json::to_value(state).unwrap();
        assert_eq!(json["status"], "fatalError");
        assert_eq!(json["source"], "accountRegistry");
        assert_eq!(json["error"]["code"], "INVALID_DATA");
    }

    #[test]
    fn registry_failure_rolls_back_identity_creation() {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = IdentityService::new(db);
        let request = CreateIdentityRequest {
            display_name: "Alice".into(),
            passphrase: "test password".into(),
            bio: None,
            passphrase_hint: None,
        };
        let identity = identity_service.create_identity(request).unwrap();
        assert!(identity_service.has_identity().unwrap());
        assert!(identity_service.is_unlocked());

        let temp = tempfile::tempdir().unwrap();
        let invalid_data_root = temp.path().join("not-a-directory");
        std::fs::write(&invalid_data_root, b"file").unwrap();
        let accounts_service = AccountsService::new(invalid_data_root);

        let result = register_created_identity(
            &identity_service,
            &accounts_service,
            &identity,
            "Alice".into(),
            None,
        );
        assert!(result.is_err());
        assert!(!identity_service.has_identity().unwrap());
        assert!(!identity_service.is_unlocked());
    }

    #[tokio::test]
    async fn password_work_concurrency_is_bounded() {
        let limit = Arc::new(Semaphore::new(2));
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();

        for _ in 0..8 {
            let limit = limit.clone();
            let active = active.clone();
            let maximum = maximum.clone();
            tasks.push(tokio::spawn(async move {
                run_password_work_with_limit(limit, move || {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    maximum.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(())
                })
                .await
            }));
        }

        for task in tasks {
            task.await.unwrap().unwrap();
        }
        assert_eq!(maximum.load(Ordering::SeqCst), 2);
    }
}

/// Get the local peer ID
#[tauri::command]
pub async fn get_peer_id(
    identity_service: State<'_, Arc<IdentityService>>,
) -> Result<String, AppError> {
    identity_service.get_peer_id()
}
