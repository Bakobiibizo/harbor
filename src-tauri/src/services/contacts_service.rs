//! Contacts service for managing peer relationships

use crate::db::{Contact, ContactData, ContactsRepository, Database};
use crate::error::{AppError, Result};
use crate::services::IdentityService;
use std::sync::Arc;

/// Service for managing contacts
pub struct ContactsService {
    db: Arc<Database>,
    identity_service: Arc<IdentityService>,
}

impl ContactsService {
    /// Create a new contacts service
    pub fn new(db: Arc<Database>, identity_service: Arc<IdentityService>) -> Self {
        Self {
            db,
            identity_service,
        }
    }

    /// Add a new contact from identity exchange data
    pub fn add_contact(
        &self,
        peer_id: &str,
        public_key: &[u8],
        x25519_public: &[u8],
        display_name: &str,
        avatar_hash: Option<&str>,
        bio: Option<&str>,
    ) -> Result<i64> {
        // Don't add ourselves as a contact
        if let Some(identity) = self.identity_service.get_identity()? {
            if identity.peer_id == peer_id {
                return Err(AppError::Validation(
                    "Cannot add self as contact".to_string(),
                ));
            }
        }

        // Check if already a contact
        if ContactsRepository::is_contact(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
        {
            // Update existing contact info instead
            ContactsRepository::update_contact_info(
                &self.db,
                peer_id,
                display_name,
                avatar_hash,
                bio,
            )
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

            // Return existing contact's ID
            let contact = ContactsRepository::get_by_peer_id(&self.db, peer_id)
                .map_err(|e| AppError::DatabaseString(e.to_string()))?
                .ok_or_else(|| AppError::NotFound("Contact not found".to_string()))?;
            return Ok(contact.id);
        }

        let contact_data = ContactData {
            peer_id: peer_id.to_string(),
            public_key: public_key.to_vec(),
            x25519_public: x25519_public.to_vec(),
            display_name: display_name.to_string(),
            avatar_hash: avatar_hash.map(String::from),
            bio: bio.map(String::from),
        };

        ContactsRepository::add_contact(&self.db, &contact_data)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get a contact by peer ID
    pub fn get_contact(&self, peer_id: &str) -> Result<Option<Contact>> {
        ContactsRepository::get_by_peer_id(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get all contacts
    pub fn get_all_contacts(&self) -> Result<Vec<Contact>> {
        ContactsRepository::get_all(&self.db).map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get all non-blocked contacts
    pub fn get_active_contacts(&self) -> Result<Vec<Contact>> {
        ContactsRepository::get_active(&self.db)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Update contact info (from network)
    pub fn update_contact_info(
        &self,
        peer_id: &str,
        display_name: &str,
        avatar_hash: Option<&str>,
        bio: Option<&str>,
    ) -> Result<bool> {
        ContactsRepository::update_contact_info(&self.db, peer_id, display_name, avatar_hash, bio)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Update last seen timestamp for a contact
    pub fn update_last_seen(&self, peer_id: &str) -> Result<bool> {
        ContactsRepository::update_last_seen(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Block a contact
    pub fn block_contact(&self, peer_id: &str) -> Result<bool> {
        ContactsRepository::block_contact(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Unblock a contact
    pub fn unblock_contact(&self, peer_id: &str) -> Result<bool> {
        ContactsRepository::unblock_contact(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Remove a contact
    pub fn remove_contact(&self, peer_id: &str) -> Result<bool> {
        ContactsRepository::remove_contact(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Check if peer is a contact
    pub fn is_contact(&self, peer_id: &str) -> Result<bool> {
        ContactsRepository::is_contact(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Check if peer is blocked
    pub fn is_blocked(&self, peer_id: &str) -> Result<bool> {
        ContactsRepository::is_blocked(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get X25519 public key for a contact (needed for encryption)
    pub fn get_x25519_public(&self, peer_id: &str) -> Result<Option<Vec<u8>>> {
        let contact = self.get_contact(peer_id)?;
        Ok(contact.map(|c| c.x25519_public))
    }

    /// Get Ed25519 public key for a contact (needed for signature verification)
    pub fn get_public_key(&self, peer_id: &str) -> Result<Option<Vec<u8>>> {
        let contact = self.get_contact(peer_id)?;
        Ok(contact.map(|c| c.public_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn create_test_services() -> (Arc<Database>, Arc<IdentityService>, ContactsService) {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = ContactsService::new(db.clone(), identity_service.clone());
        (db, identity_service, contacts_service)
    }

    #[test]
    fn test_add_and_get_contact() {
        let (_, _, service) = create_test_services();

        let id = service
            .add_contact(
                "12D3KooWTest",
                &[1, 2, 3, 4],
                &[5, 6, 7, 8],
                "Test User",
                None,
                Some("Hello!"),
            )
            .unwrap();

        assert!(id > 0);

        let contact = service.get_contact("12D3KooWTest").unwrap().unwrap();
        assert_eq!(contact.display_name, "Test User");
        assert_eq!(contact.bio, Some("Hello!".to_string()));
    }

    #[test]
    fn test_block_contact() {
        let (_, _, service) = create_test_services();

        service
            .add_contact(
                "12D3KooWTest",
                &[1, 2, 3, 4],
                &[5, 6, 7, 8],
                "Test User",
                None,
                None,
            )
            .unwrap();

        assert!(!service.is_blocked("12D3KooWTest").unwrap());

        service.block_contact("12D3KooWTest").unwrap();
        assert!(service.is_blocked("12D3KooWTest").unwrap());

        // Blocked contacts shouldn't appear in active list
        let active = service.get_active_contacts().unwrap();
        assert!(active.is_empty());
    }
}
