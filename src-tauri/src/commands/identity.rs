use crate::error::AppError;
use crate::models::{CreateIdentityRequest, IdentityInfo};
use crate::services::{AccountsService, IdentityService};
use std::sync::Arc;
use tauri::State;
use tracing::info;

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

/// Create a new identity
#[tauri::command]
pub async fn create_identity(
    identity_service: State<'_, Arc<IdentityService>>,
    accounts_service: State<'_, Arc<AccountsService>>,
    request: CreateIdentityRequest,
) -> Result<IdentityInfo, AppError> {
    let display_name = request.display_name.clone();
    let bio = request.bio.clone();

    let identity = identity_service.create_identity(request)?;

    // Register the new identity in the accounts registry
    match accounts_service.register_account(
        identity.peer_id.clone(),
        display_name,
        bio,
        identity.avatar_hash.clone(),
    ) {
        Ok(account) => {
            info!("Registered new account in registry: {}", account.id);
        }
        Err(e) => {
            // Don't fail identity creation if account registration fails
            info!("Could not register account (may already exist): {}", e);
        }
    }

    Ok(identity)
}

/// Unlock the identity with passphrase
#[tauri::command]
pub async fn unlock_identity(
    identity_service: State<'_, Arc<IdentityService>>,
    passphrase: String,
) -> Result<IdentityInfo, AppError> {
    identity_service.unlock(&passphrase)
}

/// Lock the identity
#[tauri::command]
pub async fn lock_identity(
    identity_service: State<'_, Arc<IdentityService>>,
) -> Result<(), AppError> {
    identity_service.lock();
    Ok(())
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

/// Update passphrase hint
#[tauri::command]
pub async fn update_passphrase_hint(
    identity_service: State<'_, Arc<IdentityService>>,
    hint: Option<String>,
) -> Result<(), AppError> {
    identity_service.update_passphrase_hint(hint.as_deref())
}

/// Get the local peer ID
#[tauri::command]
pub async fn get_peer_id(
    identity_service: State<'_, Arc<IdentityService>>,
) -> Result<String, AppError> {
    identity_service.get_peer_id()
}
