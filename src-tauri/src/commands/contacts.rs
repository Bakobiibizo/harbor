//! Tauri commands for contact management

use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tauri::State;
use tracing::info;

use crate::commands::network::NetworkState;
use crate::db::{Capability, ContactData};
use crate::error::AppError;
use crate::services::{ContactRevocationAction, ContactsService, PermissionsService};

/// Contact info for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactInfo {
    pub id: i64,
    pub peer_id: String,
    pub display_name: String,
    pub verified_qualified_name: Option<String>,
    pub avatar_hash: Option<String>,
    pub bio: Option<String>,
    pub is_blocked: bool,
    pub trust_level: i32,
    pub last_seen_at: Option<i64>,
    pub added_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactRequestInfo {
    pub request_id: String,
    pub peer_id: String,
    pub direction: String,
    pub display_name: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactDecisionResult {
    pub peer_id: String,
    pub status: String,
    /// `connected` means the peer was online when the signed action was queued;
    /// `offline` is nonfatal because the complete accepted state is durable.
    pub delivery: String,
}

fn request_info(value: crate::db::ContactRequestRecord) -> ContactRequestInfo {
    ContactRequestInfo {
        request_id: value.request_id,
        peer_id: value.peer_id,
        direction: value.direction,
        display_name: value.display_name,
        status: value.status,
        error: value.error,
        created_at: value.created_at,
        updated_at: value.updated_at,
    }
}

fn contact_info(
    contacts_service: &ContactsService,
    contact: crate::db::Contact,
) -> Result<ContactInfo, AppError> {
    Ok(ContactInfo {
        verified_qualified_name: contacts_service.verified_qualified_name(&contact.peer_id)?,
        id: contact.id,
        peer_id: contact.peer_id,
        display_name: contact.display_name,
        avatar_hash: contact.avatar_hash,
        bio: contact.bio,
        is_blocked: contact.is_blocked,
        trust_level: contact.trust_level,
        last_seen_at: contact.last_seen_at,
        added_at: contact.added_at,
    })
}

#[tauri::command]
pub async fn get_contact_requests(
    contacts_service: State<'_, Arc<ContactsService>>,
) -> Result<Vec<ContactRequestInfo>, AppError> {
    Ok(contacts_service
        .contact_requests()?
        .into_iter()
        .map(request_info)
        .collect())
}

/// Get all contacts
#[tauri::command]
pub async fn get_contacts(
    contacts_service: State<'_, Arc<ContactsService>>,
) -> Result<Vec<ContactInfo>, AppError> {
    let contacts = contacts_service.get_all_contacts()?;
    contacts
        .into_iter()
        .map(|contact| contact_info(&contacts_service, contact))
        .collect()
}

/// Get active (non-blocked) contacts
#[tauri::command]
pub async fn get_active_contacts(
    contacts_service: State<'_, Arc<ContactsService>>,
) -> Result<Vec<ContactInfo>, AppError> {
    let contacts = contacts_service.get_active_contacts()?;
    contacts
        .into_iter()
        .map(|contact| contact_info(&contacts_service, contact))
        .collect()
}

/// Get a single contact by peer ID
#[tauri::command]
pub async fn get_contact(
    contacts_service: State<'_, Arc<ContactsService>>,
    peer_id: String,
) -> Result<Option<ContactInfo>, AppError> {
    let contact = contacts_service.get_contact(&peer_id)?;
    contact
        .map(|contact| contact_info(&contacts_service, contact))
        .transpose()
}

/// Add a new contact
fn reject_unsigned_direct_add() -> Result<i64, AppError> {
    Err(AppError::Validation(
        "Unsigned direct contact creation is disabled. Use a Harbor v1 invite and complete the signed contact handshake."
            .into(),
    ))
}

#[tauri::command]
pub async fn add_contact(
    contacts_service: State<'_, Arc<ContactsService>>,
    peer_id: String,
    public_key: Vec<u8>,
    x25519_public: Vec<u8>,
    display_name: String,
    avatar_hash: Option<String>,
    bio: Option<String>,
) -> Result<i64, AppError> {
    let _ = (
        contacts_service,
        peer_id,
        public_key,
        x25519_public,
        display_name,
        avatar_hash,
        bio,
    );
    reject_unsigned_direct_add()
}

/// Block a contact
#[tauri::command]
pub async fn block_contact(
    contacts_service: State<'_, Arc<ContactsService>>,
    permissions_service: State<'_, Arc<PermissionsService>>,
    network: State<'_, NetworkState>,
    peer_id: String,
) -> Result<bool, AppError> {
    let revocations = permissions_service.prepare_contact_revocations(&peer_id)?;
    contacts_service.revoke_contact_atomically(
        &peer_id,
        ContactRevocationAction::Blocked,
        &revocations,
        chrono::Utc::now().timestamp(),
    )?;
    if let (Ok(peer), Ok(handle)) = (PeerId::from_str(&peer_id), network.get_handle().await) {
        let _ = handle.disconnect(peer).await;
    }
    Ok(true)
}

/// Unblock a contact
#[tauri::command]
pub async fn unblock_contact(
    contacts_service: State<'_, Arc<ContactsService>>,
    peer_id: String,
) -> Result<bool, AppError> {
    contacts_service.unblock_contact(&peer_id)
}

/// Remove a contact
#[tauri::command]
pub async fn remove_contact(
    contacts_service: State<'_, Arc<ContactsService>>,
    permissions_service: State<'_, Arc<PermissionsService>>,
    network: State<'_, NetworkState>,
    peer_id: String,
) -> Result<bool, AppError> {
    let revocations = permissions_service.prepare_contact_revocations(&peer_id)?;
    contacts_service.revoke_contact_atomically(
        &peer_id,
        ContactRevocationAction::Removed,
        &revocations,
        chrono::Utc::now().timestamp(),
    )?;
    if let (Ok(peer), Ok(handle)) = (PeerId::from_str(&peer_id), network.get_handle().await) {
        let _ = handle.disconnect(peer).await;
    }
    Ok(true)
}

/// Check if a peer is a contact
#[tauri::command]
pub async fn is_contact(
    contacts_service: State<'_, Arc<ContactsService>>,
    peer_id: String,
) -> Result<bool, AppError> {
    contacts_service.is_contact(&peer_id)
}

/// Check if a contact is blocked
#[tauri::command]
pub async fn is_contact_blocked(
    contacts_service: State<'_, Arc<ContactsService>>,
    peer_id: String,
) -> Result<bool, AppError> {
    contacts_service.is_blocked(&peer_id)
}

/// Request identity exchange with a peer (adds them as a contact)
#[tauri::command]
pub async fn request_peer_identity(
    network: State<'_, NetworkState>,
    contacts_service: State<'_, Arc<ContactsService>>,
    peer_id: String,
) -> Result<String, AppError> {
    let libp2p_peer_id = PeerId::from_str(&peer_id)
        .map_err(|e| AppError::Validation(format!("Invalid peer ID: {}", e)))?;

    let request_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    contacts_service.record_contact_request(
        &request_id,
        &peer_id,
        "outgoing",
        None,
        None,
        None,
        None,
        None,
        "pending",
        Some("request"),
        None,
        now,
    )?;
    let delivery = match network.get_handle().await {
        Ok(handle) => {
            handle
                .request_identity_action(libp2p_peer_id, request_id.clone(), "request".into())
                .await
        }
        Err(error) => Err(error),
    };
    if let Err(error) = delivery {
        contacts_service.update_contact_request(
            &request_id,
            "failed",
            Some("request"),
            Some(&error.to_string()),
            chrono::Utc::now().timestamp(),
        )?;
        return Err(error);
    }

    info!("Requested identity from peer {}", peer_id);
    Ok(request_id)
}

#[tauri::command]
pub async fn respond_contact_request(
    network: State<'_, NetworkState>,
    contacts_service: State<'_, Arc<ContactsService>>,
    permissions_service: State<'_, Arc<PermissionsService>>,
    request_id: String,
    decision: String,
) -> Result<ContactDecisionResult, AppError> {
    if decision != "accepted" && decision != "declined" {
        return Err(AppError::Validation(
            "Invalid contact request decision".into(),
        ));
    }
    let request = contacts_service
        .contact_request(&request_id)?
        .ok_or_else(|| AppError::NotFound("Contact request not found".into()))?;
    if request.direction != "incoming" || !matches!(request.status.as_str(), "review" | "failed") {
        return Err(AppError::Validation(
            "Contact request is no longer awaiting review".into(),
        ));
    }

    let now = chrono::Utc::now().timestamp();
    let committed_grants = if decision == "accepted" {
        let public_key = request.public_key.clone().ok_or_else(|| {
            AppError::Validation("Contact request has no verified signing key".into())
        })?;
        let x25519_public = request.x25519_public.clone().ok_or_else(|| {
            AppError::Validation("Contact request has no verified encryption key".into())
        })?;
        let grants = permissions_service.prepare_permission_grants(
            &request.peer_id,
            &[Capability::Chat, Capability::WallRead, Capability::Call],
        )?;
        contacts_service.accept_contact_request_atomically(
            &request_id,
            "incoming",
            &ContactData {
                peer_id: request.peer_id.clone(),
                public_key,
                x25519_public,
                display_name: request
                    .display_name
                    .clone()
                    .unwrap_or_else(|| "Harbor contact".into()),
                avatar_hash: request.avatar_hash.clone(),
                bio: request.bio.clone(),
            },
            &grants,
            now,
        )?;
        Some(grants)
    } else {
        contacts_service.update_contact_request(
            &request_id,
            &decision,
            Some(&decision),
            None,
            now,
        )?;
        None
    };
    let peer = PeerId::from_str(&request.peer_id)
        .map_err(|error| AppError::Validation(format!("Invalid peer ID: {error}")))?;
    let delivery = match (network.get_handle().await, committed_grants) {
        (Ok(handle), Some(grants)) => {
            handle
                .request_identity_action_with_grants(
                    peer,
                    request_id.clone(),
                    decision.clone(),
                    grants,
                )
                .await
        }
        (Ok(handle), None) => handle
            .request_identity_action(peer, request_id.clone(), decision.clone())
            .await
            .map(|_| true),
        (Err(error), _) => Err(error),
    };
    if decision == "accepted" {
        let (delivery_status, error) = match delivery {
            Ok(true) => ("connected", None),
            Ok(false) => ("offline", None),
            Err(error) => ("offline", Some(error.to_string())),
        };
        if let Some(error) = error.as_deref() {
            contacts_service.update_contact_request(
                &request_id,
                "accepted",
                Some("accepted"),
                Some(error),
                chrono::Utc::now().timestamp(),
            )?;
        }
        return Ok(ContactDecisionResult {
            peer_id: request.peer_id,
            status: "accepted".into(),
            delivery: delivery_status.into(),
        });
    }
    if let Err(error) = delivery {
        contacts_service.update_contact_request(
            &request_id,
            "failed",
            Some(&decision),
            Some(&error.to_string()),
            chrono::Utc::now().timestamp(),
        )?;
        return Err(error);
    }
    Ok(ContactDecisionResult {
        peer_id: request.peer_id,
        status: decision,
        delivery: "connected".into(),
    })
}

#[tauri::command]
pub async fn retry_contact_request(
    network: State<'_, NetworkState>,
    contacts_service: State<'_, Arc<ContactsService>>,
    request_id: String,
) -> Result<(), AppError> {
    let request = contacts_service
        .contact_request(&request_id)?
        .ok_or_else(|| AppError::NotFound("Contact request not found".into()))?;
    if request.status != "failed" {
        return Err(AppError::Validation(
            "Only failed contact requests can be retried".into(),
        ));
    }
    let action = request.pending_action.unwrap_or_else(|| "request".into());
    let next_status = if action == "request" {
        "pending"
    } else {
        &action
    };
    contacts_service.update_contact_request(
        &request_id,
        next_status,
        Some(&action),
        None,
        chrono::Utc::now().timestamp(),
    )?;
    let peer = PeerId::from_str(&request.peer_id)
        .map_err(|error| AppError::Validation(format!("Invalid peer ID: {error}")))?;
    let delivery = match network.get_handle().await {
        Ok(handle) => {
            handle
                .request_identity_action(peer, request_id.clone(), action.clone())
                .await
        }
        Err(error) => Err(error),
    };
    if let Err(error) = delivery {
        contacts_service.update_contact_request(
            &request_id,
            "failed",
            Some(&action),
            Some(&error.to_string()),
            chrono::Utc::now().timestamp(),
        )?;
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod unsigned_direct_add_tests {
    use super::reject_unsigned_direct_add;

    #[test]
    fn unsigned_direct_add_is_rejected() {
        let error = reject_unsigned_direct_add().unwrap_err();
        assert!(error.to_string().contains("signed contact handshake"));
    }
}
