//! Voice calling service using WebRTC signaling

use ed25519_dalek::VerifyingKey;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::Capability;
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
        identity_service: Arc<IdentityService>,
        contacts_service: Arc<ContactsService>,
        permissions_service: Arc<PermissionsService>,
    ) -> Self {
        Self {
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
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

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
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

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
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

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
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

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
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

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
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

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
