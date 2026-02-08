//! Permissions service for managing capability grants

use ed25519_dalek::VerifyingKey;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::{
    Capability, Database, GrantData, Permission, PermissionsRepository,
    RecordPermissionEventParams,
};
use crate::error::{AppError, Result};
use crate::services::{
    verify, IdentityService, Signable, SignablePermissionGrant, SignablePermissionRequest,
    SignablePermissionRevoke,
};

/// Service for managing permissions
pub struct PermissionsService {
    db: Arc<Database>,
    identity_service: Arc<IdentityService>,
}

/// A permission request to send to another peer
#[derive(Debug, Clone)]
pub struct PermissionRequestMessage {
    pub request_id: String,
    pub requester_peer_id: String,
    pub capability: String,
    pub message: Option<String>,
    pub lamport_clock: u64,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// A permission grant message
#[derive(Debug, Clone)]
pub struct PermissionGrantMessage {
    pub grant_id: String,
    pub issuer_peer_id: String,
    pub subject_peer_id: String,
    pub capability: String,
    pub scope: Option<serde_json::Value>,
    pub lamport_clock: u64,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
    pub signature: Vec<u8>,
    pub payload_cbor: Vec<u8>,
}

/// A permission revoke message
#[derive(Debug, Clone)]
pub struct PermissionRevokeMessage {
    pub grant_id: String,
    pub issuer_peer_id: String,
    pub lamport_clock: u64,
    pub revoked_at: i64,
    pub signature: Vec<u8>,
}

impl PermissionsService {
    /// Create a new permissions service
    pub fn new(db: Arc<Database>, identity_service: Arc<IdentityService>) -> Self {
        Self {
            db,
            identity_service,
        }
    }

    // ============================================================
    // Creating Requests/Grants/Revokes (for sending)
    // ============================================================

    /// Create a permission request to send to another peer
    pub fn create_permission_request(
        &self,
        capability: Capability,
        message: Option<&str>,
    ) -> Result<PermissionRequestMessage> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        let request_id = Uuid::new_v4().to_string();
        let lamport_clock =
            self.db
                .next_lamport_clock(&identity.peer_id)
                .map_err(|e| AppError::DatabaseString(e.to_string()))? as u64;
        let timestamp = chrono::Utc::now().timestamp();

        let signable = SignablePermissionRequest {
            request_id: request_id.clone(),
            requester_peer_id: identity.peer_id.clone(),
            capability: capability.as_str().to_string(),
            message: message.map(String::from),
            lamport_clock,
            timestamp,
        };

        let signature = self.identity_service.sign(&signable)?;

        Ok(PermissionRequestMessage {
            request_id,
            requester_peer_id: identity.peer_id,
            capability: capability.as_str().to_string(),
            message: message.map(String::from),
            lamport_clock,
            timestamp,
            signature,
        })
    }

    /// Create a permission grant for another peer
    pub fn create_permission_grant(
        &self,
        subject_peer_id: &str,
        capability: Capability,
        expires_in_seconds: Option<i64>,
    ) -> Result<PermissionGrantMessage> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        let grant_id = Uuid::new_v4().to_string();
        let lamport_clock =
            self.db
                .next_lamport_clock(&identity.peer_id)
                .map_err(|e| AppError::DatabaseString(e.to_string()))? as u64;
        let issued_at = chrono::Utc::now().timestamp();
        let expires_at = expires_in_seconds.map(|s| issued_at + s);

        let signable = SignablePermissionGrant {
            grant_id: grant_id.clone(),
            issuer_peer_id: identity.peer_id.clone(),
            subject_peer_id: subject_peer_id.to_string(),
            capability: capability.as_str().to_string(),
            scope: None,
            lamport_clock,
            issued_at,
            expires_at,
        };

        // Get CBOR payload for storage
        let payload_cbor = signable.signable_bytes()?;
        let signature = self.identity_service.sign(&signable)?;

        // Store locally
        let grant_data = GrantData {
            grant_id: grant_id.clone(),
            issuer_peer_id: identity.peer_id.clone(),
            subject_peer_id: subject_peer_id.to_string(),
            capability: capability.as_str().to_string(),
            scope_json: None,
            lamport_clock: lamport_clock as i64,
            issued_at,
            expires_at,
            payload_cbor: payload_cbor.clone(),
            signature: signature.clone(),
        };

        PermissionsRepository::upsert_grant(&self.db, &grant_data)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Record event
        let event_id = Uuid::new_v4().to_string();
        PermissionsRepository::record_event(
            &self.db,
            &RecordPermissionEventParams {
                event_id: &event_id,
                event_type: "grant",
                entity_id: &grant_id,
                author_peer_id: &identity.peer_id,
                issuer_peer_id: Some(&identity.peer_id),
                subject_peer_id,
                capability: capability.as_str(),
                scope_json: None,
                lamport_clock: lamport_clock as i64,
                issued_at: Some(issued_at),
                expires_at,
                payload_cbor: &payload_cbor,
                signature: &signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(PermissionGrantMessage {
            grant_id,
            issuer_peer_id: identity.peer_id,
            subject_peer_id: subject_peer_id.to_string(),
            capability: capability.as_str().to_string(),
            scope: None,
            lamport_clock,
            issued_at,
            expires_at,
            signature,
            payload_cbor,
        })
    }

    /// Revoke a previously granted permission
    pub fn revoke_permission(&self, grant_id: &str) -> Result<PermissionRevokeMessage> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        // Verify we issued this grant
        let grant = PermissionsRepository::get_by_grant_id(&self.db, grant_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Grant not found".to_string()))?;

        if grant.issuer_peer_id != identity.peer_id {
            return Err(AppError::Unauthorized(
                "Not the issuer of this grant".to_string(),
            ));
        }

        let lamport_clock =
            self.db
                .next_lamport_clock(&identity.peer_id)
                .map_err(|e| AppError::DatabaseString(e.to_string()))? as u64;
        let revoked_at = chrono::Utc::now().timestamp();

        let signable = SignablePermissionRevoke {
            grant_id: grant_id.to_string(),
            issuer_peer_id: identity.peer_id.clone(),
            lamport_clock,
            revoked_at,
        };

        let signature = self.identity_service.sign(&signable)?;
        let payload_cbor = signable.signable_bytes()?;

        // Mark as revoked locally
        PermissionsRepository::revoke_grant(&self.db, grant_id, revoked_at)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Record event
        let event_id = Uuid::new_v4().to_string();
        PermissionsRepository::record_event(
            &self.db,
            &RecordPermissionEventParams {
                event_id: &event_id,
                event_type: "revoke",
                entity_id: grant_id,
                author_peer_id: &identity.peer_id,
                issuer_peer_id: Some(&identity.peer_id),
                subject_peer_id: &grant.subject_peer_id,
                capability: &grant.capability,
                scope_json: None,
                lamport_clock: lamport_clock as i64,
                issued_at: None,
                expires_at: None,
                payload_cbor: &payload_cbor,
                signature: &signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(PermissionRevokeMessage {
            grant_id: grant_id.to_string(),
            issuer_peer_id: identity.peer_id,
            lamport_clock,
            revoked_at,
            signature,
        })
    }

    // ============================================================
    // Processing Incoming Messages
    // ============================================================

    /// Verify and store a permission grant from the network
    pub fn process_incoming_grant(
        &self,
        grant: &PermissionGrantMessage,
        issuer_public_key: &[u8],
    ) -> Result<()> {
        // Verify signature
        let signable = SignablePermissionGrant {
            grant_id: grant.grant_id.clone(),
            issuer_peer_id: grant.issuer_peer_id.clone(),
            subject_peer_id: grant.subject_peer_id.clone(),
            capability: grant.capability.clone(),
            scope: grant.scope.clone(),
            lamport_clock: grant.lamport_clock,
            issued_at: grant.issued_at,
            expires_at: grant.expires_at,
        };

        let verifying_key = VerifyingKey::from_bytes(
            issuer_public_key
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, &grant.signature)? {
            return Err(AppError::Crypto("Invalid grant signature".to_string()));
        }

        // Check for deduplication
        let event_id = format!("grant:{}", grant.grant_id);
        if PermissionsRepository::event_exists(&self.db, &event_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
        {
            return Ok(()); // Already processed
        }

        // Update lamport clock
        self.db
            .update_lamport_clock(&grant.issuer_peer_id, grant.lamport_clock as i64)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Store grant
        let grant_data = GrantData {
            grant_id: grant.grant_id.clone(),
            issuer_peer_id: grant.issuer_peer_id.clone(),
            subject_peer_id: grant.subject_peer_id.clone(),
            capability: grant.capability.clone(),
            scope_json: grant.scope.as_ref().map(|s| s.to_string()),
            lamport_clock: grant.lamport_clock as i64,
            issued_at: grant.issued_at,
            expires_at: grant.expires_at,
            payload_cbor: grant.payload_cbor.clone(),
            signature: grant.signature.clone(),
        };

        PermissionsRepository::upsert_grant(&self.db, &grant_data)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Record event
        PermissionsRepository::record_event(
            &self.db,
            &RecordPermissionEventParams {
                event_id: &event_id,
                event_type: "grant",
                entity_id: &grant.grant_id,
                author_peer_id: &grant.issuer_peer_id,
                issuer_peer_id: Some(&grant.issuer_peer_id),
                subject_peer_id: &grant.subject_peer_id,
                capability: &grant.capability,
                scope_json: grant.scope.as_ref().map(|s| s.to_string()).as_deref(),
                lamport_clock: grant.lamport_clock as i64,
                issued_at: Some(grant.issued_at),
                expires_at: grant.expires_at,
                payload_cbor: &grant.payload_cbor,
                signature: &grant.signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(())
    }

    /// Verify and process a permission revocation from the network
    pub fn process_incoming_revoke(
        &self,
        revoke: &PermissionRevokeMessage,
        issuer_public_key: &[u8],
    ) -> Result<()> {
        // Verify signature
        let signable = SignablePermissionRevoke {
            grant_id: revoke.grant_id.clone(),
            issuer_peer_id: revoke.issuer_peer_id.clone(),
            lamport_clock: revoke.lamport_clock,
            revoked_at: revoke.revoked_at,
        };

        let verifying_key = VerifyingKey::from_bytes(
            issuer_public_key
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, &revoke.signature)? {
            return Err(AppError::Crypto("Invalid revoke signature".to_string()));
        }

        // Check for deduplication
        let event_id = format!("revoke:{}:{}", revoke.grant_id, revoke.lamport_clock);
        if PermissionsRepository::event_exists(&self.db, &event_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
        {
            return Ok(()); // Already processed
        }

        // Update lamport clock
        self.db
            .update_lamport_clock(&revoke.issuer_peer_id, revoke.lamport_clock as i64)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Apply revocation
        PermissionsRepository::revoke_grant(&self.db, &revoke.grant_id, revoke.revoked_at)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Record event (get grant details for event record)
        let grant = PermissionsRepository::get_by_grant_id(&self.db, &revoke.grant_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        let payload_cbor = signable.signable_bytes()?;

        if let Some(grant) = grant {
            PermissionsRepository::record_event(
                &self.db,
                &RecordPermissionEventParams {
                    event_id: &event_id,
                    event_type: "revoke",
                    entity_id: &revoke.grant_id,
                    author_peer_id: &revoke.issuer_peer_id,
                    issuer_peer_id: Some(&revoke.issuer_peer_id),
                    subject_peer_id: &grant.subject_peer_id,
                    capability: &grant.capability,
                    scope_json: None,
                    lamport_clock: revoke.lamport_clock as i64,
                    issued_at: None,
                    expires_at: None,
                    payload_cbor: &payload_cbor,
                    signature: &revoke.signature,
                },
            )
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        }

        Ok(())
    }

    // ============================================================
    // Query Methods
    // ============================================================

    /// Check if a peer has a specific capability from us
    pub fn peer_has_capability(
        &self,
        subject_peer_id: &str,
        capability: Capability,
    ) -> Result<bool> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        PermissionsRepository::has_capability(
            &self.db,
            &identity.peer_id,
            subject_peer_id,
            capability.as_str(),
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Check if we have a capability from another peer
    pub fn we_have_capability(&self, issuer_peer_id: &str, capability: Capability) -> Result<bool> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        PermissionsRepository::has_capability(
            &self.db,
            issuer_peer_id,
            &identity.peer_id,
            capability.as_str(),
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get all permissions we've granted
    pub fn get_granted_permissions(&self) -> Result<Vec<Permission>> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        PermissionsRepository::get_permissions_by_issuer(&self.db, &identity.peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get all permissions granted to us
    pub fn get_received_permissions(&self) -> Result<Vec<Permission>> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        PermissionsRepository::get_permissions_for_subject(&self.db, &identity.peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get all peers we can chat with (we granted them chat)
    pub fn get_chat_peers(&self) -> Result<Vec<String>> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        PermissionsRepository::get_chat_contacts(&self.db, &identity.peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get the grant for a specific permission (for proof generation)
    pub fn get_capability_grant(
        &self,
        issuer_peer_id: &str,
        capability: Capability,
    ) -> Result<Option<Permission>> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        PermissionsRepository::get_capability_grant(
            &self.db,
            issuer_peer_id,
            &identity.peer_id,
            capability.as_str(),
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateIdentityRequest;

    fn create_test_service() -> (Arc<Database>, Arc<IdentityService>, PermissionsService) {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let permissions_service = PermissionsService::new(db.clone(), identity_service.clone());
        (db, identity_service, permissions_service)
    }

    #[test]
    fn test_create_grant() {
        let (_, identity_service, permissions_service) = create_test_service();

        // Create identity first
        identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Test User".to_string(),
                passphrase: "password123".to_string(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        identity_service.unlock("password123").unwrap();

        // Create a grant
        let grant = permissions_service
            .create_permission_grant("12D3KooWSubject", Capability::Chat, None)
            .unwrap();

        assert!(!grant.grant_id.is_empty());
        assert_eq!(grant.capability, "chat");

        // Verify it's stored
        assert!(permissions_service
            .peer_has_capability("12D3KooWSubject", Capability::Chat)
            .unwrap());
    }

    #[test]
    fn test_revoke_grant() {
        let (_, identity_service, permissions_service) = create_test_service();

        identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Test User".to_string(),
                passphrase: "password123".to_string(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        identity_service.unlock("password123").unwrap();

        let grant = permissions_service
            .create_permission_grant("12D3KooWSubject", Capability::Chat, None)
            .unwrap();

        // Verify capability exists
        assert!(permissions_service
            .peer_has_capability("12D3KooWSubject", Capability::Chat)
            .unwrap());

        // Revoke
        permissions_service
            .revoke_permission(&grant.grant_id)
            .unwrap();

        // Verify capability is gone
        assert!(!permissions_service
            .peer_has_capability("12D3KooWSubject", Capability::Chat)
            .unwrap());
    }
}
