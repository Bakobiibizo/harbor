//! Voice calling service using WebRTC signaling

use ed25519_dalek::VerifyingKey;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::{Capability, Database};
use crate::error::{AppError, Result};
use crate::services::{
    verify, ContactsService, IdentityService, PermissionsService, SignableSignalingAnswer,
    SignableSignalingHangup, SignableSignalingIce, SignableSignalingOffer,
};

/// Call state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallState {
    /// Outgoing call, waiting for answer
    Ringing,
    /// Incoming call, not yet answered
    Incoming,
    /// Call is connected
    Connected,
    /// Call ended
    Ended,
}

impl CallState {
    pub fn as_str(&self) -> &'static str {
        match self {
            CallState::Ringing => "ringing",
            CallState::Incoming => "incoming",
            CallState::Connected => "connected",
            CallState::Ended => "ended",
        }
    }
}

/// An active call
#[derive(Debug, Clone)]
pub struct Call {
    pub call_id: String,
    pub caller_peer_id: String,
    pub callee_peer_id: String,
    pub state: CallState,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub end_reason: Option<String>,
}

/// Service for managing voice calls
pub struct CallingService {
    #[allow(dead_code)]
    db: Arc<Database>, // Reserved for future call history storage
    identity_service: Arc<IdentityService>,
    contacts_service: Arc<ContactsService>,
    permissions_service: Arc<PermissionsService>,
}

/// An outgoing signaling offer
#[derive(Debug, Clone)]
pub struct OutgoingOffer {
    pub call_id: String,
    pub caller_peer_id: String,
    pub callee_peer_id: String,
    pub sdp: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// An outgoing signaling answer
#[derive(Debug, Clone)]
pub struct OutgoingAnswer {
    pub call_id: String,
    pub caller_peer_id: String,
    pub callee_peer_id: String,
    pub sdp: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// An outgoing ICE candidate
#[derive(Debug, Clone)]
pub struct OutgoingIce {
    pub call_id: String,
    pub sender_peer_id: String,
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<u32>,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// An outgoing hangup
#[derive(Debug, Clone)]
pub struct OutgoingHangup {
    pub call_id: String,
    pub sender_peer_id: String,
    pub reason: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

impl CallingService {
    /// Create a new calling service
    pub fn new(
        db: Arc<Database>,
        identity_service: Arc<IdentityService>,
        contacts_service: Arc<ContactsService>,
        permissions_service: Arc<PermissionsService>,
    ) -> Self {
        Self {
            db,
            identity_service,
            contacts_service,
            permissions_service,
        }
    }

    /// Start a call to a peer
    pub fn create_offer(&self, callee_peer_id: &str, sdp: &str) -> Result<OutgoingOffer> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        // Check we have call permission with this peer
        if !self
            .permissions_service
            .peer_has_capability(callee_peer_id, Capability::Call)?
        {
            return Err(AppError::PermissionDenied(
                "No call permission with this peer".to_string(),
            ));
        }

        let call_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();

        let signable = SignableSignalingOffer {
            call_id: call_id.clone(),
            caller_peer_id: identity.peer_id.clone(),
            callee_peer_id: callee_peer_id.to_string(),
            sdp: sdp.to_string(),
            timestamp,
        };

        let signature = self.identity_service.sign(&signable)?;

        Ok(OutgoingOffer {
            call_id,
            caller_peer_id: identity.peer_id,
            callee_peer_id: callee_peer_id.to_string(),
            sdp: sdp.to_string(),
            timestamp,
            signature,
        })
    }

    /// Process an incoming offer
    pub fn process_incoming_offer(
        &self,
        call_id: &str,
        caller_peer_id: &str,
        callee_peer_id: &str,
        sdp: &str,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<()> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        // Verify we are the callee
        if callee_peer_id != identity.peer_id {
            return Err(AppError::Validation("Offer not for us".to_string()));
        }

        // Verify signature
        let caller_public_key = self
            .contacts_service
            .get_public_key(caller_peer_id)?
            .ok_or_else(|| AppError::NotFound("Caller not in contacts".to_string()))?;

        let signable = SignableSignalingOffer {
            call_id: call_id.to_string(),
            caller_peer_id: caller_peer_id.to_string(),
            callee_peer_id: callee_peer_id.to_string(),
            sdp: sdp.to_string(),
            timestamp,
        };

        let verifying_key = VerifyingKey::from_bytes(
            caller_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto("Invalid offer signature".to_string()));
        }

        // Check caller has call permission from us
        if !self
            .permissions_service
            .we_have_capability(caller_peer_id, Capability::Call)?
        {
            return Err(AppError::PermissionDenied(
                "Caller doesn't have call permission".to_string(),
            ));
        }

        Ok(())
    }

    /// Answer a call
    pub fn create_answer(
        &self,
        call_id: &str,
        caller_peer_id: &str,
        sdp: &str,
    ) -> Result<OutgoingAnswer> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let timestamp = chrono::Utc::now().timestamp();

        let signable = SignableSignalingAnswer {
            call_id: call_id.to_string(),
            caller_peer_id: caller_peer_id.to_string(),
            callee_peer_id: identity.peer_id.clone(),
            sdp: sdp.to_string(),
            timestamp,
        };

        let signature = self.identity_service.sign(&signable)?;

        Ok(OutgoingAnswer {
            call_id: call_id.to_string(),
            caller_peer_id: caller_peer_id.to_string(),
            callee_peer_id: identity.peer_id,
            sdp: sdp.to_string(),
            timestamp,
            signature,
        })
    }

    /// Process an incoming answer
    pub fn process_incoming_answer(
        &self,
        call_id: &str,
        caller_peer_id: &str,
        callee_peer_id: &str,
        sdp: &str,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<()> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        // Verify we are the caller
        if caller_peer_id != identity.peer_id {
            return Err(AppError::Validation("Answer not for our call".to_string()));
        }

        // Verify signature
        let callee_public_key = self
            .contacts_service
            .get_public_key(callee_peer_id)?
            .ok_or_else(|| AppError::NotFound("Callee not in contacts".to_string()))?;

        let signable = SignableSignalingAnswer {
            call_id: call_id.to_string(),
            caller_peer_id: caller_peer_id.to_string(),
            callee_peer_id: callee_peer_id.to_string(),
            sdp: sdp.to_string(),
            timestamp,
        };

        let verifying_key = VerifyingKey::from_bytes(
            callee_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto("Invalid answer signature".to_string()));
        }

        Ok(())
    }

    /// Send an ICE candidate
    pub fn create_ice_candidate(
        &self,
        call_id: &str,
        candidate: &str,
        sdp_mid: Option<&str>,
        sdp_mline_index: Option<u32>,
    ) -> Result<OutgoingIce> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let timestamp = chrono::Utc::now().timestamp();

        let signable = SignableSignalingIce {
            call_id: call_id.to_string(),
            sender_peer_id: identity.peer_id.clone(),
            candidate: candidate.to_string(),
            sdp_mid: sdp_mid.map(String::from),
            sdp_mline_index,
            timestamp,
        };

        let signature = self.identity_service.sign(&signable)?;

        Ok(OutgoingIce {
            call_id: call_id.to_string(),
            sender_peer_id: identity.peer_id,
            candidate: candidate.to_string(),
            sdp_mid: sdp_mid.map(String::from),
            sdp_mline_index,
            timestamp,
            signature,
        })
    }

    /// Process an incoming ICE candidate
    #[allow(clippy::too_many_arguments)]
    pub fn process_incoming_ice(
        &self,
        call_id: &str,
        sender_peer_id: &str,
        candidate: &str,
        sdp_mid: Option<&str>,
        sdp_mline_index: Option<u32>,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<()> {
        // Verify signature
        let sender_public_key = self
            .contacts_service
            .get_public_key(sender_peer_id)?
            .ok_or_else(|| AppError::NotFound("Sender not in contacts".to_string()))?;

        let signable = SignableSignalingIce {
            call_id: call_id.to_string(),
            sender_peer_id: sender_peer_id.to_string(),
            candidate: candidate.to_string(),
            sdp_mid: sdp_mid.map(String::from),
            sdp_mline_index,
            timestamp,
        };

        let verifying_key = VerifyingKey::from_bytes(
            sender_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto(
                "Invalid ICE candidate signature".to_string(),
            ));
        }

        Ok(())
    }

    /// Hang up a call
    pub fn create_hangup(&self, call_id: &str, reason: &str) -> Result<OutgoingHangup> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let timestamp = chrono::Utc::now().timestamp();

        let signable = SignableSignalingHangup {
            call_id: call_id.to_string(),
            sender_peer_id: identity.peer_id.clone(),
            reason: reason.to_string(),
            timestamp,
        };

        let signature = self.identity_service.sign(&signable)?;

        Ok(OutgoingHangup {
            call_id: call_id.to_string(),
            sender_peer_id: identity.peer_id,
            reason: reason.to_string(),
            timestamp,
            signature,
        })
    }

    /// Process an incoming hangup
    pub fn process_incoming_hangup(
        &self,
        call_id: &str,
        sender_peer_id: &str,
        reason: &str,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<()> {
        // Verify signature
        let sender_public_key = self
            .contacts_service
            .get_public_key(sender_peer_id)?
            .ok_or_else(|| AppError::NotFound("Sender not in contacts".to_string()))?;

        let signable = SignableSignalingHangup {
            call_id: call_id.to_string(),
            sender_peer_id: sender_peer_id.to_string(),
            reason: reason.to_string(),
            timestamp,
        };

        let verifying_key = VerifyingKey::from_bytes(
            sender_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto("Invalid hangup signature".to_string()));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        Capability, ContactData, ContactsRepository, GrantData, PermissionsRepository,
    };
    use crate::models::CreateIdentityRequest;
    use crate::services::{ContactsService, CryptoService, IdentityService, PermissionsService};
    use std::sync::Arc;

    fn create_test_env() -> (
        CallingService,
        Arc<Database>,
        Arc<IdentityService>,
        Arc<PermissionsService>,
        String, // our peer_id
    ) {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
        let permissions_service = Arc::new(PermissionsService::new(
            db.clone(),
            identity_service.clone(),
        ));

        let info = identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Call User".to_string(),
                passphrase: "test-pass".to_string(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();

        let service = CallingService::new(
            db.clone(),
            identity_service.clone(),
            contacts_service,
            permissions_service.clone(),
        );

        (
            service,
            db,
            identity_service,
            permissions_service,
            info.peer_id,
        )
    }

    /// Helper to add a peer contact and grant call permission
    fn add_peer_with_call_permission(
        db: &Database,
        permissions: &PermissionsService,
        peer_id: &str,
        public_key: &[u8],
    ) {
        let contact_data = ContactData {
            peer_id: peer_id.to_string(),
            public_key: public_key.to_vec(),
            x25519_public: vec![0u8; 32],
            display_name: "Peer".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(db, &contact_data).unwrap();

        permissions
            .create_permission_grant(peer_id, Capability::Call, None)
            .unwrap();
    }

    #[test]
    fn test_create_offer_success() {
        let (service, db, _identity, permissions, peer_id) = create_test_env();

        let (_, peer_verifying) = CryptoService::generate_ed25519_keypair();
        let callee = "12D3KooWCallee123";
        add_peer_with_call_permission(&db, &permissions, callee, &peer_verifying.to_bytes());

        let offer = service.create_offer(callee, "v=0\r\nsdp-data").unwrap();

        assert!(!offer.call_id.is_empty());
        assert_eq!(offer.caller_peer_id, peer_id);
        assert_eq!(offer.callee_peer_id, callee);
        assert_eq!(offer.sdp, "v=0\r\nsdp-data");
        assert!(!offer.signature.is_empty());
    }

    #[test]
    fn test_create_offer_no_permission() {
        let (service, db, _identity, _permissions, _peer_id) = create_test_env();

        // Add contact but don't grant call permission
        let (_, peer_verifying) = CryptoService::generate_ed25519_keypair();
        let contact_data = ContactData {
            peer_id: "12D3KooWCallee".to_string(),
            public_key: peer_verifying.to_bytes().to_vec(),
            x25519_public: vec![0u8; 32],
            display_name: "Callee".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        let result = service.create_offer("12D3KooWCallee", "sdp-data");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_offer_requires_identity() {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
        let permissions_service = Arc::new(PermissionsService::new(
            db.clone(),
            identity_service.clone(),
        ));
        let service =
            CallingService::new(db, identity_service, contacts_service, permissions_service);

        let result = service.create_offer("12D3KooWCallee", "sdp-data");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_answer_success() {
        let (service, _db, _identity, _permissions, peer_id) = create_test_env();

        let answer = service
            .create_answer("call-123", "12D3KooWCaller", "v=0\r\nsdp-answer")
            .unwrap();

        assert_eq!(answer.call_id, "call-123");
        assert_eq!(answer.caller_peer_id, "12D3KooWCaller");
        assert_eq!(answer.callee_peer_id, peer_id);
        assert_eq!(answer.sdp, "v=0\r\nsdp-answer");
        assert!(!answer.signature.is_empty());
    }

    #[test]
    fn test_create_ice_candidate() {
        let (service, _db, _identity, _permissions, peer_id) = create_test_env();

        let ice = service
            .create_ice_candidate("call-123", "candidate:0 1 UDP", Some("audio"), Some(0))
            .unwrap();

        assert_eq!(ice.call_id, "call-123");
        assert_eq!(ice.sender_peer_id, peer_id);
        assert_eq!(ice.candidate, "candidate:0 1 UDP");
        assert_eq!(ice.sdp_mid, Some("audio".to_string()));
        assert_eq!(ice.sdp_mline_index, Some(0));
        assert!(!ice.signature.is_empty());
    }

    #[test]
    fn test_create_ice_candidate_no_sdp_fields() {
        let (service, _db, _identity, _permissions, _peer_id) = create_test_env();

        let ice = service
            .create_ice_candidate("call-123", "candidate:0 1 UDP", None, None)
            .unwrap();

        assert_eq!(ice.sdp_mid, None);
        assert_eq!(ice.sdp_mline_index, None);
    }

    #[test]
    fn test_create_hangup() {
        let (service, _db, _identity, _permissions, peer_id) = create_test_env();

        let hangup = service.create_hangup("call-123", "normal").unwrap();

        assert_eq!(hangup.call_id, "call-123");
        assert_eq!(hangup.sender_peer_id, peer_id);
        assert_eq!(hangup.reason, "normal");
        assert!(!hangup.signature.is_empty());
    }

    #[test]
    fn test_create_hangup_various_reasons() {
        let (service, _db, _identity, _permissions, _peer_id) = create_test_env();

        for reason in &["normal", "busy", "declined", "error"] {
            let hangup = service.create_hangup("call-123", reason).unwrap();
            assert_eq!(hangup.reason, *reason);
        }
    }

    #[test]
    fn test_process_incoming_offer_valid() {
        let (service, db, _identity, _permissions, peer_id) = create_test_env();

        // Create a caller with real keys
        let (caller_signing, caller_verifying) = CryptoService::generate_ed25519_keypair();
        let caller_id = "12D3KooWCaller123";

        // Add caller as contact with call permission from them to us
        let contact_data = ContactData {
            peer_id: caller_id.to_string(),
            public_key: caller_verifying.to_bytes().to_vec(),
            x25519_public: vec![0u8; 32],
            display_name: "Caller".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        // We need a grant FROM caller TO us (we_have_capability checks issuer=caller, subject=us)
        let grant_data = GrantData {
            grant_id: "grant-call-1".to_string(),
            issuer_peer_id: caller_id.to_string(),
            subject_peer_id: peer_id.clone(),
            capability: "call".to_string(),
            scope_json: None,
            lamport_clock: 1,
            issued_at: 1000,
            expires_at: None,
            payload_cbor: vec![0],
            signature: vec![0],
        };
        PermissionsRepository::upsert_grant(&db, &grant_data).unwrap();

        // Create a signed offer
        let signable = SignableSignalingOffer {
            call_id: "call-1".to_string(),
            caller_peer_id: caller_id.to_string(),
            callee_peer_id: peer_id.clone(),
            sdp: "v=0\r\nsdp".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        let sig = crate::services::sign(&caller_signing, &signable).unwrap();

        let result = service.process_incoming_offer(
            "call-1",
            caller_id,
            &peer_id,
            "v=0\r\nsdp",
            signable.timestamp,
            &sig,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_process_incoming_offer_wrong_callee() {
        let (service, db, _identity, _permissions, _peer_id) = create_test_env();

        let (_caller_signing, caller_verifying) = CryptoService::generate_ed25519_keypair();
        let caller_id = "12D3KooWCaller123";

        let contact_data = ContactData {
            peer_id: caller_id.to_string(),
            public_key: caller_verifying.to_bytes().to_vec(),
            x25519_public: vec![0u8; 32],
            display_name: "Caller".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        // Offer addressed to a different peer
        let result = service.process_incoming_offer(
            "call-1",
            caller_id,
            "12D3KooWSomeoneElse",
            "sdp",
            1000,
            &vec![0u8; 64],
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_process_incoming_offer_invalid_signature() {
        let (service, db, _identity, _permissions, peer_id) = create_test_env();

        let (_, caller_verifying) = CryptoService::generate_ed25519_keypair();
        let caller_id = "12D3KooWCaller123";

        let contact_data = ContactData {
            peer_id: caller_id.to_string(),
            public_key: caller_verifying.to_bytes().to_vec(),
            x25519_public: vec![0u8; 32],
            display_name: "Caller".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        let result = service.process_incoming_offer(
            "call-1",
            caller_id,
            &peer_id,
            "sdp",
            1000,
            &vec![0u8; 64],
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_process_incoming_ice_valid() {
        let (service, db, _identity, _permissions, _peer_id) = create_test_env();

        let (sender_signing, sender_verifying) = CryptoService::generate_ed25519_keypair();
        let sender_id = "12D3KooWSender123";

        let contact_data = ContactData {
            peer_id: sender_id.to_string(),
            public_key: sender_verifying.to_bytes().to_vec(),
            x25519_public: vec![0u8; 32],
            display_name: "Sender".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        let signable = SignableSignalingIce {
            call_id: "call-1".to_string(),
            sender_peer_id: sender_id.to_string(),
            candidate: "candidate:0 1 UDP".to_string(),
            sdp_mid: Some("audio".to_string()),
            sdp_mline_index: Some(0),
            timestamp: chrono::Utc::now().timestamp(),
        };
        let sig = crate::services::sign(&sender_signing, &signable).unwrap();

        let result = service.process_incoming_ice(
            "call-1",
            sender_id,
            "candidate:0 1 UDP",
            Some("audio"),
            Some(0),
            signable.timestamp,
            &sig,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_process_incoming_hangup_valid() {
        let (service, db, _identity, _permissions, _peer_id) = create_test_env();

        let (sender_signing, sender_verifying) = CryptoService::generate_ed25519_keypair();
        let sender_id = "12D3KooWSender123";

        let contact_data = ContactData {
            peer_id: sender_id.to_string(),
            public_key: sender_verifying.to_bytes().to_vec(),
            x25519_public: vec![0u8; 32],
            display_name: "Sender".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        let signable = SignableSignalingHangup {
            call_id: "call-1".to_string(),
            sender_peer_id: sender_id.to_string(),
            reason: "normal".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        let sig = crate::services::sign(&sender_signing, &signable).unwrap();

        let result = service.process_incoming_hangup(
            "call-1",
            sender_id,
            "normal",
            signable.timestamp,
            &sig,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_process_incoming_hangup_invalid_signature() {
        let (service, db, _identity, _permissions, _peer_id) = create_test_env();

        let (_, sender_verifying) = CryptoService::generate_ed25519_keypair();
        let sender_id = "12D3KooWSender123";

        let contact_data = ContactData {
            peer_id: sender_id.to_string(),
            public_key: sender_verifying.to_bytes().to_vec(),
            x25519_public: vec![0u8; 32],
            display_name: "Sender".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        let result =
            service.process_incoming_hangup("call-1", sender_id, "normal", 1000, &vec![0u8; 64]);

        assert!(result.is_err());
    }

    #[test]
    fn test_call_state_as_str() {
        assert_eq!(CallState::Ringing.as_str(), "ringing");
        assert_eq!(CallState::Incoming.as_str(), "incoming");
        assert_eq!(CallState::Connected.as_str(), "connected");
        assert_eq!(CallState::Ended.as_str(), "ended");
    }

    #[test]
    fn test_create_hangup_locked_identity_fails() {
        let (service, _db, identity_service, _permissions, _peer_id) = create_test_env();

        identity_service.lock();

        let result = service.create_hangup("call-123", "normal");
        assert!(result.is_err());
    }
}
