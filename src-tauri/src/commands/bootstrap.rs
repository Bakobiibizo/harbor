use crate::db::repositories::{AddBootstrapNodeInput, BootstrapNodeConfig, BootstrapNodesRepo};
use crate::db::Database;
use crate::error::AppError;
use std::sync::Arc;
use tauri::State;

/// Get all configured bootstrap nodes
#[tauri::command]
pub async fn get_bootstrap_nodes(
    db: State<'_, Arc<Database>>,
    enabled_only: Option<bool>,
) -> Result<Vec<BootstrapNodeConfig>, AppError> {
    let enabled = enabled_only.unwrap_or(false);
    BootstrapNodesRepo::get_all(&db, enabled).map_err(AppError::Database)
}

/// Add a new bootstrap node
#[tauri::command]
pub async fn add_bootstrap_node_config(
    db: State<'_, Arc<Database>>,
    address: String,
    name: Option<String>,
    priority: Option<i32>,
) -> Result<i64, AppError> {
    // Validate the multiaddress format
    let _: libp2p::Multiaddr = address
        .parse()
        .map_err(|e| AppError::Validation(format!("Invalid multiaddress: {}", e)))?;

    // Check if it already exists
    if BootstrapNodesRepo::exists(&db, &address).map_err(AppError::Database)? {
        return Err(AppError::Validation(
            "Bootstrap node with this address already exists".to_string(),
        ));
    }

    let input = AddBootstrapNodeInput {
        address,
        name,
        priority,
        is_default: Some(false),
    };

    BootstrapNodesRepo::add(&db, input).map_err(AppError::Database)
}

/// Update a bootstrap node configuration
#[tauri::command]
pub async fn update_bootstrap_node(
    db: State<'_, Arc<Database>>,
    id: i64,
    name: Option<String>,
    is_enabled: Option<bool>,
    priority: Option<i32>,
) -> Result<bool, AppError> {
    BootstrapNodesRepo::update(&db, id, name, is_enabled, priority).map_err(AppError::Database)
}

/// Remove a bootstrap node (only non-default nodes can be removed)
#[tauri::command]
pub async fn remove_bootstrap_node(
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<bool, AppError> {
    BootstrapNodesRepo::remove(&db, id).map_err(AppError::Database)
}

/// Get list of enabled bootstrap node addresses in priority order
#[tauri::command]
pub async fn get_enabled_bootstrap_addresses(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<String>, AppError> {
    BootstrapNodesRepo::get_enabled_addresses(&db).map_err(AppError::Database)
}
