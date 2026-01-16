//! Tauri commands for contact management

use serde::{Deserialize, Serialize};
use tauri::State;
use std::sync::Arc;

use crate::error::AppError;
use crate::services::ContactsService;

/// Contact info for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactInfo {
    pub id: i64,
    pub peer_id: String,
    pub display_name: String,
    pub avatar_hash: Option<String>,
    pub bio: Option<String>,
    pub is_blocked: bool,
    pub trust_level: i32,
    pub last_seen_at: Option<i64>,
    pub added_at: i64,
}

/// Get all contacts
#[tauri::command]
pub async fn get_contacts(
    contacts_service: State<'_, Arc<ContactsService>>,
) -> Result<Vec<ContactInfo>, AppError> {
    let contacts = contacts_service.get_all_contacts()?;
    Ok(contacts.into_iter().map(|c| ContactInfo {
        id: c.id,
        peer_id: c.peer_id,
        display_name: c.display_name,
        avatar_hash: c.avatar_hash,
        bio: c.bio,
        is_blocked: c.is_blocked,
        trust_level: c.trust_level,
        last_seen_at: c.last_seen_at,
        added_at: c.added_at,
    }).collect())
}

/// Get active (non-blocked) contacts
#[tauri::command]
pub async fn get_active_contacts(
    contacts_service: State<'_, Arc<ContactsService>>,
) -> Result<Vec<ContactInfo>, AppError> {
    let contacts = contacts_service.get_active_contacts()?;
    Ok(contacts.into_iter().map(|c| ContactInfo {
        id: c.id,
        peer_id: c.peer_id,
        display_name: c.display_name,
        avatar_hash: c.avatar_hash,
        bio: c.bio,
        is_blocked: c.is_blocked,
        trust_level: c.trust_level,
        last_seen_at: c.last_seen_at,
        added_at: c.added_at,
    }).collect())
}

/// Get a single contact by peer ID
#[tauri::command]
pub async fn get_contact(
    contacts_service: State<'_, Arc<ContactsService>>,
    peer_id: String,
) -> Result<Option<ContactInfo>, AppError> {
    let contact = contacts_service.get_contact(&peer_id)?;
    Ok(contact.map(|c| ContactInfo {
        id: c.id,
        peer_id: c.peer_id,
        display_name: c.display_name,
        avatar_hash: c.avatar_hash,
        bio: c.bio,
        is_blocked: c.is_blocked,
        trust_level: c.trust_level,
        last_seen_at: c.last_seen_at,
        added_at: c.added_at,
    }))
}

/// Add a new contact
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
    contacts_service.add_contact(
        &peer_id,
        &public_key,
        &x25519_public,
        &display_name,
        avatar_hash.as_deref(),
        bio.as_deref(),
    )
}

/// Block a contact
#[tauri::command]
pub async fn block_contact(
    contacts_service: State<'_, Arc<ContactsService>>,
    peer_id: String,
) -> Result<bool, AppError> {
    contacts_service.block_contact(&peer_id)
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
    peer_id: String,
) -> Result<bool, AppError> {
    contacts_service.remove_contact(&peer_id)
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
