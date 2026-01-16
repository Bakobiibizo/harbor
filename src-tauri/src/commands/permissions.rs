//! Tauri commands for permission management

use serde::{Deserialize, Serialize};
use tauri::State;
use std::sync::Arc;

use crate::db::Capability;
use crate::error::AppError;
use crate::services::PermissionsService;

/// Permission info for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionInfo {
    pub grant_id: String,
    pub issuer_peer_id: String,
    pub subject_peer_id: String,
    pub capability: String,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
    pub is_valid: bool,
}

/// Permission grant result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantResult {
    pub grant_id: String,
    pub capability: String,
    pub subject_peer_id: String,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
}

fn capability_from_str(s: &str) -> Result<Capability, AppError> {
    Capability::from_str(s)
        .ok_or_else(|| AppError::Validation(format!("Invalid capability: {}", s)))
}

/// Grant a permission to another peer
#[tauri::command]
pub async fn grant_permission(
    permissions_service: State<'_, Arc<PermissionsService>>,
    subject_peer_id: String,
    capability: String,
    expires_in_seconds: Option<i64>,
) -> Result<GrantResult, AppError> {
    let cap = capability_from_str(&capability)?;
    let grant = permissions_service.create_permission_grant(
        &subject_peer_id,
        cap,
        expires_in_seconds,
    )?;

    Ok(GrantResult {
        grant_id: grant.grant_id,
        capability: grant.capability,
        subject_peer_id: grant.subject_peer_id,
        issued_at: grant.issued_at,
        expires_at: grant.expires_at,
    })
}

/// Revoke a permission
#[tauri::command]
pub async fn revoke_permission(
    permissions_service: State<'_, Arc<PermissionsService>>,
    grant_id: String,
) -> Result<bool, AppError> {
    permissions_service.revoke_permission(&grant_id)?;
    Ok(true)
}

/// Check if a peer has a specific capability (we granted it to them)
#[tauri::command]
pub async fn peer_has_capability(
    permissions_service: State<'_, Arc<PermissionsService>>,
    peer_id: String,
    capability: String,
) -> Result<bool, AppError> {
    let cap = capability_from_str(&capability)?;
    permissions_service.peer_has_capability(&peer_id, cap)
}

/// Check if we have a specific capability from another peer
#[tauri::command]
pub async fn we_have_capability(
    permissions_service: State<'_, Arc<PermissionsService>>,
    issuer_peer_id: String,
    capability: String,
) -> Result<bool, AppError> {
    let cap = capability_from_str(&capability)?;
    permissions_service.we_have_capability(&issuer_peer_id, cap)
}

/// Get all permissions we've granted
#[tauri::command]
pub async fn get_granted_permissions(
    permissions_service: State<'_, Arc<PermissionsService>>,
) -> Result<Vec<PermissionInfo>, AppError> {
    let perms = permissions_service.get_granted_permissions()?;
    Ok(perms.into_iter().map(|p| {
        let is_valid = p.is_valid();
        PermissionInfo {
            grant_id: p.grant_id,
            issuer_peer_id: p.issuer_peer_id,
            subject_peer_id: p.subject_peer_id,
            capability: p.capability,
            issued_at: p.issued_at,
            expires_at: p.expires_at,
            is_valid,
        }
    }).collect())
}

/// Get all permissions granted to us
#[tauri::command]
pub async fn get_received_permissions(
    permissions_service: State<'_, Arc<PermissionsService>>,
) -> Result<Vec<PermissionInfo>, AppError> {
    let perms = permissions_service.get_received_permissions()?;
    Ok(perms.into_iter().map(|p| {
        let is_valid = p.is_valid();
        PermissionInfo {
            grant_id: p.grant_id,
            issuer_peer_id: p.issuer_peer_id,
            subject_peer_id: p.subject_peer_id,
            capability: p.capability,
            issued_at: p.issued_at,
            expires_at: p.expires_at,
            is_valid,
        }
    }).collect())
}

/// Get all peers we can chat with
#[tauri::command]
pub async fn get_chat_peers(
    permissions_service: State<'_, Arc<PermissionsService>>,
) -> Result<Vec<String>, AppError> {
    permissions_service.get_chat_peers()
}

/// Grant all standard permissions to a peer (chat, wall_read, call)
#[tauri::command]
pub async fn grant_all_permissions(
    permissions_service: State<'_, Arc<PermissionsService>>,
    subject_peer_id: String,
) -> Result<Vec<GrantResult>, AppError> {
    let mut results = Vec::new();

    for cap in [Capability::Chat, Capability::WallRead, Capability::Call] {
        let grant = permissions_service.create_permission_grant(
            &subject_peer_id,
            cap,
            None,
        )?;

        results.push(GrantResult {
            grant_id: grant.grant_id,
            capability: grant.capability,
            subject_peer_id: grant.subject_peer_id.clone(),
            issued_at: grant.issued_at,
            expires_at: grant.expires_at,
        });
    }

    Ok(results)
}
