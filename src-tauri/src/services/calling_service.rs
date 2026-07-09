//! Voice calling service using WebRTC signaling

use ed25519_dalek::VerifyingKey;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub use crate::db::CallState;
use crate::db::{
    CallDirection, CallMediaKind, CallSession, CallsRepository, Capability, GroupCallRoom,
    GroupCallsRepository, NewCallSession,
};
use crate::error::{AppError, Result};
use crate::p2p::protocols::signaling::{
    GroupMembershipAction, GroupMembershipSignal, SignalingEnvelope, SignalingPayload,
};
use crate::services::{
    verify, ContactsService, CryptoService, IdentityService, PermissionsService,
    SignableGroupMembership, SignableSignalingAnswer, SignableSignalingHangup,
    SignableSignalingIce, SignableSignalingOffer,
};
use crate::Database;

const MAX_SIGNALING_TIMESTAMP_SKEW_SECONDS: i64 = 5 * 60;
const MAX_SDP_BYTES: usize = 256 * 1024;
const MAX_ICE_CANDIDATE_BYTES: usize = 16 * 1024;
const GROUP_CALL_TOPOLOGY: &str = "relay_assisted_mesh_v1";
const GROUP_CALL_MAX_PARTICIPANTS: usize = 4;

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
    db: Arc<Database>,
    identity_service: Arc<IdentityService>,
    contacts_service: Arc<ContactsService>,
    permissions_service: Arc<PermissionsService>,
    seen_signaling: Mutex<HashSet<String>>,
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
    pub target_peer_id: String,
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
    pub target_peer_id: String,
    pub reason: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Parameters for processing an incoming ICE candidate
pub struct IncomingIceParams<'a> {
    pub call_id: &'a str,
    pub sender_peer_id: &'a str,
    pub candidate: &'a str,
    pub sdp_mid: Option<&'a str>,
    pub sdp_mline_index: Option<u32>,
    pub timestamp: i64,
    pub signature: &'a [u8],
}

impl CallingService {
    pub fn database(&self) -> &Database {
        &self.db
    }
    fn group_action_name(action: &GroupMembershipAction) -> &'static str {
        match action {
            GroupMembershipAction::Invite => "invite",
            GroupMembershipAction::Join => "join",
            GroupMembershipAction::Leave => "leave",
            GroupMembershipAction::Roster => "roster",
            GroupMembershipAction::Terminate => "terminate",
        }
    }

    fn group_signable(signal: &GroupMembershipSignal) -> SignableGroupMembership {
        SignableGroupMembership {
            room_id: signal.room_id.clone(),
            creator_peer_id: signal.creator_peer_id.clone(),
            sender_peer_id: signal.sender_peer_id.clone(),
            action: Self::group_action_name(&signal.action).to_string(),
            topology: signal.topology.clone(),
            roster_version: signal.roster_version,
            participants: signal.participants.clone(),
            media_mode: signal.media_mode.clone(),
            nonce: signal.nonce.clone(),
            timestamp: signal.timestamp,
        }
    }

    pub fn create_group_membership(
        &self,
        room_id: Option<&str>,
        creator_peer_id: Option<&str>,
        action: GroupMembershipAction,
        roster_version: u64,
        participants: &[String],
        media_mode: &str,
    ) -> Result<GroupMembershipSignal> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;
        let mut roster = participants.to_vec();
        roster.sort();
        roster.dedup();
        if roster.is_empty() || roster.len() > GROUP_CALL_MAX_PARTICIPANTS {
            return Err(AppError::Validation(
                "Group roster must contain between 1 and 4 unique participants".to_string(),
            ));
        }
        if !roster.contains(&identity.peer_id) {
            return Err(AppError::Validation(
                "Local peer must be in group roster".to_string(),
            ));
        }
        if !matches!(media_mode, "audio" | "video") {
            return Err(AppError::Validation("Invalid group media mode".to_string()));
        }
        for peer_id in roster.iter().filter(|peer| *peer != &identity.peer_id) {
            self.validate_contact_identity(peer_id)?;
            self.require_any_call_grant_with(peer_id)?;
        }
        let now = chrono::Utc::now().timestamp();
        let room_id = room_id
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let creator = creator_peer_id.unwrap_or(&identity.peer_id).to_string();
        let mut signal = GroupMembershipSignal {
            room_id: room_id.clone(),
            creator_peer_id: creator.clone(),
            sender_peer_id: identity.peer_id.clone(),
            action,
            topology: GROUP_CALL_TOPOLOGY.to_string(),
            roster_version,
            participants: roster.clone(),
            media_mode: media_mode.to_string(),
            nonce: Uuid::new_v4().to_string(),
            timestamp: now,
            signature: Vec::new(),
        };
        signal.signature = self.identity_service.sign(&Self::group_signable(&signal))?;
        GroupCallsRepository::upsert(
            &self.db,
            &GroupCallRoom {
                room_id,
                creator_peer_id: creator,
                topology: GROUP_CALL_TOPOLOGY.to_string(),
                media_mode: media_mode.to_string(),
                roster_version,
                participants: roster,
                state: match signal.action {
                    GroupMembershipAction::Invite => "invited",
                    GroupMembershipAction::Terminate => "terminated",
                    GroupMembershipAction::Leave => "left",
                    _ => "active",
                }
                .to_string(),
                created_at: now,
                updated_at: now,
            },
        )?;
        Ok(signal)
    }

    fn process_group_membership(&self, signal: &GroupMembershipSignal) -> Result<()> {
        let local_peer_id = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?
            .peer_id;
        if signal.topology != GROUP_CALL_TOPOLOGY {
            return Err(AppError::Validation(
                "Unsupported group-call topology".to_string(),
            ));
        }
        if signal.participants.is_empty() || signal.participants.len() > GROUP_CALL_MAX_PARTICIPANTS
        {
            return Err(AppError::Validation(
                "Invalid group roster size".to_string(),
            ));
        }
        let mut canonical = signal.participants.clone();
        canonical.sort();
        canonical.dedup();
        if canonical != signal.participants {
            return Err(AppError::Validation(
                "Group roster must be sorted and unique".to_string(),
            ));
        }
        if !canonical.contains(&signal.creator_peer_id)
            || !canonical.contains(&signal.sender_peer_id)
        {
            return Err(AppError::PermissionDenied(
                "Group sender and creator must be roster members".to_string(),
            ));
        }
        for peer_id in &canonical {
            if peer_id != &signal.sender_peer_id && peer_id != &local_peer_id {
                self.validate_contact_identity(peer_id)?;
                self.require_any_call_grant_with(peer_id)?;
            }
        }
        let public_key = self
            .contacts_service
            .get_public_key(&signal.sender_peer_id)?
            .ok_or_else(|| AppError::NotFound("Sender public key not found".to_string()))?;
        let key_bytes: [u8; 32] = public_key
            .as_slice()
            .try_into()
            .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?;
        let key = VerifyingKey::from_bytes(&key_bytes)
            .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;
        if !verify(&key, &Self::group_signable(signal), &signal.signature)? {
            return Err(AppError::Crypto(
                "Invalid group membership signature".to_string(),
            ));
        }
        if !GroupCallsRepository::record_nonce(
            &self.db,
            &signal.room_id,
            &signal.sender_peer_id,
            &signal.nonce,
            signal.timestamp,
        )? {
            return Err(AppError::AlreadyExists(
                "Duplicate group membership nonce".to_string(),
            ));
        }
        if let Some(existing) = GroupCallsRepository::get(&self.db, &signal.room_id)? {
            if existing.creator_peer_id != signal.creator_peer_id {
                return Err(AppError::Validation(
                    "Group creator cannot change".to_string(),
                ));
            }
            let member_ack = matches!(
                signal.action,
                GroupMembershipAction::Join | GroupMembershipAction::Leave
            );
            if (member_ack && signal.roster_version != existing.roster_version)
                || (!member_ack && signal.roster_version <= existing.roster_version)
            {
                return Err(AppError::Validation(
                    "Stale group roster version".to_string(),
                ));
            }
            if matches!(
                signal.action,
                GroupMembershipAction::Join | GroupMembershipAction::Leave
            ) && (!existing.participants.contains(&signal.sender_peer_id)
                || canonical != existing.participants)
            {
                return Err(AppError::PermissionDenied(
                    "Only invited roster members may join or leave without a creator roster update"
                        .to_string(),
                ));
            }
            if !matches!(
                signal.action,
                GroupMembershipAction::Join | GroupMembershipAction::Leave
            ) && signal.sender_peer_id != signal.creator_peer_id
            {
                return Err(AppError::PermissionDenied(
                    "Only the group creator may publish rosters or terminate the room".to_string(),
                ));
            }
        } else if !matches!(signal.action, GroupMembershipAction::Invite)
            || signal.sender_peer_id != signal.creator_peer_id
        {
            return Err(AppError::Validation("Unknown group room".to_string()));
        }
        let now = chrono::Utc::now().timestamp();
        GroupCallsRepository::upsert(
            &self.db,
            &GroupCallRoom {
                room_id: signal.room_id.clone(),
                creator_peer_id: signal.creator_peer_id.clone(),
                topology: signal.topology.clone(),
                media_mode: signal.media_mode.clone(),
                roster_version: signal.roster_version,
                participants: canonical,
                state: match signal.action {
                    GroupMembershipAction::Invite => "invited",
                    GroupMembershipAction::Leave => "left",
                    GroupMembershipAction::Terminate => "terminated",
                    _ => "active",
                }
                .to_string(),
                created_at: now,
                updated_at: now,
            },
        )?;
        Ok(())
    }

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
            seen_signaling: Mutex::new(HashSet::new()),
        }
    }

    fn validate_sdp(sdp: &str) -> Result<()> {
        let trimmed = sdp.trim();
        if trimmed.is_empty() {
            return Err(AppError::Validation("SDP must not be empty".to_string()));
        }
        if sdp.len() > MAX_SDP_BYTES {
            return Err(AppError::Validation("SDP is too large".to_string()));
        }
        if !trimmed.lines().any(|line| line.trim() == "v=0") {
            return Err(AppError::Validation(
                "Invalid SDP: missing v=0 line".to_string(),
            ));
        }
        Ok(())
    }

    fn media_kind_from_sdp(sdp: &str) -> CallMediaKind {
        if sdp
            .lines()
            .any(|line| line.trim_start().starts_with("m=video"))
        {
            CallMediaKind::Video
        } else {
            CallMediaKind::Audio
        }
    }

    fn validate_ice_candidate(candidate: &str) -> Result<()> {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            return Err(AppError::Validation(
                "ICE candidate must not be empty".to_string(),
            ));
        }
        if candidate.len() > MAX_ICE_CANDIDATE_BYTES {
            return Err(AppError::Validation(
                "ICE candidate is too large".to_string(),
            ));
        }
        if !trimmed.starts_with("candidate:") {
            return Err(AppError::Validation(
                "Invalid ICE candidate: missing candidate: prefix".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_hangup_reason(reason: &str) -> Result<()> {
        match reason {
            "normal" | "busy" | "declined" | "error" => Ok(()),
            _ => Err(AppError::Validation(format!(
                "Invalid hangup reason: {}",
                reason
            ))),
        }
    }

    fn validate_recent_timestamp(timestamp: i64) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        if timestamp < now - MAX_SIGNALING_TIMESTAMP_SKEW_SECONDS
            || timestamp > now + MAX_SIGNALING_TIMESTAMP_SKEW_SECONDS
        {
            return Err(AppError::Validation(
                "Signaling timestamp is outside the allowed freshness window".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_contact_identity(&self, peer_id: &str) -> Result<()> {
        if !self.contacts_service.is_contact(peer_id)? {
            return Err(AppError::NotFound("Sender not in contacts".to_string()));
        }

        let public_key = self
            .contacts_service
            .get_public_key(peer_id)?
            .ok_or_else(|| AppError::NotFound("Sender public key not found".to_string()))?;
        let public_key_bytes: [u8; 32] = public_key
            .as_slice()
            .try_into()
            .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?;
        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
            .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;
        let derived_peer_id = CryptoService::derive_peer_id_from_verifying_key(&verifying_key)?;
        if derived_peer_id != peer_id {
            return Err(AppError::Crypto(format!(
                "Contact public key derives peer ID {} but signaling sender claims {}",
                derived_peer_id, peer_id
            )));
        }
        Ok(())
    }

    fn has_any_call_grant_with(&self, peer_id: &str) -> Result<bool> {
        Ok(self
            .permissions_service
            .peer_has_capability(peer_id, Capability::Call)?
            || self
                .permissions_service
                .we_have_capability(peer_id, Capability::Call)?)
    }

    fn require_any_call_grant_with(&self, peer_id: &str) -> Result<()> {
        if !self.has_any_call_grant_with(peer_id)? {
            return Err(AppError::PermissionDenied(
                "No call permission grant with this peer".to_string(),
            ));
        }
        Ok(())
    }

    fn fingerprint_signaling(envelope: &SignalingEnvelope) -> Result<String> {
        let mut bytes = Vec::new();
        ciborium::into_writer(envelope, &mut bytes)
            .map_err(|e| AppError::Serialization(format!("CBOR encoding failed: {}", e)))?;
        let digest = Sha256::digest(&bytes);
        Ok(hex::encode(digest))
    }

    fn record_signaling_once(&self, envelope: &SignalingEnvelope) -> Result<()> {
        let fingerprint = Self::fingerprint_signaling(envelope)?;
        let mut seen = self
            .seen_signaling
            .lock()
            .map_err(|_| AppError::Internal("Signaling replay cache poisoned".to_string()))?;
        if !seen.insert(fingerprint) {
            return Err(AppError::AlreadyExists(
                "Duplicate signaling message".to_string(),
            ));
        }
        Ok(())
    }

    fn require_no_active_call_with_peer(&self, peer_id: &str) -> Result<()> {
        if CallsRepository::has_active_call_with_peer(&self.db, peer_id)? {
            return Err(AppError::AlreadyExists(
                "An active call with this peer already exists".to_string(),
            ));
        }
        Ok(())
    }

    fn require_call_state(&self, call_id: &str, expected_state: CallState) -> Result<CallSession> {
        let call = CallsRepository::get_by_call_id(&self.db, call_id)?
            .ok_or_else(|| AppError::NotFound("Call not found".to_string()))?;
        if call.state != expected_state {
            return Err(AppError::Validation(format!(
                "Invalid call transition from {} (expected {})",
                call.state.as_str(),
                expected_state.as_str()
            )));
        }
        Ok(call)
    }

    fn ensure_call_is_active(&self, call_id: &str) -> Result<CallSession> {
        let call = CallsRepository::get_by_call_id(&self.db, call_id)?
            .ok_or_else(|| AppError::NotFound("Call not found".to_string()))?;
        if call.state.is_terminal() {
            return Err(AppError::Validation("Call has already ended".to_string()));
        }
        Ok(call)
    }

    fn mark_connected(&self, call_id: &str) -> Result<()> {
        let updated =
            CallsRepository::mark_connected(&self.db, call_id, chrono::Utc::now().timestamp())?;
        if updated == 0 {
            return Err(AppError::Validation(
                "Call cannot transition to connected".to_string(),
            ));
        }
        Ok(())
    }

    fn end_active_call(&self, call_id: &str, reason: &str) -> Result<()> {
        let updated =
            CallsRepository::mark_ended(&self.db, call_id, reason, chrono::Utc::now().timestamp())?;
        if updated == 0 {
            return Err(AppError::Validation("Call cannot be ended".to_string()));
        }
        Ok(())
    }

    fn insert_outgoing_call(
        &self,
        call_id: &str,
        caller_peer_id: &str,
        callee_peer_id: &str,
        timestamp: i64,
        media_kind: CallMediaKind,
    ) -> Result<()> {
        CallsRepository::insert_session(
            &self.db,
            &NewCallSession {
                call_id: call_id.to_string(),
                peer_id: callee_peer_id.to_string(),
                caller_peer_id: caller_peer_id.to_string(),
                callee_peer_id: callee_peer_id.to_string(),
                direction: CallDirection::Outgoing,
                media_kind,
                state: CallState::Ringing,
                started_at: timestamp,
                ended_at: None,
                duration_seconds: None,
                terminal_reason: None,
            },
        )?;
        Ok(())
    }

    fn insert_incoming_call(
        &self,
        call_id: &str,
        caller_peer_id: &str,
        callee_peer_id: &str,
        timestamp: i64,
        media_kind: CallMediaKind,
    ) -> Result<()> {
        CallsRepository::insert_session(
            &self.db,
            &NewCallSession {
                call_id: call_id.to_string(),
                peer_id: caller_peer_id.to_string(),
                caller_peer_id: caller_peer_id.to_string(),
                callee_peer_id: callee_peer_id.to_string(),
                direction: CallDirection::Incoming,
                media_kind,
                state: CallState::Incoming,
                started_at: timestamp,
                ended_at: None,
                duration_seconds: None,
                terminal_reason: None,
            },
        )?;
        Ok(())
    }

    fn insert_busy_history_if_unknown(
        &self,
        call_id: &str,
        caller_peer_id: &str,
        callee_peer_id: &str,
        timestamp: i64,
    ) -> Result<()> {
        if CallsRepository::get_by_call_id(&self.db, call_id)?.is_none() {
            CallsRepository::insert_session(
                &self.db,
                &NewCallSession {
                    call_id: call_id.to_string(),
                    peer_id: caller_peer_id.to_string(),
                    caller_peer_id: caller_peer_id.to_string(),
                    callee_peer_id: callee_peer_id.to_string(),
                    direction: CallDirection::Incoming,
                    media_kind: CallMediaKind::Audio,
                    state: CallState::Ended,
                    started_at: timestamp,
                    ended_at: Some(timestamp),
                    duration_seconds: Some(0),
                    terminal_reason: Some("busy".to_string()),
                },
            )?;
        }
        Ok(())
    }

    /// Return persisted active call sessions.
    pub fn get_active_calls(&self) -> Result<Vec<CallSession>> {
        Ok(CallsRepository::get_active_calls(&self.db)?)
    }

    /// Return persisted call history, newest first.
    pub fn get_call_history(&self, limit: usize) -> Result<Vec<CallSession>> {
        Ok(CallsRepository::get_call_history(&self.db, limit)?)
    }

    /// End a call locally after a non-signaling failure, such as transport
    /// failure while sending an offer/answer. This records terminal metadata
    /// without creating or persisting any SDP/ICE/media payloads.
    pub fn end_call_locally(&self, call_id: &str, reason: &str) -> Result<()> {
        Self::validate_hangup_reason(reason)?;
        self.end_active_call(call_id, reason)
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
        Self::validate_sdp(sdp)?;
        self.require_no_active_call_with_peer(callee_peer_id)?;

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
        self.insert_outgoing_call(
            &call_id,
            &identity.peer_id,
            callee_peer_id,
            timestamp,
            Self::media_kind_from_sdp(sdp),
        )?;

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

        Self::validate_sdp(sdp)?;
        if CallsRepository::get_by_call_id(&self.db, call_id)?.is_some() {
            return Err(AppError::AlreadyExists(
                "Call session already exists".to_string(),
            ));
        }
        self.require_no_active_call_with_peer(caller_peer_id)?;
        self.insert_incoming_call(
            call_id,
            caller_peer_id,
            callee_peer_id,
            timestamp,
            Self::media_kind_from_sdp(sdp),
        )?;

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

        if !self
            .permissions_service
            .we_have_capability(caller_peer_id, Capability::Call)?
        {
            return Err(AppError::PermissionDenied(
                "No call permission from caller".to_string(),
            ));
        }
        let call = self.require_call_state(call_id, CallState::Incoming)?;
        if call.caller_peer_id.as_deref() != Some(caller_peer_id) || call.peer_id != caller_peer_id
        {
            return Err(AppError::Validation(
                "Caller does not match incoming call".to_string(),
            ));
        }
        Self::validate_sdp(sdp)?;

        let timestamp = chrono::Utc::now().timestamp();

        let signable = SignableSignalingAnswer {
            call_id: call_id.to_string(),
            caller_peer_id: caller_peer_id.to_string(),
            callee_peer_id: identity.peer_id.clone(),
            sdp: sdp.to_string(),
            timestamp,
        };

        let signature = self.identity_service.sign(&signable)?;
        self.mark_connected(call_id)?;

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

        if !self
            .permissions_service
            .peer_has_capability(callee_peer_id, Capability::Call)?
        {
            return Err(AppError::PermissionDenied(
                "Callee doesn't have call permission".to_string(),
            ));
        }

        let call = self.require_call_state(call_id, CallState::Ringing)?;
        if call.caller_peer_id.as_deref() != Some(caller_peer_id)
            || call.callee_peer_id.as_deref() != Some(callee_peer_id)
            || call.peer_id != callee_peer_id
        {
            return Err(AppError::Validation(
                "Answer does not match the outgoing call".to_string(),
            ));
        }
        self.mark_connected(call_id)?;

        Ok(())
    }

    /// Send an ICE candidate
    pub fn create_ice_candidate(
        &self,
        call_id: &str,
        target_peer_id: &str,
        candidate: &str,
        sdp_mid: Option<&str>,
        sdp_mline_index: Option<u32>,
    ) -> Result<OutgoingIce> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        self.require_any_call_grant_with(target_peer_id)?;
        Self::validate_ice_candidate(candidate)?;

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
            target_peer_id: target_peer_id.to_string(),
            candidate: candidate.to_string(),
            sdp_mid: sdp_mid.map(String::from),
            sdp_mline_index,
            timestamp,
            signature,
        })
    }

    /// Process an incoming ICE candidate
    pub fn process_incoming_ice(&self, params: &IncomingIceParams<'_>) -> Result<()> {
        let call_id = params.call_id;
        let sender_peer_id = params.sender_peer_id;
        let candidate = params.candidate;
        let sdp_mid = params.sdp_mid;
        let sdp_mline_index = params.sdp_mline_index;
        let timestamp = params.timestamp;
        let signature = params.signature;
        Self::validate_ice_candidate(candidate)?;
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
    pub fn create_hangup(
        &self,
        call_id: &str,
        target_peer_id: &str,
        reason: &str,
    ) -> Result<OutgoingHangup> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        self.require_any_call_grant_with(target_peer_id)?;
        Self::validate_hangup_reason(reason)?;
        let call = self.ensure_call_is_active(call_id)?;
        if call.peer_id != target_peer_id {
            return Err(AppError::Validation(
                "Hangup target does not match call peer".to_string(),
            ));
        }

        let timestamp = chrono::Utc::now().timestamp();

        let signable = SignableSignalingHangup {
            call_id: call_id.to_string(),
            sender_peer_id: identity.peer_id.clone(),
            reason: reason.to_string(),
            timestamp,
        };

        let signature = self.identity_service.sign(&signable)?;
        self.end_active_call(call_id, reason)?;

        Ok(OutgoingHangup {
            call_id: call_id.to_string(),
            sender_peer_id: identity.peer_id,
            target_peer_id: target_peer_id.to_string(),
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

        Self::validate_hangup_reason(reason)?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto("Invalid hangup signature".to_string()));
        }

        let call = self.ensure_call_is_active(call_id)?;
        if call.peer_id != sender_peer_id {
            return Err(AppError::Validation(
                "Hangup sender does not match call peer".to_string(),
            ));
        }
        self.end_active_call(call_id, reason)?;

        Ok(())
    }

    /// Decline an incoming call using the signed hangup payload with a
    /// `declined` reason.
    pub fn create_decline(&self, call_id: &str, caller_peer_id: &str) -> Result<OutgoingHangup> {
        let call = self.require_call_state(call_id, CallState::Incoming)?;
        if call.peer_id != caller_peer_id {
            return Err(AppError::Validation(
                "Decline target does not match incoming call".to_string(),
            ));
        }
        self.create_hangup(call_id, caller_peer_id, "declined")
    }

    /// Send a busy response using the signed hangup payload with a `busy` reason.
    ///
    /// Busy is the one terminal response allowed for an incoming call ID that
    /// was not admitted into active state, so duplicate simultaneous offers can
    /// be explicitly handled without treating the unknown call as a normal
    /// hangup.
    pub fn create_busy(&self, call_id: &str, caller_peer_id: &str) -> Result<OutgoingHangup> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        if !self
            .permissions_service
            .we_have_capability(caller_peer_id, Capability::Call)?
        {
            return Err(AppError::PermissionDenied(
                "No call permission from caller".to_string(),
            ));
        }

        if let Some(call) = CallsRepository::get_by_call_id(&self.db, call_id)? {
            if call.state == CallState::Ended {
                return Err(AppError::Validation("Call has already ended".to_string()));
            }
            if call.peer_id != caller_peer_id || call.direction != CallDirection::Incoming {
                return Err(AppError::Validation(
                    "Busy target does not match an incoming call".to_string(),
                ));
            }
        }

        let timestamp = chrono::Utc::now().timestamp();
        let signable = SignableSignalingHangup {
            call_id: call_id.to_string(),
            sender_peer_id: identity.peer_id.clone(),
            reason: "busy".to_string(),
            timestamp,
        };
        let signature = self.identity_service.sign(&signable)?;

        if CallsRepository::get_by_call_id(&self.db, call_id)?.is_some() {
            self.end_active_call(call_id, "busy")?;
        } else {
            self.insert_busy_history_if_unknown(
                call_id,
                caller_peer_id,
                &identity.peer_id,
                timestamp,
            )?;
        }

        Ok(OutgoingHangup {
            call_id: call_id.to_string(),
            sender_peer_id: identity.peer_id,
            target_peer_id: caller_peer_id.to_string(),
            reason: "busy".to_string(),
            timestamp,
            signature,
        })
    }

    /// Validate a complete incoming signaling envelope from the libp2p transport.
    ///
    /// This is the production ingress path used before emitting frontend call
    /// events.  It verifies the contact key binding, target peer, timestamp
    /// freshness, signed payload, call grants, and replay cache.
    pub fn process_incoming_signaling(&self, envelope: &SignalingEnvelope) -> Result<()> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        if envelope.recipient_peer_id != identity.peer_id {
            return Err(AppError::Validation(
                "Signaling message not for us".to_string(),
            ));
        }

        Self::validate_recent_timestamp(envelope.timestamp())?;
        self.validate_contact_identity(&envelope.sender_peer_id)?;

        match &envelope.payload {
            SignalingPayload::GroupMembership(signal) => {
                if signal.sender_peer_id != envelope.sender_peer_id {
                    return Err(AppError::Validation(
                        "Group membership sender does not match envelope".to_string(),
                    ));
                }
                self.process_group_membership(signal)?;
            }
            SignalingPayload::Offer(offer) => {
                if offer.caller_peer_id != envelope.sender_peer_id {
                    return Err(AppError::Validation(
                        "Offer caller does not match signaling sender".to_string(),
                    ));
                }
                if offer.callee_peer_id != envelope.recipient_peer_id {
                    return Err(AppError::Validation(
                        "Offer callee does not match signaling recipient".to_string(),
                    ));
                }
                Self::validate_sdp(&offer.sdp)?;
                self.process_incoming_offer(
                    &offer.call_id,
                    &offer.caller_peer_id,
                    &offer.callee_peer_id,
                    &offer.sdp,
                    offer.timestamp,
                    &offer.signature,
                )?;
            }
            SignalingPayload::Answer(answer) => {
                if answer.callee_peer_id != envelope.sender_peer_id {
                    return Err(AppError::Validation(
                        "Answer callee does not match signaling sender".to_string(),
                    ));
                }
                if answer.caller_peer_id != envelope.recipient_peer_id {
                    return Err(AppError::Validation(
                        "Answer caller does not match signaling recipient".to_string(),
                    ));
                }
                Self::validate_sdp(&answer.sdp)?;
                self.process_incoming_answer(
                    &answer.call_id,
                    &answer.caller_peer_id,
                    &answer.callee_peer_id,
                    &answer.sdp,
                    answer.timestamp,
                    &answer.signature,
                )?;
            }
            SignalingPayload::Ice(ice) => {
                if ice.sender_peer_id != envelope.sender_peer_id {
                    return Err(AppError::Validation(
                        "ICE sender does not match signaling sender".to_string(),
                    ));
                }
                self.require_any_call_grant_with(&ice.sender_peer_id)?;
                self.process_incoming_ice(&IncomingIceParams {
                    call_id: &ice.call_id,
                    sender_peer_id: &ice.sender_peer_id,
                    candidate: &ice.candidate,
                    sdp_mid: ice.sdp_mid.as_deref(),
                    sdp_mline_index: ice.sdp_mline_index,
                    timestamp: ice.timestamp,
                    signature: &ice.signature,
                })?;
            }
            SignalingPayload::Hangup(hangup) => {
                if hangup.sender_peer_id != envelope.sender_peer_id {
                    return Err(AppError::Validation(
                        "Hangup sender does not match signaling sender".to_string(),
                    ));
                }
                self.require_any_call_grant_with(&hangup.sender_peer_id)?;
                self.process_incoming_hangup(
                    &hangup.call_id,
                    &hangup.sender_peer_id,
                    &hangup.reason,
                    hangup.timestamp,
                    &hangup.signature,
                )?;
            }
            SignalingPayload::Decline(decline) => {
                if decline.reason != "declined" {
                    return Err(AppError::Validation(
                        "Decline signaling must use declined reason".to_string(),
                    ));
                }
                if decline.sender_peer_id != envelope.sender_peer_id {
                    return Err(AppError::Validation(
                        "Decline sender does not match signaling sender".to_string(),
                    ));
                }
                self.require_any_call_grant_with(&decline.sender_peer_id)?;
                self.process_incoming_hangup(
                    &decline.call_id,
                    &decline.sender_peer_id,
                    &decline.reason,
                    decline.timestamp,
                    &decline.signature,
                )?;
            }
            SignalingPayload::Busy(busy) => {
                if busy.reason != "busy" {
                    return Err(AppError::Validation(
                        "Busy signaling must use busy reason".to_string(),
                    ));
                }
                if busy.sender_peer_id != envelope.sender_peer_id {
                    return Err(AppError::Validation(
                        "Busy sender does not match signaling sender".to_string(),
                    ));
                }
                self.require_any_call_grant_with(&busy.sender_peer_id)?;
                self.process_incoming_hangup(
                    &busy.call_id,
                    &busy.sender_peer_id,
                    &busy.reason,
                    busy.timestamp,
                    &busy.signature,
                )?;
            }
        }

        self.record_signaling_once(envelope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        CallMediaKind, CallsRepository, Capability, ContactData, ContactsRepository, GrantData,
        PermissionsRepository,
    };
    use crate::models::CreateIdentityRequest;
    use crate::p2p::protocols::signaling::{
        SignalingEnvelope, SignalingHangup, SignalingIce, SignalingOffer, SignalingPayload,
    };
    use crate::services::{ContactsService, CryptoService, IdentityService, PermissionsService};
    use crate::Database;
    use base64::Engine;
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

    fn add_peer_contact(db: &Database, peer_id: &str, public_key: &[u8]) {
        let contact_data = ContactData {
            peer_id: peer_id.to_string(),
            public_key: public_key.to_vec(),
            x25519_public: vec![0u8; 32],
            display_name: "Peer".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(db, &contact_data).unwrap();
    }

    fn add_received_call_permission(db: &Database, issuer_peer_id: &str, subject_peer_id: &str) {
        let grant_data = GrantData {
            grant_id: format!("grant-call-{}-{}", issuer_peer_id, subject_peer_id),
            issuer_peer_id: issuer_peer_id.to_string(),
            subject_peer_id: subject_peer_id.to_string(),
            capability: "call".to_string(),
            scope_json: None,
            lamport_clock: 1,
            issued_at: chrono::Utc::now().timestamp(),
            expires_at: None,
            payload_cbor: vec![0],
            signature: vec![0],
        };
        PermissionsRepository::upsert_grant(db, &grant_data).unwrap();
    }

    fn identity_public_key(identity: &IdentityService) -> Vec<u8> {
        let info = identity.get_identity_info().unwrap().unwrap();
        base64::engine::general_purpose::STANDARD
            .decode(info.public_key)
            .unwrap()
    }

    fn insert_incoming_test_call(
        service: &CallingService,
        call_id: &str,
        caller_peer_id: &str,
        callee_peer_id: &str,
    ) {
        service
            .insert_incoming_call(
                call_id,
                caller_peer_id,
                callee_peer_id,
                chrono::Utc::now().timestamp(),
                CallMediaKind::Audio,
            )
            .unwrap();
    }

    fn insert_outgoing_test_call(
        service: &CallingService,
        call_id: &str,
        caller_peer_id: &str,
        callee_peer_id: &str,
    ) {
        service
            .insert_outgoing_call(
                call_id,
                caller_peer_id,
                callee_peer_id,
                chrono::Utc::now().timestamp(),
                CallMediaKind::Audio,
            )
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

        let active = service.get_active_calls().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].call_id, offer.call_id);
        assert_eq!(active[0].state, CallState::Ringing);
        assert_eq!(active[0].media_kind, CallMediaKind::Audio);
    }

    #[test]
    fn test_create_video_offer_persists_video_media_kind() {
        let (service, db, _identity, permissions, _peer_id) = create_test_env();

        let (_, peer_verifying) = CryptoService::generate_ed25519_keypair();
        let callee = "12D3KooWCalleeVideo";
        add_peer_with_call_permission(&db, &permissions, callee, &peer_verifying.to_bytes());

        let offer = service
            .create_offer(
                callee,
                "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\nm=video 9 UDP/TLS/RTP/SAVPF 96",
            )
            .unwrap();

        let active = service.get_active_calls().unwrap();
        assert_eq!(active[0].call_id, offer.call_id);
        assert_eq!(active[0].media_kind, CallMediaKind::Video);
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
        let (service, db, _identity, _permissions, peer_id) = create_test_env();
        let (_, caller_verifying) = CryptoService::generate_ed25519_keypair();
        let caller = "12D3KooWCaller";
        add_peer_contact(&db, caller, &caller_verifying.to_bytes());
        add_received_call_permission(&db, caller, &peer_id);
        insert_incoming_test_call(&service, "call-123", caller, &peer_id);

        let answer = service
            .create_answer("call-123", caller, "v=0\r\nsdp-answer")
            .unwrap();

        assert_eq!(answer.call_id, "call-123");
        assert_eq!(answer.caller_peer_id, "12D3KooWCaller");
        assert_eq!(answer.callee_peer_id, peer_id);
        assert_eq!(answer.sdp, "v=0\r\nsdp-answer");
        assert!(!answer.signature.is_empty());

        let call = CallsRepository::get_by_call_id(&db, "call-123")
            .unwrap()
            .unwrap();
        assert_eq!(call.state, CallState::Connected);
    }

    #[test]
    fn test_create_answer_rejects_double_answer_and_ended_call() {
        let (service, db, _identity, _permissions, peer_id) = create_test_env();
        let (_, caller_verifying) = CryptoService::generate_ed25519_keypair();
        let caller = "12D3KooWCaller";
        add_peer_contact(&db, caller, &caller_verifying.to_bytes());
        add_received_call_permission(&db, caller, &peer_id);
        insert_incoming_test_call(&service, "call-double", caller, &peer_id);

        service
            .create_answer("call-double", caller, "v=0\r\nsdp-answer")
            .unwrap();
        let double_answer = service.create_answer("call-double", caller, "v=0\r\nsdp-answer");
        assert!(matches!(double_answer, Err(AppError::Validation(_))));

        service.end_active_call("call-double", "normal").unwrap();
        let ended_answer = service.create_answer("call-double", caller, "v=0\r\nsdp-answer");
        assert!(matches!(ended_answer, Err(AppError::Validation(_))));
    }

    #[test]
    fn test_hangup_rejects_unknown_call() {
        let (service, db, _identity, permissions, _peer_id) = create_test_env();
        let (_, peer_verifying) = CryptoService::generate_ed25519_keypair();
        let target = "12D3KooWPeer";
        add_peer_with_call_permission(&db, &permissions, target, &peer_verifying.to_bytes());

        let result = service.create_hangup("missing-call", target, "normal");
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn test_process_incoming_offer_rejects_duplicate_active_session() {
        let (service, db, _identity, _permissions, peer_id) = create_test_env();

        let (caller_signing, caller_verifying) = CryptoService::generate_ed25519_keypair();
        let caller_id = "12D3KooWCaller123";
        add_peer_contact(&db, caller_id, &caller_verifying.to_bytes());
        add_received_call_permission(&db, caller_id, &peer_id);
        insert_incoming_test_call(&service, "existing-call", caller_id, &peer_id);

        let signable = SignableSignalingOffer {
            call_id: "new-call".to_string(),
            caller_peer_id: caller_id.to_string(),
            callee_peer_id: peer_id.clone(),
            sdp: "v=0\r\nsdp".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        let sig = crate::services::sign(&caller_signing, &signable).unwrap();

        let result = service.process_incoming_offer(
            "new-call",
            caller_id,
            &peer_id,
            "v=0\r\nsdp",
            signable.timestamp,
            &sig,
        );

        assert!(matches!(result, Err(AppError::AlreadyExists(_))));
    }

    #[test]
    fn test_create_busy_records_unknown_duplicate_as_terminal_history() {
        let (service, db, _identity, _permissions, peer_id) = create_test_env();
        let (_, caller_verifying) = CryptoService::generate_ed25519_keypair();
        let caller = "12D3KooWCaller";
        add_peer_contact(&db, caller, &caller_verifying.to_bytes());
        add_received_call_permission(&db, caller, &peer_id);

        let busy = service.create_busy("duplicate-call", caller).unwrap();
        assert_eq!(busy.reason, "busy");
        let record = CallsRepository::get_by_call_id(&db, "duplicate-call")
            .unwrap()
            .unwrap();
        assert_eq!(record.state, CallState::Ended);
        assert_eq!(record.terminal_reason.as_deref(), Some("busy"));
        assert_eq!(record.duration_seconds, Some(0));
    }

    #[test]
    fn test_call_history_available_after_service_restart() {
        let (service, db, identity_service, permissions, peer_id) = create_test_env();
        let (_, peer_verifying) = CryptoService::generate_ed25519_keypair();
        let target = "12D3KooWPeer";
        add_peer_with_call_permission(&db, &permissions, target, &peer_verifying.to_bytes());
        let offer = service.create_offer(target, "v=0\r\nsdp-data").unwrap();

        let restarted_contacts =
            Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
        let restarted_permissions = Arc::new(PermissionsService::new(
            db.clone(),
            identity_service.clone(),
        ));
        let restarted = CallingService::new(
            db.clone(),
            identity_service,
            restarted_contacts,
            restarted_permissions,
        );

        let active = restarted.get_active_calls().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].call_id, offer.call_id);
        assert_eq!(active[0].caller_peer_id.as_deref(), Some(peer_id.as_str()));
        assert_eq!(active[0].callee_peer_id.as_deref(), Some(target));
    }

    #[test]
    fn test_create_ice_candidate() {
        let (service, db, _identity, permissions, peer_id) = create_test_env();
        let (_, peer_verifying) = CryptoService::generate_ed25519_keypair();
        let target = "12D3KooWPeer";
        add_peer_with_call_permission(&db, &permissions, target, &peer_verifying.to_bytes());

        let ice = service
            .create_ice_candidate(
                "call-123",
                target,
                "candidate:0 1 UDP",
                Some("audio"),
                Some(0),
            )
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
        let (service, db, _identity, permissions, _peer_id) = create_test_env();
        let (_, peer_verifying) = CryptoService::generate_ed25519_keypair();
        let target = "12D3KooWPeer";
        add_peer_with_call_permission(&db, &permissions, target, &peer_verifying.to_bytes());

        let ice = service
            .create_ice_candidate("call-123", target, "candidate:0 1 UDP", None, None)
            .unwrap();

        assert_eq!(ice.sdp_mid, None);
        assert_eq!(ice.sdp_mline_index, None);
    }

    #[test]
    fn test_create_hangup() {
        let (service, db, _identity, permissions, peer_id) = create_test_env();
        let (_, peer_verifying) = CryptoService::generate_ed25519_keypair();
        let target = "12D3KooWPeer";
        add_peer_with_call_permission(&db, &permissions, target, &peer_verifying.to_bytes());
        insert_outgoing_test_call(&service, "call-123", &peer_id, target);

        let hangup = service.create_hangup("call-123", target, "normal").unwrap();

        assert_eq!(hangup.call_id, "call-123");
        assert_eq!(hangup.sender_peer_id, peer_id);
        assert_eq!(hangup.reason, "normal");
        assert!(!hangup.signature.is_empty());
        let ended = CallsRepository::get_by_call_id(&db, "call-123")
            .unwrap()
            .unwrap();
        assert_eq!(ended.state, CallState::Ended);
        assert_eq!(ended.terminal_reason.as_deref(), Some("normal"));
    }

    #[test]
    fn test_create_hangup_various_reasons() {
        let (service, db, _identity, permissions, peer_id) = create_test_env();
        let (_, peer_verifying) = CryptoService::generate_ed25519_keypair();
        let target = "12D3KooWPeer";
        add_peer_with_call_permission(&db, &permissions, target, &peer_verifying.to_bytes());

        for reason in &["normal", "busy", "declined", "error"] {
            let call_id = format!("call-{}", reason);
            insert_outgoing_test_call(&service, &call_id, &peer_id, target);
            let hangup = service.create_hangup(&call_id, target, reason).unwrap();
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

        let result = service.process_incoming_ice(&IncomingIceParams {
            call_id: "call-1",
            sender_peer_id: sender_id,
            candidate: "candidate:0 1 UDP",
            sdp_mid: Some("audio"),
            sdp_mline_index: Some(0),
            timestamp: signable.timestamp,
            signature: &sig,
        });

        assert!(result.is_ok());
    }

    #[test]
    fn test_process_incoming_hangup_valid() {
        let (service, db, _identity, _permissions, peer_id) = create_test_env();

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
        insert_outgoing_test_call(&service, "call-1", &peer_id, sender_id);

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
        let ended = CallsRepository::get_by_call_id(&db, "call-1")
            .unwrap()
            .unwrap();
        assert_eq!(ended.state, CallState::Ended);
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

    fn signed_offer_envelope(
        caller_identity: &IdentityService,
        caller_peer_id: &str,
        callee_peer_id: &str,
        timestamp: i64,
    ) -> SignalingEnvelope {
        let signable = SignableSignalingOffer {
            call_id: "call-signal-1".to_string(),
            caller_peer_id: caller_peer_id.to_string(),
            callee_peer_id: callee_peer_id.to_string(),
            sdp: "v=0\r\ns=Harbor\r\n".to_string(),
            timestamp,
        };
        let signature = caller_identity.sign(&signable).unwrap();
        SignalingEnvelope {
            sender_peer_id: caller_peer_id.to_string(),
            recipient_peer_id: callee_peer_id.to_string(),
            payload: SignalingPayload::Offer(SignalingOffer {
                call_id: signable.call_id,
                caller_peer_id: signable.caller_peer_id,
                callee_peer_id: signable.callee_peer_id,
                sdp: signable.sdp,
                timestamp: signable.timestamp,
                signature,
            }),
        }
    }

    #[test]
    fn test_process_incoming_signaling_valid_offer_then_duplicate_rejected() {
        let (_caller_service, _caller_db, caller_identity, _caller_permissions, caller_peer_id) =
            create_test_env();
        let (callee_service, callee_db, _callee_identity, _callee_permissions, callee_peer_id) =
            create_test_env();

        add_peer_contact(
            &callee_db,
            &caller_peer_id,
            &identity_public_key(&caller_identity),
        );
        add_received_call_permission(&callee_db, &caller_peer_id, &callee_peer_id);

        let envelope = signed_offer_envelope(
            &caller_identity,
            &caller_peer_id,
            &callee_peer_id,
            chrono::Utc::now().timestamp(),
        );

        assert!(callee_service.process_incoming_signaling(&envelope).is_ok());
        let duplicate = callee_service.process_incoming_signaling(&envelope);
        assert!(matches!(duplicate, Err(AppError::AlreadyExists(_))));
    }

    #[test]
    fn test_process_incoming_signaling_rejects_wrong_recipient() {
        let (_caller_service, _caller_db, caller_identity, _caller_permissions, caller_peer_id) =
            create_test_env();
        let (callee_service, _callee_db, _callee_identity, _callee_permissions, callee_peer_id) =
            create_test_env();

        let mut envelope = signed_offer_envelope(
            &caller_identity,
            &caller_peer_id,
            &callee_peer_id,
            chrono::Utc::now().timestamp(),
        );
        envelope.recipient_peer_id = "12D3KooWNotUs".to_string();

        let result = callee_service.process_incoming_signaling(&envelope);
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[test]
    fn test_process_incoming_signaling_rejects_missing_permission() {
        let (_caller_service, _caller_db, caller_identity, _caller_permissions, caller_peer_id) =
            create_test_env();
        let (callee_service, callee_db, _callee_identity, _callee_permissions, callee_peer_id) =
            create_test_env();

        add_peer_contact(
            &callee_db,
            &caller_peer_id,
            &identity_public_key(&caller_identity),
        );

        let envelope = signed_offer_envelope(
            &caller_identity,
            &caller_peer_id,
            &callee_peer_id,
            chrono::Utc::now().timestamp(),
        );

        let result = callee_service.process_incoming_signaling(&envelope);
        assert!(matches!(result, Err(AppError::PermissionDenied(_))));
    }

    #[test]
    fn test_process_incoming_signaling_rejects_invalid_signature() {
        let (_caller_service, _caller_db, caller_identity, _caller_permissions, caller_peer_id) =
            create_test_env();
        let (callee_service, callee_db, _callee_identity, _callee_permissions, callee_peer_id) =
            create_test_env();

        add_peer_contact(
            &callee_db,
            &caller_peer_id,
            &identity_public_key(&caller_identity),
        );
        add_received_call_permission(&callee_db, &caller_peer_id, &callee_peer_id);

        let mut envelope = signed_offer_envelope(
            &caller_identity,
            &caller_peer_id,
            &callee_peer_id,
            chrono::Utc::now().timestamp(),
        );
        if let SignalingPayload::Offer(offer) = &mut envelope.payload {
            offer.signature = vec![0u8; 64];
        }

        let result = callee_service.process_incoming_signaling(&envelope);
        assert!(matches!(result, Err(AppError::Crypto(_))));
    }

    #[test]
    fn test_process_incoming_signaling_rejects_stale_timestamp() {
        let (_caller_service, _caller_db, caller_identity, _caller_permissions, caller_peer_id) =
            create_test_env();
        let (callee_service, callee_db, _callee_identity, _callee_permissions, callee_peer_id) =
            create_test_env();

        add_peer_contact(
            &callee_db,
            &caller_peer_id,
            &identity_public_key(&caller_identity),
        );
        add_received_call_permission(&callee_db, &caller_peer_id, &callee_peer_id);

        let stale = chrono::Utc::now().timestamp() - MAX_SIGNALING_TIMESTAMP_SKEW_SECONDS - 1;
        let envelope =
            signed_offer_envelope(&caller_identity, &caller_peer_id, &callee_peer_id, stale);

        let result = callee_service.process_incoming_signaling(&envelope);
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[test]
    fn test_process_incoming_signaling_valid_ice_and_busy_payloads() {
        let (_sender_service, _sender_db, sender_identity, _sender_permissions, sender_peer_id) =
            create_test_env();
        let (
            receiver_service,
            receiver_db,
            _receiver_identity,
            receiver_permissions,
            receiver_peer_id,
        ) = create_test_env();

        add_peer_with_call_permission(
            &receiver_db,
            &receiver_permissions,
            &sender_peer_id,
            &identity_public_key(&sender_identity),
        );

        let timestamp = chrono::Utc::now().timestamp();
        let ice_signable = SignableSignalingIce {
            call_id: "call-ice-1".to_string(),
            sender_peer_id: sender_peer_id.clone(),
            candidate: "candidate:0 1 UDP".to_string(),
            sdp_mid: Some("audio".to_string()),
            sdp_mline_index: Some(0),
            timestamp,
        };
        let ice_signature = sender_identity.sign(&ice_signable).unwrap();
        let ice_envelope = SignalingEnvelope {
            sender_peer_id: sender_peer_id.clone(),
            recipient_peer_id: receiver_peer_id.clone(),
            payload: SignalingPayload::Ice(SignalingIce {
                call_id: ice_signable.call_id,
                sender_peer_id: ice_signable.sender_peer_id,
                candidate: ice_signable.candidate,
                sdp_mid: ice_signable.sdp_mid,
                sdp_mline_index: ice_signable.sdp_mline_index,
                timestamp: ice_signable.timestamp,
                signature: ice_signature,
            }),
        };
        assert!(receiver_service
            .process_incoming_signaling(&ice_envelope)
            .is_ok());

        insert_outgoing_test_call(
            &receiver_service,
            "call-busy-1",
            &receiver_peer_id,
            &sender_peer_id,
        );

        let busy_timestamp = chrono::Utc::now().timestamp();
        let busy_signable = SignableSignalingHangup {
            call_id: "call-busy-1".to_string(),
            sender_peer_id: sender_peer_id.clone(),
            reason: "busy".to_string(),
            timestamp: busy_timestamp,
        };
        let busy_signature = sender_identity.sign(&busy_signable).unwrap();
        let busy_envelope = SignalingEnvelope {
            sender_peer_id,
            recipient_peer_id: receiver_peer_id,
            payload: SignalingPayload::Busy(SignalingHangup {
                call_id: busy_signable.call_id,
                sender_peer_id: busy_signable.sender_peer_id,
                reason: busy_signable.reason,
                timestamp: busy_signable.timestamp,
                signature: busy_signature,
            }),
        };
        assert!(receiver_service
            .process_incoming_signaling(&busy_envelope)
            .is_ok());
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

        let result = service.create_hangup("call-123", "12D3KooWPeer", "normal");
        assert!(result.is_err());
    }

    #[test]
    fn test_group_invite_is_signed_persisted_and_replay_safe() {
        let (alice, alice_db, alice_identity, alice_permissions, alice_peer) = create_test_env();
        let (bob, bob_db, bob_identity, bob_permissions, bob_peer) = create_test_env();
        add_peer_with_call_permission(
            &alice_db,
            &alice_permissions,
            &bob_peer,
            &identity_public_key(&bob_identity),
        );
        add_peer_with_call_permission(
            &bob_db,
            &bob_permissions,
            &alice_peer,
            &identity_public_key(&alice_identity),
        );

        let mut roster = vec![alice_peer.clone(), bob_peer.clone()];
        roster.sort();
        let invite = alice
            .create_group_membership(
                Some("room-signed"),
                None,
                GroupMembershipAction::Invite,
                1,
                &roster,
                "video",
            )
            .unwrap();
        let envelope = SignalingEnvelope {
            sender_peer_id: alice_peer,
            recipient_peer_id: bob_peer,
            payload: SignalingPayload::GroupMembership(invite),
        };

        bob.process_incoming_signaling(&envelope).unwrap();
        let room = GroupCallsRepository::get(&bob_db, "room-signed")
            .unwrap()
            .unwrap();
        assert_eq!(room.roster_version, 1);
        assert_eq!(room.participants, roster);
        assert_eq!(room.state, "invited");
        assert!(bob.process_incoming_signaling(&envelope).is_err());
    }
}
