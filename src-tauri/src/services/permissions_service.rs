//! Permissions service for managing capability grants

use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::db::repositories::PrivateIntroductionsRepository;
use crate::db::{
    Capability, Database, GrantData, Permission, PermissionsRepository, RecordPermissionEventParams,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRevokeMessage {
    pub grant_id: String,
    pub issuer_peer_id: String,
    pub lamport_clock: u64,
    pub revoked_at: i64,
    pub signature: Vec<u8>,
}

impl PermissionsService {
    fn contact_card_capability(capability: Capability) -> &'static str {
        match capability {
            Capability::Chat => "message:send",
            Capability::WallRead => "wall:read",
            Capability::Call => "call:initiate",
        }
    }

    fn effective_capability(
        &self,
        issuer: &str,
        subject: &str,
        capability: Capability,
    ) -> Result<bool> {
        let local_peer_id = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?
            .peer_id;
        let remote_peer_id = if issuer == local_peer_id {
            subject
        } else if subject == local_peer_id {
            issuer
        } else {
            return Ok(false);
        };
        let relationship_denied = self
            .db
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT CASE WHEN
                       EXISTS(SELECT 1 FROM contact_revocation_tombstones WHERE peer_id = ?1)
                       OR NOT EXISTS(
                         SELECT 1 FROM contacts WHERE peer_id = ?1 AND is_blocked = 0
                       )
                     THEN 1 ELSE 0 END",
                    [remote_peer_id],
                    |row| row.get::<_, i64>(0),
                )
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))?
            != 0;
        if relationship_denied {
            return Ok(false);
        }

        let decision = PrivateIntroductionsRepository::new(&self.db)
            .capability_decision(
                issuer,
                subject,
                Self::contact_card_capability(capability),
                chrono::Utc::now().timestamp(),
            )
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        match decision {
            Some(value) => Ok(value),
            None => PermissionsRepository::has_capability(
                &self.db,
                issuer,
                subject,
                capability.as_str(),
            )
            .map_err(|e| AppError::DatabaseString(e.to_string())),
        }
    }
    /// Create a new permissions service
    pub fn new(db: Arc<Database>, identity_service: Arc<IdentityService>) -> Self {
        Self {
            db,
            identity_service,
        }
    }

    /// Build signed grants without mutating durable state. Callers that need a
    /// multi-table invariant can include these exact envelopes in their own
    /// transaction instead of exposing one grant at a time.
    pub fn prepare_permission_grants(
        &self,
        subject_peer_id: &str,
        capabilities: &[Capability],
    ) -> Result<Vec<PermissionGrantMessage>> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;
        let current_clock = self
            .db
            .get_lamport_clock(&identity.peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        let issued_at = chrono::Utc::now().timestamp();

        capabilities
            .iter()
            .enumerate()
            .map(|(offset, capability)| {
                let lamport_clock = current_clock
                    .checked_add(offset as i64 + 1)
                    .ok_or_else(|| AppError::Internal("Lamport clock overflow".into()))?
                    as u64;
                let grant_id = Uuid::new_v4().to_string();
                let signable = SignablePermissionGrant {
                    grant_id: grant_id.clone(),
                    issuer_peer_id: identity.peer_id.clone(),
                    subject_peer_id: subject_peer_id.to_string(),
                    capability: capability.as_str().to_string(),
                    scope: None,
                    lamport_clock,
                    issued_at,
                    expires_at: None,
                };
                let payload_cbor = signable.signable_bytes()?;
                let signature = self.identity_service.sign(&signable)?;
                Ok(PermissionGrantMessage {
                    grant_id,
                    issuer_peer_id: identity.peer_id.clone(),
                    subject_peer_id: subject_peer_id.to_string(),
                    capability: capability.as_str().to_string(),
                    scope: None,
                    lamport_clock,
                    issued_at,
                    expires_at: None,
                    signature,
                    payload_cbor,
                })
            })
            .collect()
    }

    /// Build signed revocations for every active capability this identity
    /// issued to a peer. The caller persists these exact envelopes together
    /// with the relationship teardown in one transaction.
    pub fn prepare_contact_revocations(
        &self,
        subject_peer_id: &str,
    ) -> Result<Vec<PermissionRevokeMessage>> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;
        let grants = PermissionsRepository::get_permissions_by_issuer(&self.db, &identity.peer_id)
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
        let active: Vec<_> = grants
            .into_iter()
            .filter(|grant| grant.subject_peer_id == subject_peer_id && grant.revoked_at.is_none())
            .collect();
        let current_clock = self
            .db
            .get_lamport_clock(&identity.peer_id)
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
        let revoked_at = chrono::Utc::now().timestamp();

        active
            .into_iter()
            .enumerate()
            .map(|(offset, grant)| {
                let lamport_clock = current_clock
                    .checked_add(offset as i64 + 1)
                    .ok_or_else(|| AppError::Internal("Lamport clock overflow".into()))?
                    as u64;
                let signable = SignablePermissionRevoke {
                    grant_id: grant.grant_id.clone(),
                    issuer_peer_id: identity.peer_id.clone(),
                    lamport_clock,
                    revoked_at,
                };
                Ok(PermissionRevokeMessage {
                    grant_id: grant.grant_id,
                    issuer_peer_id: identity.peer_id.clone(),
                    lamport_clock,
                    revoked_at,
                    signature: self.identity_service.sign(&signable)?,
                })
            })
            .collect()
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
        self.validate_incoming_grant(grant, issuer_public_key)?;

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

    /// Validate a received grant without writing it. This is used before the
    /// contact acceptance transaction so malformed input cannot cause a
    /// partially materialized relationship.
    pub fn validate_incoming_grant(
        &self,
        grant: &PermissionGrantMessage,
        issuer_public_key: &[u8],
    ) -> Result<()> {
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

        let issuer_peer_id =
            crate::services::CryptoService::derive_peer_id_from_verifying_key(&verifying_key)?;
        if issuer_peer_id != grant.issuer_peer_id {
            return Err(AppError::Unauthorized(
                "Permission grant issuer does not match its signing key".to_string(),
            ));
        }
        let local_peer_id = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?
            .peer_id;
        if grant.subject_peer_id != local_peer_id {
            return Err(AppError::Unauthorized(
                "Permission grant is addressed to another peer".to_string(),
            ));
        }
        if Capability::from_str(&grant.capability).is_none() {
            return Err(AppError::Validation(format!(
                "Unknown permission capability: {}",
                grant.capability
            )));
        }
        if grant.payload_cbor != signable.signable_bytes()? {
            return Err(AppError::Crypto(
                "Permission grant payload does not match signed fields".to_string(),
            ));
        }

        if !verify(&verifying_key, &signable, &grant.signature)? {
            return Err(AppError::Crypto("Invalid grant signature".to_string()));
        }

        Ok(())
    }

    /// Verify and process a permission revocation from the network
    pub fn process_incoming_revoke(
        &self,
        revoke: &PermissionRevokeMessage,
        issuer_public_key: &[u8],
    ) -> Result<()> {
        self.validate_incoming_revoke(revoke, issuer_public_key)?;

        let signable = SignablePermissionRevoke {
            grant_id: revoke.grant_id.clone(),
            issuer_peer_id: revoke.issuer_peer_id.clone(),
            lamport_clock: revoke.lamport_clock,
            revoked_at: revoke.revoked_at,
        };

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

    /// Validate a received revocation without writing it so callers can make
    /// the revocation and relationship teardown atomic.
    pub fn validate_incoming_revoke(
        &self,
        revoke: &PermissionRevokeMessage,
        issuer_public_key: &[u8],
    ) -> Result<()> {
        let verifying_key = VerifyingKey::from_bytes(
            issuer_public_key
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|error| AppError::Crypto(format!("Invalid public key: {error}")))?;
        let issuer_peer_id =
            crate::services::CryptoService::derive_peer_id_from_verifying_key(&verifying_key)?;
        if issuer_peer_id != revoke.issuer_peer_id {
            return Err(AppError::Unauthorized(
                "Permission revocation issuer does not match its signing key".to_string(),
            ));
        }
        let signable = SignablePermissionRevoke {
            grant_id: revoke.grant_id.clone(),
            issuer_peer_id: revoke.issuer_peer_id.clone(),
            lamport_clock: revoke.lamport_clock,
            revoked_at: revoke.revoked_at,
        };
        if !verify(&verifying_key, &signable, &revoke.signature)? {
            return Err(AppError::Crypto("Invalid revoke signature".to_string()));
        }
        let grant = PermissionsRepository::get_by_grant_id(&self.db, &revoke.grant_id)
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
        if let Some(grant) = grant {
            if grant.issuer_peer_id != revoke.issuer_peer_id {
                return Err(AppError::Unauthorized(
                    "Permission revocation does not match the grant issuer".to_string(),
                ));
            }
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

        self.effective_capability(&identity.peer_id, subject_peer_id, capability)
    }

    /// Check if we have a capability from another peer
    pub fn we_have_capability(&self, issuer_peer_id: &str, capability: Capability) -> Result<bool> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        self.effective_capability(issuer_peer_id, &identity.peer_id, capability)
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
    use crate::db::{ContactData, ContactsRepository};
    use crate::models::{
        domain, CapabilityGrantRecord, CapabilityRevocationRecord, CreateIdentityRequest,
    };
    use base64::Engine;

    fn create_test_service() -> (Arc<Database>, Arc<IdentityService>, PermissionsService) {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let permissions_service = PermissionsService::new(db.clone(), identity_service.clone());
        (db, identity_service, permissions_service)
    }

    fn add_contact(db: &Database, peer_id: &str) {
        ContactsRepository::add_contact(
            db,
            &ContactData {
                peer_id: peer_id.into(),
                public_key: vec![1; 32],
                x25519_public: vec![2; 32],
                display_name: "Test contact".into(),
                avatar_hash: None,
                bio: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn contact_card_revocation_overrides_legacy_permission() {
        let (db, identity_service, service) = create_test_service();
        identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Test User".into(),
                passphrase: "password123".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        let issuer = identity_service.get_identity().unwrap().unwrap().peer_id;
        let subject = "12D3KooWContact";
        add_contact(&db, subject);
        service
            .create_permission_grant(subject, Capability::WallRead, None)
            .unwrap();
        assert!(service
            .peer_has_capability(subject, Capability::WallRead)
            .unwrap());

        let repo = PrivateIntroductionsRepository::new(&db);
        let now = chrono::Utc::now().timestamp();
        let grant = CapabilityGrantRecord {
            domain: domain::CAPABILITY_GRANT.into(),
            version: 1,
            grant_id: "private-wall".into(),
            issuer_peer_id: issuer.clone(),
            subject_peer_id: subject.into(),
            capability: "wall:read".into(),
            revision: 1,
            issued_at: now,
            expires_at: Some(now + 60),
            revocation_id: "private-wall-revoke".into(),
        };
        repo.apply_grant(&grant, now).unwrap();
        assert!(service
            .peer_has_capability(subject, Capability::WallRead)
            .unwrap());
        repo.apply_revocation(&CapabilityRevocationRecord {
            domain: domain::CAPABILITY_REVOCATION.into(),
            version: 1,
            grant_id: grant.grant_id,
            issuer_peer_id: issuer,
            revision: 2,
            revoked_at: now + 1,
            revocation_id: grant.revocation_id,
        })
        .unwrap();
        assert!(!service
            .peer_has_capability(subject, Capability::WallRead)
            .unwrap());
    }

    #[test]
    fn expired_contact_card_capability_denies_access() {
        let (db, identity_service, service) = create_test_service();
        identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Test User".into(),
                passphrase: "password123".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        let issuer = identity_service.get_identity().unwrap().unwrap().peer_id;
        let subject = "12D3KooWExpired";
        let now = chrono::Utc::now().timestamp();
        PrivateIntroductionsRepository::new(&db)
            .apply_grant(
                &CapabilityGrantRecord {
                    domain: domain::CAPABILITY_GRANT.into(),
                    version: 1,
                    grant_id: "expired-call".into(),
                    issuer_peer_id: issuer,
                    subject_peer_id: subject.into(),
                    capability: "call:initiate".into(),
                    revision: 1,
                    issued_at: now - 20,
                    expires_at: Some(now - 1),
                    revocation_id: "expired-call-revoke".into(),
                },
                now - 20,
            )
            .unwrap();
        assert!(!service
            .peer_has_capability(subject, Capability::Call)
            .unwrap());
    }

    #[test]
    fn test_create_grant() {
        let (db, identity_service, permissions_service) = create_test_service();

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
        add_contact(&db, "12D3KooWSubject");

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
        let (db, identity_service, permissions_service) = create_test_service();

        identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Test User".to_string(),
                passphrase: "password123".to_string(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        identity_service.unlock("password123").unwrap();
        add_contact(&db, "12D3KooWSubject");

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

    #[test]
    fn signed_wall_read_grant_survives_recipient_restart() {
        let (_, issuer_identity, issuer_permissions) = create_test_service();
        let temp = tempfile::tempdir().unwrap();
        let recipient_path = temp.path().join("recipient.db");

        issuer_identity
            .create_identity(CreateIdentityRequest {
                display_name: "Issuer".into(),
                passphrase: "password123".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        let issuer_info = issuer_identity.get_identity_info().unwrap().unwrap();

        let recipient_peer_id = {
            let recipient_db = Arc::new(Database::new(recipient_path.clone()).unwrap());
            let recipient_identity = Arc::new(IdentityService::new(recipient_db.clone()));
            let recipient_permissions =
                PermissionsService::new(recipient_db.clone(), recipient_identity.clone());
            let recipient_info = recipient_identity
                .create_identity(CreateIdentityRequest {
                    display_name: "Recipient".into(),
                    passphrase: "password123".into(),
                    bio: None,
                    passphrase_hint: None,
                })
                .unwrap();
            let grant = issuer_permissions
                .create_permission_grant(&recipient_info.peer_id, Capability::WallRead, None)
                .unwrap();
            let issuer_public_key = base64::engine::general_purpose::STANDARD
                .decode(&issuer_info.public_key)
                .unwrap();
            add_contact(&recipient_db, &issuer_info.peer_id);
            recipient_permissions
                .process_incoming_grant(&grant, &issuer_public_key)
                .unwrap();
            assert!(recipient_permissions
                .we_have_capability(&issuer_info.peer_id, Capability::WallRead)
                .unwrap());
            recipient_info.peer_id
        };

        let reopened_db = Arc::new(Database::new(recipient_path).unwrap());
        let reopened_identity = Arc::new(IdentityService::new(reopened_db.clone()));
        let reopened_permissions = PermissionsService::new(reopened_db, reopened_identity);
        assert!(reopened_permissions
            .we_have_capability(&issuer_info.peer_id, Capability::WallRead)
            .unwrap());
        assert!(!recipient_peer_id.is_empty());
    }

    #[test]
    fn incoming_grant_for_another_recipient_is_rejected() {
        let (_, issuer_identity, issuer_permissions) = create_test_service();
        let (_, recipient_identity, recipient_permissions) = create_test_service();
        issuer_identity
            .create_identity(CreateIdentityRequest {
                display_name: "Issuer".into(),
                passphrase: "password123".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        recipient_identity
            .create_identity(CreateIdentityRequest {
                display_name: "Recipient".into(),
                passphrase: "password123".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        let grant = issuer_permissions
            .create_permission_grant("12D3KooWSomeoneElse", Capability::WallRead, None)
            .unwrap();
        let issuer_info = issuer_identity.get_identity_info().unwrap().unwrap();
        let issuer_public_key = base64::engine::general_purpose::STANDARD
            .decode(issuer_info.public_key)
            .unwrap();

        assert!(matches!(
            recipient_permissions.process_incoming_grant(&grant, &issuer_public_key),
            Err(AppError::Unauthorized(message)) if message.contains("another peer")
        ));
    }
}
