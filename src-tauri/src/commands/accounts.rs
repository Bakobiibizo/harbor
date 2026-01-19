use crate::error::AppError;
use crate::services::accounts_service::AccountInfo;
use crate::services::AccountsService;
use std::sync::Arc;
use tauri::State;

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
    account_id: String,
) -> Result<AccountInfo, AppError> {
    accounts_service.set_active_account(&account_id)
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
