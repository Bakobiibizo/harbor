use base64::Engine;
use chrono::Utc;
use futures::StreamExt;
use libp2p::{
    autonat, dcutr, identify, kad, mdns, ping, relay,
    request_response::{self, ResponseChannel},
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm,
};
use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

/// Public relay servers that support libp2p relay v2
/// Only Harbor relay servers are listed here. IPFS bootstrap nodes use relay v1
/// and RSA-based peer IDs that are incompatible with relay v2.
const PUBLIC_RELAYS: &[&str] = &[
    // Harbor community relay (primary)
    "/ip4/100.49.236.191/tcp/4001/p2p/12D3KooWMfwHKfzDrZ2V3Zniw3Qu797bHrKsFKAdG9CtQiaEhbQ3",
];

/// Application-level deadline for a direct-message request. This is kept
/// independent of libp2p's timeout so abandoned or stalled requests cannot
/// retain callers indefinitely.
const MESSAGE_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const PENDING_REQUEST_MAINTENANCE_INTERVAL: Duration = Duration::from_millis(250);
const MESSAGE_OUTBOX_CLAIM_LEASE_SECONDS: u32 = 30;
const MESSAGE_OUTBOX_MAX_IN_FLIGHT: usize = 32;
const MESSAGE_OUTBOX_BATCH_SIZE: usize = 16;
const POST_RELAY_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const POST_RELAY_OUTBOX_CLAIM_LEASE_SECONDS: i64 = 35;
const POST_RELAY_OUTBOX_MAX_IN_FLIGHT: usize = 16;
const POST_RELAY_OUTBOX_BATCH_SIZE: usize = 8;

fn message_runtime_unavailable(event_id: &str, message_id: &str) -> MessageDeliveryFailure {
    MessageDeliveryFailure {
        event_id: event_id.to_string(),
        message_id: message_id.to_string(),
        kind: MessageDeliveryFailureKind::RuntimeUnavailable,
        detail: "network service is unavailable".into(),
    }
}

use super::behaviour::{
    ChatBehaviour, ChatBehaviourEvent, ContentSyncRequest, ContentSyncResponse,
    IdentityExchangeRequest, IdentityExchangeResponse, MessagingRequest, MessagingResponse,
    PostSummaryProto,
};
use super::config::NetworkConfig;
use super::protocols::board_sync::{
    BoardSyncRequest as WireBoardSyncRequest, BoardSyncResponse as WireBoardSyncResponse,
    NameClaimRequest, SignedNameClaimRequest, WallSocialEventItem,
};
use super::protocols::messaging::{MessagingCodec, MessagingMessage};
use super::protocols::signaling::{SignalingEnvelope, SignalingPayload, SignalingResponse};
use super::swarm::build_swarm;
use super::types::*;
use crate::db::Capability;
use crate::error::{AppError, Result};
use crate::services::board_service::StorableBoardPost;
use crate::services::content_sync_service::RemotePostParams;
use crate::services::mentions_service::IncomingMentionEnvelope;
use crate::services::messaging_service::{IncomingMessageEditParams, IncomingMessageParams};
use crate::services::{
    BoardService, CallingService, ContactsService, ContentSyncService, IdentityService,
    IncomingWallSocialEventParams, MediaStorageService, MediaTransferUpdate, MentionsService,
    MessagingService, PermissionsService, PostsService, SignableGetWallPosts,
    SignableGetWallSocialEvents, SignableWallPostSubmit, SignableWallSocialEventSubmit,
    WallSocialService,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

const POW_MAX_DIFFICULTY: u8 = 24;
const POW_MAX_DURATION: Duration = Duration::from_secs(10);
const POW_MAX_ATTEMPTS: u64 = 25_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkSolveError {
    Cancelled,
    DeadlineExceeded,
    WorkBudgetExceeded,
}

fn solve_work_bounded(
    c: &super::protocols::board_sync::WorkChallenge,
    cancelled: &AtomicBool,
    deadline: Instant,
    max_attempts: u64,
) -> std::result::Result<u64, WorkSolveError> {
    use sha2::{Digest, Sha256};
    for nonce in 0..max_attempts {
        if nonce % 1024 == 0 {
            if cancelled.load(Ordering::Relaxed) {
                return Err(WorkSolveError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(WorkSolveError::DeadlineExceeded);
            }
        }
        let mut h = Sha256::new();
        for part in [
            "harbor-pow-v1",
            &c.relay,
            &c.id,
            &c.requester,
            &c.target,
            &c.action,
            &c.audience,
            &c.expires_at.to_string(),
            &nonce.to_string(),
        ] {
            h.update((part.len() as u32).to_be_bytes());
            h.update(part.as_bytes())
        }
        let d = h.finalize();
        let bits = d.iter().take_while(|b| **b == 0).count() as u8 * 8
            + d.iter()
                .find(|b| **b != 0)
                .map_or(0, |b| b.leading_zeros() as u8);
        if bits >= c.difficulty {
            return Ok(nonce);
        }
    }
    Err(WorkSolveError::WorkBudgetExceeded)
}

fn spawn_work_solver(
    challenge: super::protocols::board_sync::WorkChallenge,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
) -> tokio::task::JoinHandle<std::result::Result<u64, WorkSolveError>> {
    tokio::task::spawn_blocking(move || {
        solve_work_bounded(&challenge, &cancelled, deadline, POW_MAX_ATTEMPTS)
    })
}

fn validate_relay_auth_challenge(
    peer: PeerId,
    local_peer_id: &str,
    expected_audience: &str,
    challenge: &super::protocols::board_sync::RelayAuthChallenge,
    now: i64,
) -> std::result::Result<libp2p::identity::PublicKey, &'static str> {
    if challenge.domain != "harbor/relay-challenge/1"
        || challenge.version != 1
        || challenge.peer_id != local_peer_id
        || challenge.audience != expected_audience
        || challenge.id.is_empty()
        || challenge.id.len() > 128
        || challenge.relay.is_empty()
        || challenge.relay.len() > 255
        || challenge.key_id.is_empty()
        || challenge.key_id.len() > 128
        || challenge.nonce.is_empty()
        || challenge.nonce.len() > 128
        || challenge.relay_public_key.is_empty()
        || challenge.relay_public_key.len() > 128
        || challenge.relay_signature.is_empty()
        || challenge.relay_signature.len() > 256
        || challenge.issued_at > now + 30
        || challenge.expires_at < now
        || challenge.expires_at < challenge.issued_at
        || challenge.expires_at - challenge.issued_at > 180
    {
        return Err("RELAY_AUTH_CHALLENGE_INVALID");
    }
    let public = libp2p::identity::PublicKey::try_decode_protobuf(&challenge.relay_public_key)
        .map_err(|_| "RELAY_AUTH_KEY_INVALID")?;
    if PeerId::from_public_key(&public) != peer {
        return Err("RELAY_AUTH_TRANSPORT_MISMATCH");
    }
    let mut unsigned = challenge.clone();
    unsigned.relay_signature.clear();
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(&unsigned, &mut bytes)
        .map_err(|_| "RELAY_AUTH_CHALLENGE_INVALID")?;
    if !public.verify(&bytes, &challenge.relay_signature) {
        return Err("RELAY_AUTH_SIGNATURE_INVALID");
    }
    Ok(public)
}

fn validate_work_challenge(
    peer: PeerId,
    local_peer_id: &str,
    expected_target: &str,
    expected_relay: &str,
    expected_key_id: &str,
    relay_public_key: &[u8],
    challenge: &super::protocols::board_sync::WorkChallenge,
    now: i64,
) -> std::result::Result<(), &'static str> {
    if challenge.id.is_empty()
        || challenge.id.len() > 128
        || challenge.relay.len() > 255
        || challenge.requester.len() > 128
        || challenge.target.len() > 512
        || challenge.action.len() > 64
        || challenge.audience != "introduce"
        || challenge.key_id.len() > 128
        || challenge.relay != expected_relay
        || challenge.requester != local_peer_id
        || challenge.target != expected_target
        || challenge.action != "introduce"
        || challenge.key_id != expected_key_id
        || challenge.difficulty > POW_MAX_DIFFICULTY
        || challenge.expires_at < now
        || challenge.expires_at > now + 330
        || challenge.delivery_key.len() != 32
        || challenge.relay_signature.is_empty()
        || challenge.relay_signature.len() > 256
    {
        return Err("INTRODUCTION_WORK_INVALID");
    }
    let public = libp2p::identity::PublicKey::try_decode_protobuf(relay_public_key)
        .map_err(|_| "INTRODUCTION_WORK_KEY_INVALID")?;
    if PeerId::from_public_key(&public) != peer {
        return Err("INTRODUCTION_WORK_TRANSPORT_MISMATCH");
    }
    let mut unsigned = challenge.clone();
    unsigned.relay_signature.clear();
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(&unsigned, &mut bytes).map_err(|_| "INTRODUCTION_WORK_INVALID")?;
    if !public.verify(&bytes, &challenge.relay_signature) {
        return Err("INTRODUCTION_WORK_SIGNATURE_INVALID");
    }
    Ok(())
}

#[cfg(test)]
mod pow_budget_tests {
    use super::super::protocols::board_sync::{RelayAuthChallenge, WorkChallenge};
    use super::*;

    fn signed_auth_challenge(
        relay_key: &libp2p::identity::Keypair,
        local_peer_id: &str,
        now: i64,
    ) -> RelayAuthChallenge {
        let mut challenge = RelayAuthChallenge {
            domain: "harbor/relay-challenge/1".into(),
            version: 1,
            id: "auth-1".into(),
            relay: "relay.test".into(),
            peer_id: local_peer_id.into(),
            audience: "introduce".into(),
            issued_at: now,
            expires_at: now + 120,
            nonce: "0123456789abcdef".into(),
            key_id: "key-1".into(),
            relay_public_key: relay_key.public().encode_protobuf(),
            relay_signature: Vec::new(),
        };
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&challenge, &mut bytes).unwrap();
        challenge.relay_signature = relay_key.sign(&bytes).unwrap();
        challenge
    }

    fn signed_work_challenge(
        relay_key: &libp2p::identity::Keypair,
        local_peer_id: &str,
        now: i64,
        difficulty: u8,
    ) -> WorkChallenge {
        let mut challenge = WorkChallenge {
            id: "work-1".into(),
            relay: "relay.test".into(),
            requester: local_peer_id.into(),
            target: "@alice@relay.test".into(),
            action: "introduce".into(),
            audience: "introduce".into(),
            expires_at: now + 300,
            difficulty,
            key_id: "key-1".into(),
            relay_signature: Vec::new(),
            delivery_key: vec![7; 32],
        };
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&challenge, &mut bytes).unwrap();
        challenge.relay_signature = relay_key.sign(&bytes).unwrap();
        challenge
    }

    #[test]
    fn relay_auth_and_work_are_transport_bound_before_computation() {
        let relay_key = libp2p::identity::Keypair::generate_ed25519();
        let relay_peer = relay_key.public().to_peer_id();
        let local_peer = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id()
            .to_string();
        let now = 1_000;
        let auth = signed_auth_challenge(&relay_key, &local_peer, now);
        assert!(
            validate_relay_auth_challenge(relay_peer, &local_peer, "introduce", &auth, now,)
                .is_ok()
        );

        let work = signed_work_challenge(&relay_key, &local_peer, now, 14);
        assert!(validate_work_challenge(
            relay_peer,
            &local_peer,
            "@alice@relay.test",
            "relay.test",
            "key-1",
            &auth.relay_public_key,
            &work,
            now,
        )
        .is_ok());

        let attacker = libp2p::identity::Keypair::generate_ed25519();
        let forged = signed_work_challenge(&attacker, &local_peer, now, 14);
        assert_eq!(
            validate_work_challenge(
                relay_peer,
                &local_peer,
                "@alice@relay.test",
                "relay.test",
                "key-1",
                &auth.relay_public_key,
                &forged,
                now,
            ),
            Err("INTRODUCTION_WORK_SIGNATURE_INVALID")
        );
    }

    #[test]
    fn excessive_difficulty_and_tampered_bindings_reject_without_solving() {
        let relay_key = libp2p::identity::Keypair::generate_ed25519();
        let relay_peer = relay_key.public().to_peer_id();
        let local_peer = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id()
            .to_string();
        let now = 1_000;
        let auth = signed_auth_challenge(&relay_key, &local_peer, now);
        let excessive = signed_work_challenge(&relay_key, &local_peer, now, POW_MAX_DIFFICULTY + 1);
        assert_eq!(
            validate_work_challenge(
                relay_peer,
                &local_peer,
                "@alice@relay.test",
                "relay.test",
                "key-1",
                &auth.relay_public_key,
                &excessive,
                now,
            ),
            Err("INTRODUCTION_WORK_INVALID")
        );

        let mut swapped = signed_work_challenge(&relay_key, &local_peer, now, 14);
        swapped.target = "@mallory@relay.test".into();
        assert!(validate_work_challenge(
            relay_peer,
            &local_peer,
            "@alice@relay.test",
            "relay.test",
            "key-1",
            &auth.relay_public_key,
            &swapped,
            now,
        )
        .is_err());
    }

    #[test]
    fn solver_obeys_deadline_and_work_budget() {
        let relay_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer = relay_key.public().to_peer_id().to_string();
        let easy = signed_work_challenge(&relay_key, &local_peer, 1_000, 0);
        let cancel = AtomicBool::new(false);
        assert_eq!(
            solve_work_bounded(&easy, &cancel, Instant::now() + Duration::from_secs(1), 1),
            Ok(0)
        );

        let hard = signed_work_challenge(&relay_key, &local_peer, 1_000, 255);
        assert_eq!(
            solve_work_bounded(&hard, &cancel, Instant::now(), 1_024),
            Err(WorkSolveError::DeadlineExceeded)
        );
        assert_eq!(
            solve_work_bounded(&hard, &cancel, Instant::now() + Duration::from_secs(1), 1,),
            Err(WorkSolveError::WorkBudgetExceeded)
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn offloaded_solver_keeps_async_polling_responsive_and_cancels() {
        let relay_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer = relay_key.public().to_peer_id().to_string();
        let hard = signed_work_challenge(&relay_key, &local_peer, 1_000, 255);
        let cancelled = Arc::new(AtomicBool::new(false));
        let handle = spawn_work_solver(
            hard,
            cancelled.clone(),
            Instant::now() + Duration::from_secs(5),
        );

        tokio::time::timeout(Duration::from_millis(250), async {
            tokio::task::yield_now().await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        })
        .await
        .expect("async actor polling was blocked by proof-of-work");

        cancelled.store(true, Ordering::Relaxed);
        let result = tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("proof-of-work did not observe cancellation")
            .expect("blocking solver panicked");
        assert_eq!(result, Err(WorkSolveError::Cancelled));
    }
}
fn should_ack_ingest<T, E>(result: &std::result::Result<T, E>) -> bool {
    result.is_ok()
}

fn messaging_sender_matches_transport(peer: &PeerId, claimed_sender_peer_id: &str) -> bool {
    peer.to_string() == claimed_sender_peer_id
}

fn post_relay_request_matches(
    request: &WireBoardSyncRequest,
    post_id: &str,
    mutation_type: &str,
) -> bool {
    match (request, mutation_type) {
        (
            WireBoardSyncRequest::SubmitWallPost {
                post_id: wire_id, ..
            },
            "create" | "update",
        ) => wire_id == post_id,
        (
            WireBoardSyncRequest::DeleteWallPost {
                post_id: wire_id, ..
            },
            "delete",
        ) => wire_id == post_id,
        _ => false,
    }
}

fn post_relay_error_is_conflict(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    ["conflict", "stale", "lamport", "newer", "tombstone"]
        .iter()
        .any(|marker| normalized.contains(marker))
}

fn identity_request_signing_bytes(request: &IdentityExchangeRequest) -> Result<Vec<u8>> {
    let mut unsigned = request.clone();
    unsigned.signature.clear();
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(&unsigned, &mut bytes)
        .map_err(|error| AppError::Serialization(error.to_string()))?;
    Ok(bytes)
}

fn identity_response_signing_bytes(response: &IdentityExchangeResponse) -> Result<Vec<u8>> {
    let mut unsigned = response.clone();
    unsigned.signature.clear();
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(&unsigned, &mut bytes)
        .map_err(|error| AppError::Serialization(error.to_string()))?;
    Ok(bytes)
}

fn verify_identity_request(peer: PeerId, request: &IdentityExchangeRequest, now: i64) -> bool {
    if request.requester_peer_id != peer.to_string()
        || uuid::Uuid::parse_str(&request.request_id).is_err()
        || !matches!(
            request.action.as_str(),
            "request" | "accepted" | "declined" | "revoked" | "profile"
        )
        || request.public_key.len() != 32
        || request.x25519_public.len() != 32
        || request.display_name.is_empty()
        || request.display_name.chars().count() > 128
        || (request.action == "profile" && request.profile_revision == 0)
        || request.profile_revision > i64::MAX as u64
        || request.timestamp > now + 30
        || now - request.timestamp > 300
    {
        return false;
    }
    let Ok(raw) = <[u8; 32]>::try_from(request.public_key.as_slice()) else {
        return false;
    };
    let Ok(verifying_key) = ed25519_dalek::VerifyingKey::from_bytes(&raw) else {
        return false;
    };
    let Ok(derived) =
        crate::services::CryptoService::derive_peer_id_from_verifying_key(&verifying_key)
    else {
        return false;
    };
    if derived != request.requester_peer_id {
        return false;
    }
    let Ok(signature) = ed25519_dalek::Signature::from_slice(&request.signature) else {
        return false;
    };
    let Ok(bytes) = identity_request_signing_bytes(request) else {
        return false;
    };
    use ed25519_dalek::Verifier;
    verifying_key.verify(&bytes, &signature).is_ok()
}

fn verify_identity_response(peer: PeerId, response: &IdentityExchangeResponse, now: i64) -> bool {
    if response.peer_id != peer.to_string()
        || uuid::Uuid::parse_str(&response.request_id).is_err()
        || !matches!(
            response.status.as_str(),
            "review" | "accepted" | "declined" | "revoked" | "failed" | "profile_updated"
        )
        || response.public_key.len() != 32
        || response.x25519_public.len() != 32
        || response.display_name.is_empty()
        || response.display_name.chars().count() > 128
        || response.timestamp > now + 30
        || now - response.timestamp > 300
    {
        return false;
    }
    let Ok(raw) = <[u8; 32]>::try_from(response.public_key.as_slice()) else {
        return false;
    };
    let Ok(verifying_key) = ed25519_dalek::VerifyingKey::from_bytes(&raw) else {
        return false;
    };
    let Ok(derived) =
        crate::services::CryptoService::derive_peer_id_from_verifying_key(&verifying_key)
    else {
        return false;
    };
    if derived != response.peer_id {
        return false;
    }
    let Ok(signature) = ed25519_dalek::Signature::from_slice(&response.signature) else {
        return false;
    };
    let Ok(bytes) = identity_response_signing_bytes(response) else {
        return false;
    };
    use ed25519_dalek::Verifier;
    verifying_key.verify(&bytes, &signature).is_ok()
}

/// Handle to interact with the network service
#[derive(Clone)]
pub struct NetworkHandle {
    command_tx: mpsc::Sender<(NetworkCommand, Option<oneshot::Sender<NetworkResponse>>)>,
}

async fn await_name_registration(
    rx: oneshot::Receiver<NetworkResponse>,
    timeout: Duration,
) -> Result<(super::protocols::board_sync::NameClaim, Vec<u8>)> {
    match tokio::time::timeout(timeout, rx).await {
        Err(_) => Err(AppError::Network(
            "Name registration timed out. Check your relay connection and retry.".into(),
        )),
        Ok(Ok(NetworkResponse::RelayNameClaim {
            claim,
            relay_public_key,
        })) => Ok((*claim, relay_public_key)),
        Ok(Ok(NetworkResponse::Error(e))) => Err(AppError::Network(e)),
        Ok(_) => Err(AppError::Internal("Unexpected relay-name response".into())),
    }
}

impl NetworkHandle {
    #[cfg(test)]
    pub(crate) fn test_shutdown_runtime(
    ) -> (Self, tokio::task::JoinHandle<()>, oneshot::Receiver<()>) {
        let (command_tx, mut command_rx) = mpsc::channel(4);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let actor = tokio::spawn(async move {
            while let Some((command, _)) = command_rx.recv().await {
                if matches!(command, NetworkCommand::Shutdown) {
                    let _ = shutdown_tx.send(());
                    break;
                }
            }
        });
        (Self { command_tx }, actor, shutdown_rx)
    }

    pub async fn resolve_delivery_key(
        &self,
        relay_peer_id: PeerId,
        target: String,
    ) -> Result<(Vec<u8>, i64)> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::ResolveDeliveryKey {
                    relay_peer_id,
                    target: target.clone(),
                    response_tx: tx,
                },
                None,
            ))
            .await
            .map_err(|_| AppError::NetworkServiceUnavailable("Network unavailable".into()))?;
        match rx.await {
            Ok(NetworkResponse::DeliveryKey {
                target: t,
                key,
                expires_at,
            }) if t == target => Ok((key, expires_at)),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal(
                "Unexpected delivery-key response".into(),
            )),
        }
    }
    pub async fn active_relay(&self) -> Result<PeerId> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::GetActiveRelay, Some(tx)))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;
        match rx.await {
            Ok(NetworkResponse::ActiveRelay(p)) => Ok(p),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("No active relay".into())),
        }
    }
    pub async fn fetch_introductions(
        &self,
        relay_peer_id: PeerId,
        limit: u32,
    ) -> Result<Vec<super::protocols::board_sync::QueuedEnvelope>> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::FetchIntroductions {
                    relay_peer_id,
                    limit,
                    response_tx: tx,
                },
                None,
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;
        match rx.await {
            Ok(NetworkResponse::Introductions(v)) => Ok(v),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal(
                "Unexpected introduction fetch response".into(),
            )),
        }
    }
    pub async fn submit_introduction(
        &self,
        relay_peer_id: PeerId,
        target: String,
        request_id: String,
        ephemeral_public_key: Vec<u8>,
        ciphertext: Vec<u8>,
        expires_at: i64,
    ) -> Result<(String, u32)> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::SubmitIntroduction {
                    relay_peer_id,
                    target,
                    request_id,
                    ephemeral_public_key,
                    ciphertext,
                    expires_at,
                    response_tx: tx,
                },
                None,
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;
        match rx.await {
            Ok(NetworkResponse::IntroductionAccepted {
                request_id,
                retry_after,
            }) => Ok((request_id, retry_after)),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal(
                "Unexpected introduction response".into(),
            )),
        }
    }
    pub async fn register_relay_name(
        &self,
        relay_peer_id: PeerId,
        local_name: String,
        namespace: String,
    ) -> Result<(super::protocols::board_sync::NameClaim, Vec<u8>)> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::RegisterRelayName {
                    relay_peer_id,
                    local_name,
                    namespace,
                    response_tx: tx,
                },
                None,
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;
        await_name_registration(rx, Duration::from_secs(12)).await
    }
    /// Dial a peer at the given addresses
    pub async fn dial(&self, peer_id: PeerId, addresses: Vec<Multiaddr>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::Dial { peer_id, addresses }, Some(tx)))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Tear down a peer transport and cancel its pending message attempts.
    /// Relationship revocation uses this only after the durable local commit.
    pub async fn disconnect(&self, peer_id: PeerId) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::Disconnect { peer_id }, Some(tx)))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;
        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(error)) => Err(AppError::Network(error)),
            _ => Err(AppError::Internal("Unexpected disconnect response".into())),
        }
    }

    /// Get list of connected peers
    pub async fn get_connected_peers(&self) -> Result<Vec<PeerInfo>> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::GetConnectedPeers, Some(tx)))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Peers(peers)) => Ok(peers),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Get network statistics
    pub async fn get_stats(&self) -> Result<NetworkStats> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::GetStats, Some(tx)))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Stats(stats)) => Ok(stats),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Bootstrap the DHT
    pub async fn bootstrap(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::Bootstrap, Some(tx)))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Shutdown the network service
    pub async fn shutdown(&self) -> Result<()> {
        self.command_tx
            .send((NetworkCommand::Shutdown, None))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;
        Ok(())
    }

    /// Send a message to a peer
    pub async fn send_message(
        &self,
        peer_id: PeerId,
        protocol: String,
        event_id: String,
        message_id: String,
        payload: Vec<u8>,
    ) -> Result<MessageDeliveryReceipt> {
        self.send_message_attempt(peer_id, protocol, event_id, message_id, payload)
            .await
            .map_err(|failure| AppError::Network(failure.stable_message()))
    }

    /// Submit one exact wire event and return a typed terminal attempt outcome
    /// suitable for durable outbox transitions.
    pub async fn send_message_attempt(
        &self,
        peer_id: PeerId,
        protocol: String,
        event_id: String,
        message_id: String,
        payload: Vec<u8>,
    ) -> std::result::Result<MessageDeliveryReceipt, MessageDeliveryFailure> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::SendMessage {
                    peer_id,
                    protocol,
                    event_id: event_id.clone(),
                    message_id: message_id.clone(),
                    payload,
                    response_tx: tx,
                },
                None,
            ))
            .await
            .map_err(|_| message_runtime_unavailable(&event_id, &message_id))?;

        match rx.await {
            Ok(NetworkResponse::MessageDelivered(receipt)) => Ok(receipt),
            Ok(NetworkResponse::MessageDeliveryFailed(failure)) => Err(failure),
            Ok(NetworkResponse::Error(detail)) => Err(MessageDeliveryFailure {
                event_id,
                message_id,
                kind: MessageDeliveryFailureKind::InvalidResponse,
                detail,
            }),
            Ok(_) => Err(MessageDeliveryFailure {
                event_id,
                message_id,
                kind: MessageDeliveryFailureKind::InvalidResponse,
                detail: "network actor returned an unexpected response".into(),
            }),
            Err(_) => Err(message_runtime_unavailable(&event_id, &message_id)),
        }
    }

    /// Send a signed call signaling envelope to a peer and wait for the
    /// request-response acknowledgement.
    pub async fn send_signaling(&self, peer_id: PeerId, envelope: SignalingEnvelope) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::SendSignaling {
                    peer_id,
                    envelope,
                    response_tx: tx,
                },
                None,
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected signaling response".into())),
        }
    }

    /// Request identity from a peer
    pub async fn request_identity_action(
        &self,
        peer_id: PeerId,
        request_id: String,
        action: String,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::RequestIdentity {
                    peer_id,
                    request_id,
                    action,
                    permission_grants: None,
                    permission_revocations: None,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::IdentityQueued { .. }) | Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Queue an identity action carrying the exact capability grants that were
    /// committed atomically with local acceptance.
    pub async fn request_identity_action_with_grants(
        &self,
        peer_id: PeerId,
        request_id: String,
        action: String,
        permission_grants: Vec<crate::services::PermissionGrantMessage>,
    ) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::RequestIdentity {
                    peer_id,
                    request_id,
                    action,
                    permission_grants: Some(permission_grants),
                    permission_revocations: None,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;
        match rx.await {
            Ok(NetworkResponse::IdentityQueued { connected }) => Ok(connected),
            Ok(NetworkResponse::Error(error)) => Err(AppError::Network(error)),
            _ => Err(AppError::Internal("Unexpected identity response".into())),
        }
    }

    /// Get listening addresses (with peer ID appended)
    pub async fn get_listening_addresses(&self) -> Result<Vec<String>> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::GetListeningAddresses, Some(tx)))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Addresses(addrs)) => Ok(addrs),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Add a bootstrap node and dial it
    pub async fn add_bootstrap_node(&self, address: Multiaddr) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::AddBootstrapNode { address }, Some(tx)))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Add a custom relay server and attempt to get a relay reservation
    pub async fn add_relay_server(&self, address: Multiaddr) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::AddRelayServer { address }, Some(tx)))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Request content manifest from a peer
    pub async fn request_content_manifest(
        &self,
        peer_id: PeerId,
        cursor: std::collections::HashMap<String, u64>,
        limit: u32,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::RequestContentManifest {
                    peer_id,
                    cursor,
                    limit,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Join a community (register peer and list boards)
    pub async fn join_community(&self, relay_peer_id: PeerId, relay_address: String) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::JoinCommunity {
                    relay_peer_id,
                    relay_address,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// List boards on a relay
    pub async fn list_boards(&self, relay_peer_id: PeerId) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::ListBoards { relay_peer_id }, Some(tx)))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Get board posts from a relay
    pub async fn get_board_posts(
        &self,
        relay_peer_id: PeerId,
        board_id: String,
        after_timestamp: Option<i64>,
        limit: u32,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::GetBoardPosts {
                    relay_peer_id,
                    board_id,
                    after_timestamp,
                    limit,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Submit a board post to a relay
    pub async fn submit_board_post(
        &self,
        relay_peer_id: PeerId,
        board_id: String,
        content_text: String,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::SubmitBoardPost {
                    relay_peer_id,
                    board_id,
                    content_text,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Delete a board post on a relay
    pub async fn delete_board_post(&self, relay_peer_id: PeerId, post_id: String) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::DeleteBoardPost {
                    relay_peer_id,
                    post_id,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Submit a wall post to a relay for offline availability
    #[allow(clippy::too_many_arguments)]
    pub async fn submit_wall_post_to_relay(
        &self,
        relay_peer_id: PeerId,
        post_id: String,
        content_type: String,
        content_text: Option<String>,
        visibility: String,
        lamport_clock: i64,
        created_at: i64,
        signature: Vec<u8>,
        media_hashes: Vec<String>,
        media_items: Vec<super::protocols::board_sync::WallPostMediaItem>,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::SubmitWallPostToRelay {
                    relay_peer_id,
                    post_id,
                    content_type,
                    content_text,
                    visibility,
                    lamport_clock,
                    created_at,
                    signature,
                    media_hashes,
                    media_items,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Fetch media by hash from a specific peer
    pub async fn fetch_media(&self, peer_id: PeerId, media_hash: String) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::FetchMedia {
                    peer_id,
                    media_hash,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Get wall posts for a specific author from a relay
    pub async fn get_wall_posts_from_relay(
        &self,
        relay_peer_id: PeerId,
        author_peer_id: String,
        since_lamport_clock: i64,
        limit: u32,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::GetWallPostsFromRelay {
                    relay_peer_id,
                    author_peer_id,
                    since_lamport_clock,
                    limit,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Delete a wall post on a relay
    pub async fn delete_wall_post_on_relay(
        &self,
        relay_peer_id: PeerId,
        post_id: String,
        lamport_clock: u64,
        deleted_at: i64,
        signature: Vec<u8>,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::DeleteWallPostOnRelay {
                    relay_peer_id,
                    post_id,
                    lamport_clock,
                    deleted_at,
                    signature,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Submit signed wall social events to a relay.
    pub async fn submit_wall_social_events_to_relay(
        &self,
        relay_peer_id: PeerId,
        events: Vec<WallSocialEventItem>,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::SubmitWallSocialEventsToRelay {
                    relay_peer_id,
                    events,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;
        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Get signed wall social events from a relay.
    pub async fn get_wall_social_events_from_relay(
        &self,
        relay_peer_id: PeerId,
        author_peer_id: String,
        post_ids: Vec<String>,
        after_timestamp: i64,
        limit: u32,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::GetWallSocialEventsFromRelay {
                    relay_peer_id,
                    author_peer_id,
                    post_ids,
                    after_timestamp,
                    limit,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;
        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Connect to public relay servers for NAT traversal
    pub async fn connect_to_public_relays(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::ConnectToPublicRelays, Some(tx)))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Request content fetch from a peer
    pub async fn request_content_fetch(
        &self,
        peer_id: PeerId,
        post_id: String,
        include_media: bool,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::RequestContentFetch {
                    peer_id,
                    post_id,
                    include_media,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Trigger feed content sync from connected peers
    pub async fn sync_feed(&self, limit: u32) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::SyncFeed { limit }, Some(tx)))
            .await
            .map_err(|_| {
                AppError::NetworkServiceUnavailable("Network service unavailable".into())
            })?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }
}

use super::types::NatStatus;

/// The network service manages the libp2p swarm
pub struct NetworkService {
    swarm: Swarm<ChatBehaviour>,
    config: NetworkConfig,
    identity_service: Arc<IdentityService>,
    messaging_service: Option<Arc<MessagingService>>,
    calling_service: Option<Arc<CallingService>>,
    contacts_service: Option<Arc<ContactsService>>,
    permissions_service: Option<Arc<PermissionsService>>,
    posts_service: Option<Arc<PostsService>>,
    content_sync_service: Option<Arc<ContentSyncService>>,
    wall_social_service: Option<Arc<WallSocialService>>,
    board_service: Option<Arc<BoardService>>,
    mentions_service: Option<Arc<MentionsService>>,
    media_service: Option<Arc<MediaStorageService>>,
    command_rx: mpsc::Receiver<(NetworkCommand, Option<oneshot::Sender<NetworkResponse>>)>,
    event_tx: mpsc::Sender<NetworkEvent>,
    connected_peers: HashMap<PeerId, PeerInfo>,
    discovered_peers: HashMap<PeerId, Vec<Multiaddr>>,
    listening_addresses: Vec<Multiaddr>,
    stats: NetworkStats,
    start_time: Instant,
    /// Current NAT status
    nat_status: NatStatus,
    /// Relay addresses we're reachable at
    relay_addresses: Vec<Multiaddr>,
    /// External addresses discovered via AutoNAT
    external_addresses: Vec<Multiaddr>,
    /// Whether we've attempted to connect to relays
    relay_connection_attempted: bool,
    /// Relay peers we've dialed but haven't yet requested a reservation for.
    /// Key: relay peer ID, Value: full relay multiaddr (transport + /p2p/<id>).
    /// Reservation is requested in Identify::Received after the connection is fully negotiated.
    pending_relay_reservations: HashMap<PeerId, Multiaddr>,
    /// Relay peers that we're probing for community support.
    /// Key: relay peer ID, Value: the original relay multiaddr string (e.g. "/ip4/.../p2p/...").
    /// After a relay reservation is accepted, we send a ListBoards probe; if we get
    /// a BoardList response back, the relay is a community relay and we auto-join.
    pending_community_probes: HashMap<PeerId, String>,
    /// Relay peers that have been confirmed as community relays.
    community_relays: HashMap<PeerId, String>,
    /// Relay peers where we've sent RegisterPeer and are waiting for PeerRegistered
    /// before sending ListBoards. This prevents the race condition where ListBoards
    /// arrives at the relay before RegisterPeer has been processed (which would fail
    /// signature verification since the peer's public key hasn't been stored yet).
    pending_board_registrations: std::collections::HashSet<PeerId>,
    /// Pending signaling requests waiting for a request-response outcome.
    pending_signaling_requests:
        HashMap<request_response::OutboundRequestId, oneshot::Sender<NetworkResponse>>,
    /// Direct-message requests are keyed by libp2p's unique outbound request
    /// ID, never by peer. This keeps simultaneous requests to one peer from
    /// resolving the wrong caller when responses arrive out of order.
    pending_messaging_requests: PendingMessagingRequests<request_response::OutboundRequestId>,
    pending_identity_requests: HashMap<request_response::OutboundRequestId, (String, String)>,
    pending_name_registration: HashMap<PeerId, PendingNameRegistration>,
    pending_introduction_submit: HashMap<PeerId, PendingIntroductionSubmit>,
    pending_introduction_fetch: HashMap<PeerId, PendingIntroductionFetch>,
    pending_delivery_resolution: HashMap<PeerId, PendingDeliveryResolution>,
    pow_result_tx: mpsc::Sender<PowCompletion>,
    pow_result_rx: mpsc::Receiver<PowCompletion>,
    pending_media_fetches: HashMap<request_response::OutboundRequestId, PendingMediaFetch>,
    pending_media_fetch_by_hash: HashMap<String, request_response::OutboundRequestId>,
    /// Durable local post mutations correlated to their exact board protocol request.
    pending_post_relay_requests:
        HashMap<request_response::OutboundRequestId, PendingPostRelayRequest>,
}

struct PendingPostRelayRequest {
    event_id: String,
    post_id: String,
    mutation_type: String,
    relay_peer_id: PeerId,
    deadline: Instant,
    attempt_count: i64,
    max_attempts: i64,
}

struct PendingMediaFetch {
    peer_id: PeerId,
    media_hash: String,
    profile_id: String,
    offset: u64,
}

struct PendingMessagingRequest {
    event_id: String,
    message_id: String,
    peer_id: PeerId,
    deadline: Instant,
    /// Foreground submissions have a waiter. Restart/background outbox
    /// attempts do not, but still retain full correlation and terminal state.
    response_tx: Option<oneshot::Sender<NetworkResponse>>,
}

/// Small request-correlation state machine kept independent of the swarm so
/// its cleanup and terminal-state invariants can be tested deterministically.
struct PendingMessagingRequests<K> {
    requests: HashMap<K, PendingMessagingRequest>,
}

impl<K> Default for PendingMessagingRequests<K> {
    fn default() -> Self {
        Self {
            requests: HashMap::new(),
        }
    }
}

impl<K> PendingMessagingRequests<K>
where
    K: Eq + Hash + Clone,
{
    fn insert(
        &mut self,
        request_id: K,
        event_id: String,
        message_id: String,
        peer_id: PeerId,
        deadline: Instant,
        response_tx: Option<oneshot::Sender<NetworkResponse>>,
    ) {
        if let Some(replaced) = self.requests.insert(
            request_id,
            PendingMessagingRequest {
                event_id,
                message_id,
                peer_id,
                deadline,
                response_tx,
            },
        ) {
            if let Some(response_tx) = replaced.response_tx {
                let _ = response_tx.send(NetworkResponse::MessageDeliveryFailed(
                    MessageDeliveryFailure {
                        event_id: replaced.event_id,
                        message_id: replaced.message_id,
                        kind: MessageDeliveryFailureKind::Cancelled,
                        detail: "duplicate outbound request ID replaced the pending attempt".into(),
                    },
                ));
            }
        }
    }

    fn fail(
        &mut self,
        request_id: &K,
        kind: MessageDeliveryFailureKind,
        detail: impl Into<String>,
    ) -> Option<MessageDeliveryFailure> {
        let Some(pending) = self.requests.remove(request_id) else {
            return None;
        };
        let failure = MessageDeliveryFailure {
            event_id: pending.event_id,
            message_id: pending.message_id,
            kind,
            detail: detail.into(),
        };
        if let Some(response_tx) = pending.response_tx {
            let _ = response_tx.send(NetworkResponse::MessageDeliveryFailed(failure.clone()));
        }
        Some(failure)
    }

    fn complete_remote(
        &mut self,
        request_id: &K,
        peer_id: &PeerId,
        response: MessagingResponse,
    ) -> Option<MessageDeliveryAttemptOutcome> {
        let Some(pending) = self.requests.remove(request_id) else {
            return None;
        };
        let outcome = if pending.peer_id != *peer_id {
            NetworkResponse::MessageDeliveryFailed(MessageDeliveryFailure {
                event_id: pending.event_id,
                message_id: pending.message_id,
                kind: MessageDeliveryFailureKind::InvalidResponse,
                detail: "response arrived from an unexpected peer".into(),
            })
        } else if response.success {
            if response.message_id.as_deref() == Some(pending.message_id.as_str()) {
                NetworkResponse::MessageDelivered(MessageDeliveryReceipt {
                    event_id: pending.event_id,
                    message_id: pending.message_id,
                })
            } else {
                NetworkResponse::MessageDeliveryFailed(MessageDeliveryFailure {
                    event_id: pending.event_id,
                    message_id: pending.message_id.clone(),
                    kind: MessageDeliveryFailureKind::InvalidResponse,
                    detail: format!(
                        "acknowledgement message ID mismatch: expected {}, received {:?}",
                        pending.message_id, response.message_id
                    ),
                })
            }
        } else {
            NetworkResponse::MessageDeliveryFailed(MessageDeliveryFailure {
                event_id: pending.event_id,
                message_id: pending.message_id,
                kind: MessageDeliveryFailureKind::Rejected,
                detail: response
                    .error
                    .unwrap_or_else(|| "remote peer rejected the message".into()),
            })
        };
        let terminal = match &outcome {
            NetworkResponse::MessageDelivered(receipt) => {
                MessageDeliveryAttemptOutcome::Delivered(receipt.clone())
            }
            NetworkResponse::MessageDeliveryFailed(failure) => {
                MessageDeliveryAttemptOutcome::Failed(failure.clone())
            }
            _ => unreachable!("messaging completion must be a typed terminal outcome"),
        };
        if let Some(response_tx) = pending.response_tx {
            let _ = response_tx.send(outcome);
        }
        Some(terminal)
    }

    fn fail_peer(
        &mut self,
        peer_id: &PeerId,
        kind: MessageDeliveryFailureKind,
        detail: &str,
    ) -> Vec<MessageDeliveryFailure> {
        let request_ids: Vec<K> = self
            .requests
            .iter()
            .filter_map(|(request_id, pending)| {
                (pending.peer_id == *peer_id).then(|| request_id.clone())
            })
            .collect();
        request_ids
            .into_iter()
            .filter_map(|request_id| self.fail(&request_id, kind, detail))
            .collect()
    }

    fn maintain(&mut self, now: Instant) -> (usize, Vec<MessageDeliveryFailure>) {
        let cancelled_ids: Vec<K> = self
            .requests
            .iter()
            .filter_map(|(request_id, pending)| {
                pending
                    .response_tx
                    .as_ref()
                    .is_some_and(oneshot::Sender::is_closed)
                    .then(|| request_id.clone())
            })
            .collect();
        let cancelled = cancelled_ids.len();
        for request_id in cancelled_ids {
            self.requests.remove(&request_id);
        }

        let expired_ids: Vec<K> = self
            .requests
            .iter()
            .filter_map(|(request_id, pending)| {
                (pending.deadline <= now).then(|| request_id.clone())
            })
            .collect();
        let expired = expired_ids
            .into_iter()
            .filter_map(|request_id| {
                self.fail(
                    &request_id,
                    MessageDeliveryFailureKind::Timeout,
                    "peer did not acknowledge the message before its deadline",
                )
            })
            .collect();
        (cancelled, expired)
    }

    fn fail_all(&mut self, error: &str) -> Vec<MessageDeliveryFailure> {
        let request_ids: Vec<K> = self.requests.keys().cloned().collect();
        request_ids
            .into_iter()
            .filter_map(|request_id| {
                self.fail(&request_id, MessageDeliveryFailureKind::Shutdown, error)
            })
            .collect()
    }

    fn len(&self) -> usize {
        self.requests.len()
    }
}

struct PendingNameRegistration {
    local_name: String,
    namespace: String,
    session_token: Option<String>,
    relay_public_key: Vec<u8>,
    response_tx: oneshot::Sender<NetworkResponse>,
}
struct PendingIntroductionSubmit {
    target: String,
    request_id: String,
    ephemeral_public_key: Vec<u8>,
    ciphertext: Vec<u8>,
    expires_at: i64,
    session_token: Option<String>,
    relay_name: Option<String>,
    relay_key_id: Option<String>,
    relay_public_key: Vec<u8>,
    pow_challenge_id: Option<String>,
    pow_cancel: Arc<AtomicBool>,
    response_tx: oneshot::Sender<NetworkResponse>,
}
struct PendingIntroductionFetch {
    limit: u32,
    session_token: Option<String>,
    response_tx: oneshot::Sender<NetworkResponse>,
}

struct PendingDeliveryResolution {
    target: String,
    relay_name: Option<String>,
    relay_key_id: Option<String>,
    relay_public_key: Vec<u8>,
    response_tx: oneshot::Sender<NetworkResponse>,
}

struct PowCompletion {
    peer: PeerId,
    challenge: super::protocols::board_sync::WorkChallenge,
    result: std::result::Result<u64, WorkSolveError>,
}

impl NetworkService {
    /// Create a new network service
    pub fn new(
        config: NetworkConfig,
        identity_service: Arc<IdentityService>,
        keypair: libp2p::identity::Keypair,
    ) -> Result<(Self, NetworkHandle, mpsc::Receiver<NetworkEvent>)> {
        let swarm = build_swarm(keypair, &config)?;

        let (command_tx, command_rx) = mpsc::channel(256);
        let (event_tx, event_rx) = mpsc::channel(256);
        let (pow_result_tx, pow_result_rx) = mpsc::channel(8);

        let handle = NetworkHandle { command_tx };

        let service = Self {
            swarm,
            config,
            identity_service,
            messaging_service: None,
            calling_service: None,
            contacts_service: None,
            permissions_service: None,
            posts_service: None,
            content_sync_service: None,
            wall_social_service: None,
            board_service: None,
            mentions_service: None,
            media_service: None,
            command_rx,
            event_tx,
            connected_peers: HashMap::new(),
            discovered_peers: HashMap::new(),
            listening_addresses: Vec::new(),
            stats: NetworkStats::default(),
            start_time: Instant::now(),
            nat_status: NatStatus::Unknown,
            relay_addresses: Vec::new(),
            external_addresses: Vec::new(),
            relay_connection_attempted: false,
            pending_relay_reservations: HashMap::new(),
            pending_community_probes: HashMap::new(),
            community_relays: HashMap::new(),
            pending_board_registrations: std::collections::HashSet::new(),
            pending_signaling_requests: HashMap::new(),
            pending_messaging_requests: PendingMessagingRequests::default(),
            pending_identity_requests: HashMap::new(),
            pending_name_registration: HashMap::new(),
            pending_introduction_submit: HashMap::new(),
            pending_introduction_fetch: HashMap::new(),
            pending_delivery_resolution: HashMap::new(),
            pow_result_tx,
            pow_result_rx,
            pending_media_fetches: HashMap::new(),
            pending_media_fetch_by_hash: HashMap::new(),
            pending_post_relay_requests: HashMap::new(),
        };

        Ok((service, handle, event_rx))
    }

    /// Set the messaging service for processing incoming messages
    pub fn set_messaging_service(&mut self, service: Arc<MessagingService>) {
        self.messaging_service = Some(service);
    }

    /// Set the calling service for validating incoming signaling.
    pub fn set_calling_service(&mut self, service: Arc<CallingService>) {
        self.calling_service = Some(service);
    }
    pub fn set_mentions_service(&mut self, service: Arc<MentionsService>) {
        self.mentions_service = Some(service)
    }

    /// Set the contacts service for storing contacts from identity exchange
    pub fn set_contacts_service(&mut self, service: Arc<ContactsService>) {
        self.contacts_service = Some(service);
    }

    /// Set the permissions service for granting permissions to contacts
    pub fn set_permissions_service(&mut self, service: Arc<PermissionsService>) {
        self.permissions_service = Some(service);
    }

    /// Set the posts service for serving post fetch requests
    pub fn set_posts_service(&mut self, service: Arc<PostsService>) {
        self.posts_service = Some(service);
    }

    /// Set content sync service for handling manifest processing + storage
    pub fn set_content_sync_service(&mut self, service: Arc<ContentSyncService>) {
        self.content_sync_service = Some(service);
    }

    /// Set wall social service for applying signed comments and reactions.
    pub fn set_wall_social_service(&mut self, service: Arc<WallSocialService>) {
        self.wall_social_service = Some(service);
    }

    /// Set board service for community board operations
    pub fn set_board_service(&mut self, service: Arc<BoardService>) {
        self.board_service = Some(service);
    }

    /// Set media storage service for P2P media transfer
    pub fn set_media_service(&mut self, service: Arc<MediaStorageService>) {
        self.media_service = Some(service);
    }

    /// Get the local peer ID
    pub fn local_peer_id(&self) -> &PeerId {
        self.swarm.local_peer_id()
    }

    /// Create an identity exchange request
    fn create_identity_request(
        &self,
        request_id: String,
        action: String,
        subject_peer_id: &str,
        committed_grants: Option<Vec<crate::services::PermissionGrantMessage>>,
        committed_revocations: Option<Vec<crate::services::PermissionRevokeMessage>>,
    ) -> Result<IdentityExchangeRequest> {
        let info = self
            .identity_service
            .get_identity_info()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        let timestamp = chrono::Utc::now().timestamp();
        let (profile_revision, avatar_mime_type) = self.identity_service.profile_revision()?;
        let engine = base64::engine::general_purpose::STANDARD;
        let permission_grants = if action == "accepted" {
            match committed_grants {
                Some(grants) => grants,
                None => self.create_contact_acceptance_grants(subject_peer_id)?,
            }
        } else {
            Vec::new()
        };
        let permission_revocations = if action == "revoked" {
            committed_revocations.ok_or_else(|| {
                AppError::Validation("Revocation delivery requires committed envelopes".into())
            })?
        } else {
            Vec::new()
        };
        let mut request = IdentityExchangeRequest {
            request_id,
            action,
            requester_peer_id: info.peer_id,
            public_key: engine
                .decode(info.public_key)
                .map_err(|error| AppError::Crypto(error.to_string()))?,
            x25519_public: engine
                .decode(info.x25519_public)
                .map_err(|error| AppError::Crypto(error.to_string()))?,
            display_name: info.display_name,
            avatar_hash: info.avatar_hash,
            avatar_mime_type,
            profile_revision,
            bio: info.bio,
            timestamp,
            permission_grants,
            permission_revocations,
            signature: Vec::new(),
        };
        let bytes = identity_request_signing_bytes(&request)?;
        request.signature = self.identity_service.sign_raw(&bytes)?;
        Ok(request)
    }

    fn create_identity_response(
        &self,
        request_id: String,
        status: String,
        subject_peer_id: &str,
        committed_grants: Option<Vec<crate::services::PermissionGrantMessage>>,
    ) -> Result<IdentityExchangeResponse> {
        let info = self
            .identity_service
            .get_identity_info()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;
        let timestamp = chrono::Utc::now().timestamp();
        let (profile_revision, avatar_mime_type) = self.identity_service.profile_revision()?;
        let engine = base64::engine::general_purpose::STANDARD;
        let permission_grants = if status == "accepted" {
            match committed_grants {
                Some(grants) => grants,
                None => self.create_contact_acceptance_grants(subject_peer_id)?,
            }
        } else {
            Vec::new()
        };
        let mut response = IdentityExchangeResponse {
            request_id,
            status,
            peer_id: info.peer_id,
            public_key: engine
                .decode(info.public_key)
                .map_err(|error| AppError::Crypto(error.to_string()))?,
            x25519_public: engine
                .decode(info.x25519_public)
                .map_err(|error| AppError::Crypto(error.to_string()))?,
            display_name: info.display_name,
            avatar_hash: info.avatar_hash,
            avatar_mime_type,
            profile_revision,
            bio: info.bio,
            timestamp,
            permission_grants,
            permission_revocations: Vec::new(),
            signature: Vec::new(),
        };
        response.signature = self
            .identity_service
            .sign_raw(&identity_response_signing_bytes(&response)?)?;
        Ok(response)
    }

    fn create_contact_acceptance_grants(
        &self,
        subject_peer_id: &str,
    ) -> Result<Vec<crate::services::PermissionGrantMessage>> {
        let Some(permissions) = self.permissions_service.as_ref() else {
            return Err(AppError::Internal(
                "Permissions service unavailable during contact acceptance".into(),
            ));
        };
        [Capability::Chat, Capability::WallRead, Capability::Call]
            .into_iter()
            .map(|capability| {
                permissions.create_permission_grant(subject_peer_id, capability, None)
            })
            .collect()
    }

    fn validate_contact_acceptance_grants(
        &self,
        expected_issuer: &str,
        issuer_public_key: &[u8],
        grants: &[crate::services::PermissionGrantMessage],
    ) -> Result<()> {
        let Some(permissions) = self.permissions_service.as_ref() else {
            return Err(AppError::Internal(
                "Permissions service unavailable during contact acceptance".into(),
            ));
        };
        for grant in grants {
            if grant.issuer_peer_id != expected_issuer {
                return Err(AppError::Unauthorized(
                    "Contact acceptance contained a grant from another issuer".into(),
                ));
            }
            permissions.validate_incoming_grant(grant, issuer_public_key)?;
        }
        for capability in [Capability::Chat, Capability::WallRead, Capability::Call] {
            if !grants
                .iter()
                .any(|grant| grant.capability == capability.as_str())
            {
                return Err(AppError::PermissionDenied(format!(
                    "Contact acceptance is missing signed {} capability",
                    capability.as_str()
                )));
            }
        }
        Ok(())
    }

    /// Existing beta contacts may predate capability exchange. Replaying a
    /// signed acceptance on their next connection repairs both sides without
    /// weakening the requirement for an author-issued WallRead signature.
    fn reconcile_contact_acceptance_grants(&mut self, peer_id: PeerId) {
        let peer = peer_id.to_string();
        let is_contact = self
            .contacts_service
            .as_ref()
            .and_then(|service| service.get_contact(&peer).ok().flatten())
            .is_some_and(|contact| !contact.is_blocked);
        if !is_contact {
            return;
        }
        let needs_reconciliation = self
            .permissions_service
            .as_ref()
            .map(|permissions| {
                !permissions
                    .peer_has_capability(&peer, Capability::WallRead)
                    .unwrap_or(false)
                    || !permissions
                        .we_have_capability(&peer, Capability::WallRead)
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        if !needs_reconciliation {
            return;
        }

        // Use a transient correlation ID so a failed repair attempt cannot
        // regress the durable accepted request row back to `failed`.
        let request_id = uuid::Uuid::new_v4().to_string();

        match self.create_identity_request(request_id.clone(), "accepted".into(), &peer, None, None)
        {
            Ok(request) => {
                let outbound_id = self
                    .swarm
                    .behaviour_mut()
                    .identity_exchange
                    .send_request(&peer_id, request);
                self.pending_identity_requests
                    .insert(outbound_id, (request_id, "accepted".into()));
            }
            Err(error) => warn!("Could not reconcile contact grants with {peer}: {error}"),
        }
    }

    /// Publish the current signed monotonic profile revision whenever an
    /// authorized contact reconnects, covering offline updates and restarts.
    fn publish_profile_to_contact(&mut self, peer_id: PeerId) {
        let peer = peer_id.to_string();
        let authorized = self
            .contacts_service
            .as_ref()
            .and_then(|service| service.get_contact(&peer).ok().flatten())
            .is_some_and(|contact| !contact.is_blocked);
        if !authorized {
            return;
        }
        let request_id = uuid::Uuid::new_v4().to_string();
        match self.create_identity_request(request_id.clone(), "profile".into(), &peer, None, None)
        {
            Ok(request) => {
                let outbound_id = self
                    .swarm
                    .behaviour_mut()
                    .identity_exchange
                    .send_request(&peer_id, request);
                self.pending_identity_requests
                    .insert(outbound_id, (request_id, "profile".into()));
            }
            Err(error) => warn!("Could not publish profile revision to {peer}: {error}"),
        }
    }

    fn stage_signed_contact_profile(
        &mut self,
        peer: PeerId,
        peer_id: &str,
        revision: u64,
        display_name: &str,
        avatar_hash: Option<&str>,
        avatar_mime_type: Option<&str>,
        bio: Option<&str>,
    ) -> Result<()> {
        let contacts = self
            .contacts_service
            .clone()
            .ok_or_else(|| AppError::Internal("Contacts service unavailable".into()))?;
        if !contacts.stage_profile_update(
            peer_id,
            revision,
            display_name,
            avatar_hash,
            avatar_mime_type,
            bio,
        )? {
            return Ok(());
        }
        let Some(hash) = avatar_hash else {
            return Ok(());
        };
        let media = self
            .media_service
            .clone()
            .ok_or_else(|| AppError::Internal("Media service unavailable".into()))?;
        if media.has_media(hash) {
            let old = contacts
                .get_contact(peer_id)?
                .and_then(|contact| contact.avatar_hash);
            if contacts.promote_verified_profile_avatar(peer_id, hash)? {
                if let Some(old) = old.filter(|old| old != hash) {
                    let _ = media.delete_media_if_orphaned(&old);
                }
            }
            return Ok(());
        }
        media.ensure_transfer(
            hash,
            Some(peer_id),
            "image",
            avatar_mime_type,
            Some("profile-avatar"),
            None,
        )?;
        let profile_id = self.identity_service.get_peer_id()?;
        self.queue_media_range(peer, hash.to_string(), profile_id, 0, true)
            .map_err(AppError::Network)
    }

    /// Start listening on configured addresses and wait until both transports
    /// report that their sockets are live. `Swarm::listen_on` only queues a
    /// listener, so returning immediately would expose a handle for a swarm
    /// whose bind can still fail on the next event-loop tick.
    pub(crate) async fn start_listening(&mut self) -> Result<()> {
        // Listen on TCP
        let tcp_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", self.config.tcp_port)
            .parse()
            .map_err(|e| AppError::Network(format!("Invalid TCP address: {}", e)))?;
        let tcp_listener = self.swarm.listen_on(tcp_addr.clone())?;
        info!("Starting TCP listener: {}", tcp_addr);

        // Listen on QUIC
        let quic_addr: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", self.config.quic_port)
            .parse()
            .map_err(|e| AppError::Network(format!("Invalid QUIC address: {}", e)))?;
        let quic_listener = self.swarm.listen_on(quic_addr.clone())?;
        info!("Starting QUIC listener: {}", quic_addr);

        let readiness = async {
            let mut tcp_ready = false;
            let mut quic_ready = false;

            while !tcp_ready || !quic_ready {
                let event = self.swarm.select_next_some().await;
                match &event {
                    SwarmEvent::NewListenAddr { listener_id, .. } => {
                        tcp_ready |= listener_id == &tcp_listener;
                        quic_ready |= listener_id == &quic_listener;
                    }
                    SwarmEvent::ListenerError { listener_id, error }
                        if listener_id == &tcp_listener || listener_id == &quic_listener =>
                    {
                        return Err(AppError::Network(format!(
                            "Listener failed before startup completed: {error}"
                        )));
                    }
                    _ => {}
                }

                self.handle_swarm_event(event).await;
            }

            Ok(())
        };

        tokio::time::timeout(Duration::from_secs(10), readiness)
            .await
            .map_err(|_| AppError::Network("Timed out waiting for network listeners".into()))??;

        Ok(())
    }

    fn connect_configured_bootstraps(&mut self) {
        for address in self.config.bootstrap_nodes.clone() {
            let Some(peer_id) = address.iter().find_map(|protocol| match protocol {
                libp2p::multiaddr::Protocol::P2p(peer_id) => Some(peer_id),
                _ => None,
            }) else {
                warn!("Ignoring bootstrap address without peer ID: {address}");
                continue;
            };

            let address_without_peer: Multiaddr = address
                .iter()
                .filter(|protocol| !matches!(protocol, libp2p::multiaddr::Protocol::P2p(_)))
                .collect();
            self.swarm
                .behaviour_mut()
                .kademlia
                .add_address(&peer_id, address_without_peer);

            match self.swarm.dial(address.clone()) {
                Ok(()) => info!("Dialing configured bootstrap node: {address}"),
                Err(error) => warn!("Could not dial configured bootstrap node {address}: {error}"),
            }
        }
    }

    /// Run the network event loop
    pub async fn run(mut self) {
        info!("Network service starting...");

        if let Err(e) = self.start_listening().await {
            error!("Failed to start listening: {}", e);
            return;
        }

        self.run_after_listening().await;
    }

    /// Run a service whose listeners were bound synchronously by its runtime
    /// owner before the handle was published.
    pub async fn run_after_listening(mut self) {
        self.connect_configured_bootstraps();

        // Auto-connect to relay on start (don't wait for AutoNAT)
        info!("Auto-connecting to Harbor relay...");
        self.connect_to_relays().await;

        let mut pending_request_maintenance =
            tokio::time::interval(PENDING_REQUEST_MAINTENANCE_INTERVAL);
        pending_request_maintenance.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Handle swarm events
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await;
                }

                // Handle commands from the application
                command = self.command_rx.recv() => {
                    let Some((command, response_tx)) = command else {
                        let failures = self.pending_messaging_requests.fail_all(
                            "network command channel closed",
                        );
                        self.apply_message_attempt_failures(failures);
                        self.fail_all_post_relay_requests(
                            "Network service stopped before relay acknowledgement",
                        );
                        info!("Network service stopping because all handles were dropped");
                        break;
                    };
                    let should_shutdown = matches!(command, NetworkCommand::Shutdown);
                    let response = self.handle_command(command).await;
                    if let Some(tx) = response_tx {
                        let _ = tx.send(response);
                    }
                    if should_shutdown {
                        let failures = self.pending_messaging_requests.fail_all(
                            "MESSAGE_RUNTIME_SHUTDOWN: network service is shutting down",
                        );
                        self.apply_message_attempt_failures(failures);
                        self.fail_all_post_relay_requests(
                            "Network service shut down before relay acknowledgement",
                        );
                        info!("Network service shutting down...");
                        break;
                    }
                }

                completion = self.pow_result_rx.recv() => {
                    if let Some(completion) = completion {
                        self.handle_pow_completion(completion);
                    }
                }

                _ = pending_request_maintenance.tick() => {
                    let (cancelled, expired) =
                        self.pending_messaging_requests.maintain(Instant::now());
                    if cancelled > 0 || !expired.is_empty() {
                        debug!(
                            "Cleaned pending message attempts: {} cancelled, {} expired",
                            cancelled, expired.len()
                        );
                    }
                    self.apply_message_attempt_failures(expired);
                    self.dispatch_due_message_outbox();
                    self.expire_post_relay_requests();
                    self.dispatch_due_post_relay_outbox();
                    self.dispatch_due_contact_revocations();
                    self.maintain_pending_introduction_work();
                }
            }
        }
    }

    fn maintain_pending_introduction_work(&mut self) {
        let now = Utc::now().timestamp();
        self.pending_introduction_submit.retain(|_, pending| {
            let keep = !pending.response_tx.is_closed() && pending.expires_at >= now;
            if !keep {
                pending.pow_cancel.store(true, Ordering::Relaxed);
            }
            keep
        });
    }

    fn handle_pow_completion(&mut self, completion: PowCompletion) {
        let Some(pending) = self.pending_introduction_submit.get(&completion.peer) else {
            return;
        };
        if pending.pow_challenge_id.as_deref() != Some(completion.challenge.id.as_str())
            || pending.pow_cancel.load(Ordering::Relaxed)
        {
            return;
        }
        let nonce = match completion.result {
            Ok(nonce) if completion.challenge.expires_at >= Utc::now().timestamp() => nonce,
            Ok(_) | Err(WorkSolveError::DeadlineExceeded) => {
                let pending = self
                    .pending_introduction_submit
                    .remove(&completion.peer)
                    .expect("pending introduction checked above");
                pending.pow_cancel.store(true, Ordering::Relaxed);
                let _ = pending
                    .response_tx
                    .send(NetworkResponse::Error("INTRODUCTION_WORK_TIMEOUT".into()));
                return;
            }
            Err(WorkSolveError::Cancelled) => return,
            Err(WorkSolveError::WorkBudgetExceeded) => {
                let pending = self
                    .pending_introduction_submit
                    .remove(&completion.peer)
                    .expect("pending introduction checked above");
                pending.pow_cancel.store(true, Ordering::Relaxed);
                let _ = pending.response_tx.send(NetworkResponse::Error(
                    "INTRODUCTION_WORK_BUDGET_EXCEEDED".into(),
                ));
                return;
            }
        };
        let Some(pending) = self.pending_introduction_submit.get(&completion.peer) else {
            return;
        };
        let envelope = super::protocols::board_sync::IntroductionEnvelope {
            version: 1,
            request_id: pending.request_id.clone(),
            target: pending.target.clone(),
            requester_peer_id: self.identity_service.get_peer_id().unwrap_or_default(),
            requester_ephemeral_x25519_key: pending.ephemeral_public_key.clone(),
            message_ciphertext: pending.ciphertext.clone(),
            issued_at: Utc::now().timestamp(),
            expires_at: pending.expires_at,
            work_challenge: completion.challenge,
            work_nonce: nonce,
        };
        self.swarm.behaviour_mut().board_sync.send_request(
            &completion.peer,
            WireBoardSyncRequest::SubmitIntroduction {
                session_token: pending.session_token.clone().unwrap_or_default(),
                envelope,
            },
        );
    }

    fn dispatch_due_contact_revocations(&mut self) {
        let Some(contacts) = self.contacts_service.clone() else {
            return;
        };
        let now = Utc::now().timestamp();
        let entries = match contacts.claim_due_contact_revocations(now, 30, 8) {
            Ok(entries) => entries,
            Err(error) => {
                warn!("Could not claim contact-revocation outbox: {error}");
                return;
            }
        };
        for entry in entries {
            let Ok(peer_id) = entry.peer_id.parse::<PeerId>() else {
                let _ =
                    contacts.retry_contact_revocation(&entry.request_id, "invalid peer ID", now);
                continue;
            };
            let request = self.create_identity_request(
                entry.request_id.clone(),
                "revoked".into(),
                &entry.peer_id,
                None,
                Some(entry.revocations),
            );
            match request {
                Ok(request) => {
                    let outbound_id = self
                        .swarm
                        .behaviour_mut()
                        .identity_exchange
                        .send_request(&peer_id, request);
                    self.pending_identity_requests
                        .insert(outbound_id, (entry.request_id, "revoked".into()));
                }
                Err(error) => {
                    let _ = contacts.retry_contact_revocation(
                        &entry.request_id,
                        &error.to_string(),
                        now,
                    );
                }
            }
        }
    }

    /// Integration point for the durable outbox driver. Keeping dispatch on
    /// the actor maintenance tick serializes transport submission with pending
    /// request registration without requiring another task to mutate the swarm.
    /// The messaging service can claim due rows here during restart recovery
    /// and on every subsequent tick.
    fn dispatch_due_message_outbox(&mut self) {
        let available =
            MESSAGE_OUTBOX_MAX_IN_FLIGHT.saturating_sub(self.pending_messaging_requests.len());
        if available == 0 {
            return;
        }
        let Some(messaging_service) = self.messaging_service.clone() else {
            return;
        };
        let now = Utc::now().timestamp();
        let limit = available.min(MESSAGE_OUTBOX_BATCH_SIZE) as u32;
        let entries = match messaging_service.claim_due_outbox(
            now,
            MESSAGE_OUTBOX_CLAIM_LEASE_SECONDS,
            limit,
        ) {
            Ok(entries) => entries,
            Err(error) => {
                warn!("Could not claim due direct-message outbox events: {error}");
                return;
            }
        };

        for entry in entries {
            match entry.peer_id.parse::<PeerId>() {
                Ok(peer_id) => {
                    let event_id = entry.event_id;
                    let message_id = entry.message_id;
                    self.enqueue_message_attempt(
                        peer_id,
                        "message".to_string(),
                        event_id.clone(),
                        message_id.clone(),
                        entry.payload,
                        None,
                    );
                    match messaging_service.record_outbox_sent(
                        &event_id,
                        &message_id,
                        Utc::now().timestamp(),
                    ) {
                        Ok(Some(change)) => self.emit_message_delivery_change(change),
                        Ok(None) => {}
                        Err(error) => warn!("Could not mark queued message as sent: {error}"),
                    }
                }
                Err(error) => self.apply_message_attempt_outcome(
                    MessageDeliveryAttemptOutcome::Failed(MessageDeliveryFailure {
                        event_id: entry.event_id,
                        message_id: entry.message_id,
                        kind: MessageDeliveryFailureKind::InvalidResponse,
                        detail: format!("queued peer ID is invalid: {error}"),
                    }),
                ),
            }
        }
    }

    fn configured_connected_relay(&self) -> Option<PeerId> {
        self.relay_addresses
            .iter()
            .filter_map(|address| {
                address.iter().find_map(|protocol| match protocol {
                    libp2p::multiaddr::Protocol::P2p(peer_id) => Some(peer_id),
                    _ => None,
                })
            })
            .find(|peer_id| self.connected_peers.contains_key(peer_id))
            .or_else(|| {
                PUBLIC_RELAYS.iter().find_map(|address| {
                    address
                        .parse::<Multiaddr>()
                        .ok()?
                        .iter()
                        .find_map(|protocol| match protocol {
                            libp2p::multiaddr::Protocol::P2p(peer_id)
                                if self.connected_peers.contains_key(&peer_id) =>
                            {
                                Some(peer_id)
                            }
                            _ => None,
                        })
                })
            })
    }

    fn dispatch_due_post_relay_outbox(&mut self) {
        let available =
            POST_RELAY_OUTBOX_MAX_IN_FLIGHT.saturating_sub(self.pending_post_relay_requests.len());
        if available == 0 {
            return;
        }
        let Some(posts_service) = self.posts_service.clone() else {
            return;
        };
        let Some(relay_peer_id) = self.configured_connected_relay() else {
            return;
        };
        let now = Utc::now().timestamp();
        let limit = available.min(POST_RELAY_OUTBOX_BATCH_SIZE) as u32;
        let entries = match posts_service.claim_due_relay_outbox(
            &relay_peer_id.to_string(),
            now,
            POST_RELAY_OUTBOX_CLAIM_LEASE_SECONDS,
            limit,
        ) {
            Ok(entries) => entries,
            Err(error) => {
                warn!("Could not claim durable post relay outbox: {error}");
                return;
            }
        };

        for entry in entries {
            let request: WireBoardSyncRequest =
                match ciborium::de::from_reader(entry.payload_cbor.as_slice()) {
                    Ok(request) => request,
                    Err(error) => {
                        let detail = format!("Persisted relay request is invalid CBOR: {error}");
                        let _ = posts_service.fail_relay_outbox_attempt(
                            &entry.event_id,
                            &detail,
                            false,
                            now,
                        );
                        self.emit_post_relay_status(
                            &entry,
                            if entry.attempt_count >= entry.max_attempts {
                                "failed"
                            } else {
                                "local_pending"
                            },
                            Some(detail),
                        );
                        continue;
                    }
                };
            if !post_relay_request_matches(&request, &entry.post_id, &entry.mutation_type) {
                let detail =
                    "Persisted relay request does not match its outbox identity".to_string();
                let _ =
                    posts_service.fail_relay_outbox_attempt(&entry.event_id, &detail, true, now);
                self.emit_post_relay_status(&entry, "conflict", Some(detail));
                continue;
            }
            let request_id = self
                .swarm
                .behaviour_mut()
                .board_sync
                .send_request(&relay_peer_id, request);
            self.pending_post_relay_requests.insert(
                request_id,
                PendingPostRelayRequest {
                    event_id: entry.event_id,
                    post_id: entry.post_id,
                    mutation_type: entry.mutation_type,
                    relay_peer_id,
                    deadline: Instant::now() + POST_RELAY_REQUEST_TIMEOUT,
                    attempt_count: entry.attempt_count,
                    max_attempts: entry.max_attempts,
                },
            );
        }
    }

    fn expire_post_relay_requests(&mut self) {
        let now = Instant::now();
        let expired: Vec<_> = self
            .pending_post_relay_requests
            .iter()
            .filter(|(_, pending)| pending.deadline <= now)
            .map(|(request_id, _)| *request_id)
            .collect();
        for request_id in expired {
            if let Some(pending) = self.pending_post_relay_requests.remove(&request_id) {
                self.fail_post_relay_request(pending, "Relay acknowledgement timed out", false);
            }
        }
    }

    fn fail_all_post_relay_requests(&mut self, detail: &str) {
        let pending = std::mem::take(&mut self.pending_post_relay_requests);
        for (_, request) in pending {
            self.fail_post_relay_request(request, detail, false);
        }
    }

    fn fail_post_relay_request(
        &self,
        pending: PendingPostRelayRequest,
        detail: &str,
        conflict: bool,
    ) {
        let Some(posts_service) = self.posts_service.as_ref() else {
            return;
        };
        if let Err(error) = posts_service.fail_relay_outbox_attempt(
            &pending.event_id,
            detail,
            conflict,
            Utc::now().timestamp(),
        ) {
            warn!("Could not persist post relay failure: {error}");
            return;
        }
        let status = if conflict {
            "conflict"
        } else if pending.attempt_count >= pending.max_attempts {
            "failed"
        } else {
            "local_pending"
        };
        let _ = self
            .event_tx
            .try_send(NetworkEvent::PostRelayStatusChanged {
                post_id: pending.post_id,
                event_id: pending.event_id,
                status: status.to_string(),
                error: Some(detail.to_string()),
            });
    }

    fn emit_post_relay_status(
        &self,
        entry: &crate::db::PostRelayOutboxEntry,
        status: &str,
        error: Option<String>,
    ) {
        let _ = self
            .event_tx
            .try_send(NetworkEvent::PostRelayStatusChanged {
                post_id: entry.post_id.clone(),
                event_id: entry.event_id.clone(),
                status: status.to_string(),
                error,
            });
    }

    fn apply_message_attempt_failures(&mut self, failures: Vec<MessageDeliveryFailure>) {
        for failure in failures {
            self.apply_message_attempt_outcome(MessageDeliveryAttemptOutcome::Failed(failure));
        }
    }

    fn apply_message_attempt_outcome(&mut self, outcome: MessageDeliveryAttemptOutcome) {
        let Some(messaging_service) = self.messaging_service.as_ref() else {
            return;
        };
        let now = Utc::now().timestamp();
        let transition = match &outcome {
            MessageDeliveryAttemptOutcome::Delivered(receipt) => {
                messaging_service.record_outbox_delivery(receipt, now)
            }
            MessageDeliveryAttemptOutcome::Failed(failure) => {
                messaging_service.record_outbox_failure(failure, now)
            }
        };
        match transition {
            Ok(Some(change)) => self.emit_message_delivery_change(change),
            Ok(None) => {}
            Err(error) => warn!("Could not persist message delivery outcome: {error}"),
        }
    }

    fn emit_message_delivery_change(
        &self,
        change: crate::services::messaging_service::MessageDeliveryStateChange,
    ) {
        if let Err(error) = self
            .event_tx
            .try_send(NetworkEvent::MessageDeliveryChanged {
                message_id: change.message_id,
                status: change.status,
                timestamp: change.timestamp,
                error: change.error,
            })
        {
            warn!("Could not emit message delivery update: {error}");
        }
    }

    /// Submit a foreground or background outbox attempt through the same
    /// correlation path. Background attempts pass `None` for `response_tx` and
    /// consume the typed outcome returned by response/failure maintenance.
    fn enqueue_message_attempt(
        &mut self,
        peer_id: PeerId,
        protocol: String,
        event_id: String,
        message_id: String,
        payload: Vec<u8>,
        response_tx: Option<oneshot::Sender<NetworkResponse>>,
    ) -> request_response::OutboundRequestId {
        let request_id = self.swarm.behaviour_mut().messaging.send_request(
            &peer_id,
            MessagingRequest {
                message_type: protocol,
                payload,
            },
        );
        self.pending_messaging_requests.insert(
            request_id.clone(),
            event_id,
            message_id,
            peer_id,
            Instant::now() + MESSAGE_REQUEST_TIMEOUT,
            response_tx,
        );
        request_id
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<ChatBehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on: {}", address);
                self.listening_addresses.push(address.clone());
                let _ = self
                    .event_tx
                    .send(NetworkEvent::ListeningOn {
                        address: address.to_string(),
                    })
                    .await;
            }

            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                info!("Connected to peer: {} at {:?}", peer_id, endpoint);
                let peer_info = PeerInfo {
                    peer_id: peer_id.to_string(),
                    addresses: vec![endpoint.get_remote_address().to_string()],
                    protocol_version: None,
                    agent_version: None,
                    is_connected: true,
                    last_seen: Some(chrono::Utc::now().timestamp()),
                };
                let first_connection = self.connected_peers.insert(peer_id, peer_info).is_none();
                self.stats.connected_peers = self.connected_peers.len();

                let _ = self
                    .event_tx
                    .send(NetworkEvent::PeerConnected {
                        peer_id: peer_id.to_string(),
                    })
                    .await;
                if first_connection {
                    self.reconcile_contact_acceptance_grants(peer_id);
                    self.publish_profile_to_contact(peer_id);
                }
            }

            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                info!("Disconnected from peer: {} (cause: {:?})", peer_id, cause);
                self.connected_peers.remove(&peer_id);
                self.stats.connected_peers = self.connected_peers.len();
                let failures = self.pending_messaging_requests.fail_peer(
                    &peer_id,
                    MessageDeliveryFailureKind::Disconnected,
                    "connection closed before acknowledgement",
                );
                self.apply_message_attempt_failures(failures);
                let post_requests: Vec<_> = self
                    .pending_post_relay_requests
                    .iter()
                    .filter(|(_, pending)| pending.relay_peer_id == peer_id)
                    .map(|(request_id, _)| *request_id)
                    .collect();
                for request_id in post_requests {
                    if let Some(pending) = self.pending_post_relay_requests.remove(&request_id) {
                        self.fail_post_relay_request(
                            pending,
                            "Relay disconnected before acknowledging the mutation",
                            false,
                        );
                    }
                }

                let _ = self
                    .event_tx
                    .send(NetworkEvent::PeerDisconnected {
                        peer_id: peer_id.to_string(),
                    })
                    .await;
            }

            SwarmEvent::ExternalAddrConfirmed { address } => {
                info!("External address confirmed: {}", address);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::ExternalAddressDiscovered {
                        address: address.to_string(),
                    })
                    .await;
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    warn!("Failed to connect to peer {}: {}", peer_id, error);
                } else {
                    warn!("Outgoing connection error: {}", error);
                }
            }

            SwarmEvent::Behaviour(behaviour_event) => {
                self.handle_behaviour_event(behaviour_event).await;
            }

            _ => {}
        }
    }

    async fn handle_content_sync_request(
        &mut self,
        peer: PeerId,
        _request_id: request_response::InboundRequestId,
        request: ContentSyncRequest,
        channel: ResponseChannel<ContentSyncResponse>,
    ) {
        let Some(ref content_sync_service) = self.content_sync_service else {
            let _ = self.swarm.behaviour_mut().content_sync.send_response(
                channel,
                ContentSyncResponse::Error {
                    error: "Content sync service unavailable".to_string(),
                },
            );
            return;
        };

        match request {
            ContentSyncRequest::Manifest {
                requester_peer_id,
                cursor,
                limit,
                timestamp,
                signature,
            } => {
                // Ensure peer id matches claimed requester
                if requester_peer_id != peer.to_string() {
                    let _ = self.swarm.behaviour_mut().content_sync.send_response(
                        channel,
                        ContentSyncResponse::Error {
                            error: "requester_peer_id mismatch".to_string(),
                        },
                    );
                    return;
                }

                match content_sync_service.process_manifest_request(
                    &requester_peer_id,
                    &cursor,
                    limit,
                    timestamp,
                    &signature,
                ) {
                    Ok(resp) => {
                        let response = ContentSyncResponse::Manifest {
                            responder_peer_id: resp.responder_peer_id,
                            posts: resp
                                .posts
                                .into_iter()
                                .map(|p| PostSummaryProto {
                                    post_id: p.post_id,
                                    author_peer_id: p.author_peer_id,
                                    lamport_clock: p.lamport_clock,
                                    content_type: p.content_type,
                                    has_media: p.has_media,
                                    media_hashes: p.media_hashes,
                                    created_at: p.created_at,
                                })
                                .collect(),
                            has_more: resp.has_more,
                            next_cursor: resp.next_cursor,
                            timestamp: resp.timestamp,
                            signature: resp.signature,
                        };

                        if let Err(e) = self
                            .swarm
                            .behaviour_mut()
                            .content_sync
                            .send_response(channel, response)
                        {
                            warn!("Failed to send content manifest response: {:?}", e);
                        }
                    }
                    Err(e) => {
                        let _ = self.swarm.behaviour_mut().content_sync.send_response(
                            channel,
                            ContentSyncResponse::Error {
                                error: e.to_string(),
                            },
                        );
                    }
                }
            }
            ContentSyncRequest::FetchPost {
                post_id,
                include_media,
                requester_peer_id,
                timestamp,
                signature,
            } => {
                // Ensure peer id matches claimed requester
                if requester_peer_id != peer.to_string() {
                    let _ = self.swarm.behaviour_mut().content_sync.send_response(
                        channel,
                        ContentSyncResponse::Error {
                            error: "requester_peer_id mismatch".to_string(),
                        },
                    );
                    return;
                }

                match content_sync_service.process_fetch_request(
                    &requester_peer_id,
                    &post_id,
                    include_media,
                    timestamp,
                    &signature,
                ) {
                    Ok(resp) => {
                        let response = ContentSyncResponse::Post {
                            post_id: resp.post_id,
                            author_peer_id: resp.author_peer_id,
                            content_type: resp.content_type,
                            content_text: resp.content_text,
                            visibility: resp.visibility,
                            lamport_clock: resp.lamport_clock,
                            created_at: resp.created_at,
                            signature: resp.signature,
                            media_hashes: resp.media_hashes,
                            media_items: resp
                                .media_items
                                .into_iter()
                                .map(|m| super::protocols::board_sync::WallPostMediaItem {
                                    media_hash: m.media_hash,
                                    media_type: m.media_type,
                                    mime_type: m.mime_type,
                                    file_name: m.file_name,
                                    file_size: m.file_size,
                                    width: m.width,
                                    height: m.height,
                                    duration_seconds: m.duration_seconds,
                                    sort_order: m.sort_order,
                                    signature: m.signature,
                                })
                                .collect(),
                        };

                        if let Err(e) = self
                            .swarm
                            .behaviour_mut()
                            .content_sync
                            .send_response(channel, response)
                        {
                            warn!("Failed to send fetch post response: {:?}", e);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to process fetch request from {}: {}", peer, e);
                        let _ = self.swarm.behaviour_mut().content_sync.send_response(
                            channel,
                            ContentSyncResponse::Error {
                                error: e.to_string(),
                            },
                        );
                    }
                }
            }
            ContentSyncRequest::FetchSocialEvents { .. } => {
                let _ = self.swarm.behaviour_mut().content_sync.send_response(
                    channel,
                    ContentSyncResponse::Error {
                        error: "social event sync service unavailable in this network path"
                            .to_string(),
                    },
                );
            }
        }
    }

    async fn handle_content_sync_response(
        &mut self,
        peer: PeerId,
        _request_id: request_response::OutboundRequestId,
        response: ContentSyncResponse,
    ) {
        let Some(ref content_sync_service) = self.content_sync_service else {
            return;
        };

        match response {
            ContentSyncResponse::Manifest {
                responder_peer_id,
                posts,
                has_more,
                next_cursor,
                timestamp,
                signature,
            } => {
                if responder_peer_id != peer.to_string() {
                    warn!(
                        "Content manifest responder mismatch: expected {}, got {}",
                        peer, responder_peer_id
                    );
                    return;
                }

                // Convert wire format to service format
                let service_posts: Vec<crate::services::PostSummary> = posts
                    .into_iter()
                    .map(|p| crate::services::PostSummary {
                        post_id: p.post_id,
                        author_peer_id: p.author_peer_id,
                        lamport_clock: p.lamport_clock,
                        content_type: p.content_type,
                        has_media: p.has_media,
                        media_hashes: p.media_hashes,
                        created_at: p.created_at,
                    })
                    .collect();

                match content_sync_service.process_manifest_response(
                    &responder_peer_id,
                    &service_posts,
                    has_more,
                    &next_cursor,
                    timestamp,
                    &signature,
                ) {
                    Ok(posts_to_fetch) => {
                        // Emit manifest received event
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::ContentManifestReceived {
                                peer_id: peer.to_string(),
                                post_count: posts_to_fetch.len(),
                                has_more,
                            })
                            .await;

                        // Issue fetch requests for posts we need
                        for post_id in posts_to_fetch {
                            match content_sync_service.create_fetch_request(post_id.clone(), false)
                            {
                                Ok(fetch_req) => {
                                    let request = ContentSyncRequest::FetchPost {
                                        post_id: fetch_req.post_id,
                                        include_media: fetch_req.include_media,
                                        requester_peer_id: fetch_req.requester_peer_id,
                                        timestamp: fetch_req.timestamp,
                                        signature: fetch_req.signature,
                                    };
                                    self.swarm
                                        .behaviour_mut()
                                        .content_sync
                                        .send_request(&peer, request);
                                    debug!("Sent fetch request for post {} to {}", post_id, peer);
                                }
                                Err(e) => {
                                    warn!("Failed to create fetch request for {}: {}", post_id, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to process manifest response: {}", e);
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::ContentSyncError {
                                peer_id: peer.to_string(),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            }
            ContentSyncResponse::Post {
                post_id,
                author_peer_id,
                content_type,
                content_text,
                visibility,
                lamport_clock,
                created_at,
                signature,
                media_hashes,
                media_items,
            } => {
                info!("Received post {} from {}", post_id, peer);

                // Verify the author matches the peer we requested from
                if author_peer_id != peer.to_string() {
                    warn!(
                        "Post author mismatch: expected {}, got {}",
                        peer, author_peer_id
                    );
                    return;
                }

                let signed_media_items: Vec<crate::services::SignedPostMediaMetadata> = media_items
                    .iter()
                    .map(|m| crate::services::SignedPostMediaMetadata {
                        media_hash: m.media_hash.clone(),
                        media_type: m.media_type.clone(),
                        mime_type: m.mime_type.clone(),
                        file_name: m.file_name.clone(),
                        file_size: m.file_size,
                        width: m.width,
                        height: m.height,
                        duration_seconds: m.duration_seconds,
                        sort_order: m.sort_order,
                        signature: m.signature.clone(),
                    })
                    .collect();

                // Store the remote post and verified media metadata
                match content_sync_service.store_remote_post(&RemotePostParams {
                    post_id: &post_id,
                    author_peer_id: &author_peer_id,
                    content_type: &content_type,
                    content_text: content_text.as_deref(),
                    visibility: &visibility,
                    lamport_clock,
                    created_at,
                    signature: &signature,
                    media_hashes: &media_hashes,
                    media_items: &signed_media_items,
                }) {
                    Ok(_) => {
                        info!("Stored remote post {} from {}", post_id, peer);
                        if let Err(e) = content_sync_service.store_author_sync_cursor(
                            &peer.to_string(),
                            &author_peer_id,
                            lamport_clock,
                        ) {
                            warn!(
                                "Failed to advance direct sync cursor for {} from {}: {}",
                                author_peer_id, peer, e
                            );
                        }
                        // Emit event for UI to refresh feed
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::ContentFetched {
                                peer_id: peer.to_string(),
                                post_id,
                            })
                            .await;
                    }
                    Err(e) => {
                        warn!("Failed to store remote post {}: {}", post_id, e);
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::ContentSyncError {
                                peer_id: peer.to_string(),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            }
            ContentSyncResponse::SocialEvents { events, .. } => {
                debug!(
                    "Received {} wall social events from {}; social event storage is handled by service callers",
                    events.len(),
                    peer
                );
            }
            ContentSyncResponse::Error { error } => {
                warn!("Content sync error from {}: {}", peer, error);
            }
        }
    }

    async fn handle_behaviour_event(&mut self, event: ChatBehaviourEvent) {
        match event {
            ChatBehaviourEvent::Mdns(event) => {
                self.handle_mdns_event(event).await;
            }

            ChatBehaviourEvent::Identify(event) => {
                self.handle_identify_event(event).await;
            }

            ChatBehaviourEvent::Kademlia(event) => {
                self.handle_kademlia_event(event).await;
            }

            ChatBehaviourEvent::Ping(event) => {
                self.handle_ping_event(event);
            }

            ChatBehaviourEvent::IdentityExchange(event) => {
                self.handle_identity_exchange_event(event).await;
            }

            ChatBehaviourEvent::Messaging(event) => {
                self.handle_messaging_event(event).await;
            }

            ChatBehaviourEvent::ContentSync(event) => {
                self.handle_content_sync_event(event).await;
            }

            ChatBehaviourEvent::BoardSync(event) => {
                self.handle_board_sync_event(event).await;
            }

            ChatBehaviourEvent::MediaSync(event) => {
                self.handle_media_sync_event(event).await;
            }

            ChatBehaviourEvent::Signaling(event) => {
                self.handle_signaling_event(event).await;
            }

            ChatBehaviourEvent::RelayClient(event) => {
                self.handle_relay_client_event(event).await;
            }

            ChatBehaviourEvent::Dcutr(event) => {
                self.handle_dcutr_event(event).await;
            }

            ChatBehaviourEvent::Autonat(event) => {
                self.handle_autonat_event(event).await;
            }
        }
    }

    /// Handle mDNS discovery and expiry events
    async fn handle_mdns_event(&mut self, event: mdns::Event) {
        match event {
            mdns::Event::Discovered(peers) => {
                for (peer_id, addr) in peers {
                    info!("mDNS discovered peer: {} at {}", peer_id, addr);
                    self.discovered_peers
                        .entry(peer_id)
                        .or_default()
                        .push(addr.clone());

                    // Add to Kademlia routing table
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr);

                    let _ = self
                        .event_tx
                        .send(NetworkEvent::PeerDiscovered {
                            peer_id: peer_id.to_string(),
                        })
                        .await;
                }
            }

            mdns::Event::Expired(peers) => {
                for (peer_id, addr) in peers {
                    debug!("mDNS peer expired: {} at {}", peer_id, addr);
                    if let Some(addrs) = self.discovered_peers.get_mut(&peer_id) {
                        addrs.retain(|a| a != &addr);
                        if addrs.is_empty() {
                            self.discovered_peers.remove(&peer_id);
                            let _ = self
                                .event_tx
                                .send(NetworkEvent::PeerExpired {
                                    peer_id: peer_id.to_string(),
                                })
                                .await;
                        }
                    }
                }
            }
        }
    }

    /// Handle libp2p Identify protocol events
    async fn handle_identify_event(&mut self, event: identify::Event) {
        if let identify::Event::Received { peer_id, info, .. } = event {
            debug!("Identified peer: {} - {}", peer_id, info.agent_version);
            if let Some(peer_info) = self.connected_peers.get_mut(&peer_id) {
                peer_info.protocol_version = Some(info.protocol_version);
                peer_info.agent_version = Some(info.agent_version);
            }

            // Add addresses to Kademlia
            for addr in info.listen_addrs {
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, addr);
            }

            // If this peer is a relay we're waiting on, request the reservation NOW.
            // This is the correct timing — the connection is fully negotiated and
            // the relay client transport knows about it.
            if let Some(relay_addr) = self.pending_relay_reservations.remove(&peer_id) {
                let circuit_listen_addr: Multiaddr = relay_addr
                    .clone()
                    .with(libp2p::multiaddr::Protocol::P2pCircuit);
                info!(
                    "Requesting relay reservation on {} (post-identify)",
                    circuit_listen_addr
                );
                match self.swarm.listen_on(circuit_listen_addr.clone()) {
                    Ok(id) => {
                        info!(
                            "Relay listener registered: {:?} on {}",
                            id, circuit_listen_addr
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to request relay reservation {}: {}",
                            circuit_listen_addr, e
                        );
                    }
                }
            }
        }
    }

    /// Handle Kademlia DHT events
    async fn handle_kademlia_event(&mut self, event: kad::Event) {
        if let kad::Event::RoutingUpdated { peer, .. } = event {
            debug!("Kademlia routing updated for peer: {}", peer);
        }
    }

    /// Handle ping protocol events
    fn handle_ping_event(&mut self, event: ping::Event) {
        if let Ok(rtt) = event.result {
            debug!("Ping to {} succeeded: {:?}", event.peer, rtt);
        }
    }

    /// Handle identity exchange request/response events
    async fn handle_identity_exchange_event(
        &mut self,
        event: request_response::Event<IdentityExchangeRequest, IdentityExchangeResponse>,
    ) {
        match event {
            request_response::Event::Message { peer, message, .. } => match message {
                request_response::Message::Request {
                    request_id,
                    request,
                    channel,
                } => {
                    info!("Received identity request from {}", peer);
                    self.handle_identity_request(peer, request_id, request, channel)
                        .await;
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    info!("Received identity response from {}", peer);
                    self.handle_identity_response(peer, request_id, response)
                        .await;
                }
            },
            request_response::Event::OutboundFailure {
                peer,
                request_id,
                error,
                ..
            } => {
                if let Some((contact_request_id, action)) =
                    self.pending_identity_requests.remove(&request_id)
                {
                    if let Some(service) = &self.contacts_service {
                        let message = format!("Contact request delivery failed: {error}");
                        if action == "revoked" {
                            let _ = service.retry_contact_revocation(
                                &contact_request_id,
                                &message,
                                chrono::Utc::now().timestamp(),
                            );
                            let _ = self.swarm.disconnect_peer_id(peer);
                        }
                        // Acceptance is already a complete local transaction.
                        // Transport failure is therefore an offline delivery
                        // outcome, not permission to regress durable state.
                        let failure_status = match action.as_str() {
                            "revoked" => "revoked",
                            "accepted" => "accepted",
                            _ => "failed",
                        };
                        let _ = service.update_contact_request(
                            &contact_request_id,
                            failure_status,
                            Some(&action),
                            Some(&message),
                            chrono::Utc::now().timestamp(),
                        );
                        if let Ok(Some(request)) = service.contact_request(&contact_request_id) {
                            let _ = self
                                .event_tx
                                .send(NetworkEvent::ContactRequestChanged {
                                    request_id: request.request_id,
                                    peer_id: request.peer_id,
                                    display_name: request.display_name,
                                    direction: request.direction,
                                    status: failure_status.into(),
                                })
                                .await;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Handle messaging protocol request/response events
    async fn handle_messaging_event(
        &mut self,
        event: request_response::Event<MessagingRequest, MessagingResponse>,
    ) {
        match event {
            request_response::Event::Message { peer, message, .. } => match message {
                request_response::Message::Request {
                    request_id,
                    request,
                    channel,
                } => {
                    debug!("Received message request from {}", peer);
                    self.handle_messaging_request(peer, request_id, request, channel)
                        .await;
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    debug!("Received message response from {}", peer);
                    if let Some(outcome) = self.pending_messaging_requests.complete_remote(
                        &request_id,
                        &peer,
                        response,
                    ) {
                        self.apply_message_attempt_outcome(outcome);
                    } else {
                        debug!(
                            "Received messaging response from {} without a pending request",
                            peer
                        );
                    }
                }
            },
            request_response::Event::OutboundFailure {
                peer,
                request_id,
                error,
                ..
            } => {
                warn!("Messaging outbound failure to {}: {}", peer, error);
                if let Some(failure) = self.pending_messaging_requests.fail(
                    &request_id,
                    MessageDeliveryFailureKind::Network,
                    error.to_string(),
                ) {
                    self.apply_message_attempt_outcome(MessageDeliveryAttemptOutcome::Failed(
                        failure,
                    ));
                }
            }
            request_response::Event::InboundFailure { peer, error, .. } => {
                warn!("Messaging inbound failure from {}: {}", peer, error);
            }
            request_response::Event::ResponseSent { .. } => {}
        }
    }

    /// Handle content sync protocol request/response events
    async fn handle_content_sync_event(
        &mut self,
        event: request_response::Event<ContentSyncRequest, ContentSyncResponse>,
    ) {
        if let request_response::Event::Message { peer, message, .. } = event {
            match message {
                request_response::Message::Request {
                    request_id,
                    request,
                    channel,
                } => {
                    debug!("Received content sync request from {}", peer);
                    self.handle_content_sync_request(peer, request_id, request, channel)
                        .await;
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    debug!("Received content sync response from {}", peer);
                    self.handle_content_sync_response(peer, request_id, response)
                        .await;
                }
            }
        }
    }

    /// Handle board sync protocol events (messages and outbound failures)
    async fn handle_board_sync_event(
        &mut self,
        event: request_response::Event<WireBoardSyncRequest, WireBoardSyncResponse>,
    ) {
        match event {
            request_response::Event::Message { peer, message, .. } => match message {
                request_response::Message::Request { channel, .. } => {
                    // Client doesn't serve board requests; send error
                    let _ = self.swarm.behaviour_mut().board_sync.send_response(
                        channel,
                        WireBoardSyncResponse::Error {
                            error: "Not a relay server".to_string(),
                        },
                    );
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    self.resolve_post_relay_response(request_id, peer, &response);
                    self.handle_board_sync_response(peer, response).await;
                }
            },

            request_response::Event::OutboundFailure {
                peer,
                request_id,
                error,
                ..
            } => {
                if let Some(pending) = self.pending_post_relay_requests.remove(&request_id) {
                    self.fail_post_relay_request(
                        pending,
                        &format!("Relay transport failed before acknowledgement: {error}"),
                        false,
                    );
                }
                // Clean up any pending community probe / registration state.
                // This happens when the relay doesn't support the board sync protocol.
                let was_probe = self.pending_community_probes.remove(&peer).is_some();
                let was_registration = self.pending_board_registrations.remove(&peer);
                if let Some(pending) = self.pending_name_registration.remove(&peer) {
                    let _ = pending.response_tx.send(NetworkResponse::Error(format!(
                        "Name registration failed while contacting the relay: {error}"
                    )));
                }
                if was_probe || was_registration {
                    debug!(
                        "Relay {} does not support board sync protocol (outbound failure: {})",
                        peer, error
                    );
                } else {
                    warn!("Board sync outbound failure to peer {}: {}", peer, error);
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::BoardSyncError {
                            relay_peer_id: peer.to_string(),
                            error: format!("Failed to reach relay: {}", error),
                        })
                        .await;
                }
            }

            _ => {}
        }
    }

    /// Handle media sync events (P2P image transfer)
    async fn handle_media_sync_event(
        &mut self,
        event: request_response::Event<
            super::protocols::media_sync::MediaFetchRequest,
            super::protocols::media_sync::MediaFetchResponse,
        >,
    ) {
        match event {
            request_response::Event::Message { peer, message, .. } => match message {
                request_response::Message::Request {
                    request, channel, ..
                } => {
                    // Inbound: a peer is requesting media from us
                    let response = self.handle_media_fetch_request(peer, &request);
                    if let Err(e) = self
                        .swarm
                        .behaviour_mut()
                        .media_sync
                        .send_response(channel, response)
                    {
                        warn!("Failed to send media sync response: {:?}", e);
                    }
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    // Outbound: we received media bytes from a peer
                    let pending = self.pending_media_fetches.remove(&request_id);
                    if let Some(pending) = pending.as_ref() {
                        self.pending_media_fetch_by_hash.remove(&pending.media_hash);
                    }
                    self.handle_media_chunk_response(peer, pending, response)
                        .await;
                }
            },
            request_response::Event::OutboundFailure {
                peer,
                request_id,
                error,
                ..
            } => {
                warn!("Media fetch outbound failure to peer {}: {}", peer, error);
                if let Some(pending) = self.pending_media_fetches.remove(&request_id) {
                    self.pending_media_fetch_by_hash.remove(&pending.media_hash);
                    self.transition_media(
                        Some(&pending.profile_id),
                        &pending.media_hash,
                        MediaTransferUpdate {
                            status: "failed",
                            bytes_received: None,
                            total_bytes: None,
                            error_code: Some("transport_timeout"),
                            error_message: Some(
                                "The attachment source did not respond. You can retry.",
                            ),
                            increment_attempt: false,
                        },
                    );
                }
            }
            request_response::Event::InboundFailure { peer, error, .. } => {
                warn!("Media fetch inbound failure from peer {}: {}", peer, error);
            }
            _ => {}
        }
    }

    fn transition_media(
        &self,
        profile_id: Option<&str>,
        hash: &str,
        update: MediaTransferUpdate<'_>,
    ) {
        let Some(media_service) = self.media_service.as_ref() else {
            return;
        };
        match media_service.update_transfer(hash, update) {
            Ok(state) => {
                let profile_id = profile_id
                    .map(str::to_owned)
                    .or_else(|| self.identity_service.get_peer_id().ok());
                if let Some(profile_id) = profile_id {
                    let _ = self
                        .event_tx
                        .try_send(NetworkEvent::MediaTransferChanged { profile_id, state });
                }
            }
            Err(error) => warn!("Could not persist media lifecycle update: {error}"),
        }
    }

    /// Handle voice-call signaling protocol events.
    async fn handle_signaling_event(
        &mut self,
        event: request_response::Event<SignalingEnvelope, SignalingResponse>,
    ) {
        match event {
            request_response::Event::Message { peer, message, .. } => match message {
                request_response::Message::Request {
                    request, channel, ..
                } => {
                    let response = self.handle_signaling_request(peer, request).await;
                    if let Err(error) = self
                        .swarm
                        .behaviour_mut()
                        .signaling
                        .send_response(channel, response)
                    {
                        warn!("Failed to send signaling response to {}: {:?}", peer, error);
                    }
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    self.handle_signaling_response(peer, request_id, response)
                        .await;
                }
            },
            request_response::Event::OutboundFailure {
                peer,
                request_id,
                error,
                ..
            } => {
                let error_message = format!("SIGNALING_NETWORK_FAILURE: {}", error);
                warn!("Signaling outbound failure to {}: {}", peer, error);
                if let Some(response_tx) = self.pending_signaling_requests.remove(&request_id) {
                    let _ = response_tx.send(NetworkResponse::Error(error_message.clone()));
                }
                let _ = self
                    .event_tx
                    .send(NetworkEvent::CallSignalingError {
                        peer_id: peer.to_string(),
                        error: error_message,
                    })
                    .await;
            }
            request_response::Event::InboundFailure { peer, error, .. } => {
                let error_message = format!("SIGNALING_INBOUND_FAILURE: {}", error);
                warn!("Signaling inbound failure from {}: {}", peer, error);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::CallSignalingError {
                        peer_id: peer.to_string(),
                        error: error_message,
                    })
                    .await;
            }
            _ => {}
        }
    }

    async fn handle_signaling_request(
        &mut self,
        peer: PeerId,
        request: SignalingEnvelope,
    ) -> SignalingResponse {
        let call_id = Some(request.call_id().to_string());

        if request.sender_peer_id != peer.to_string() {
            let error = format!(
                "sender peer mismatch: envelope sender {} arrived from {}",
                request.sender_peer_id, peer
            );
            warn!("Rejected signaling request: {}", error);
            return SignalingResponse::rejected(call_id, error);
        }

        let Some(ref calling_service) = self.calling_service else {
            let error = "Calling service not available".to_string();
            warn!("Rejected signaling request from {}: {}", peer, error);
            return SignalingResponse::rejected(call_id, error);
        };

        match calling_service.process_incoming_signaling(&request) {
            Ok(crate::services::calling_service::IncomingSignalingOutcome::Applied) => {
                let _ = self
                    .event_tx
                    .send(NetworkEvent::CallSignalingReceived {
                        peer_id: peer.to_string(),
                        message: request.clone(),
                    })
                    .await;
                SignalingResponse::accepted(request.call_id().to_string())
            }
            Ok(crate::services::calling_service::IncomingSignalingOutcome::Duplicate) => {
                debug!(
                    "Ignored duplicate signaling request {} from {}",
                    request.call_id(),
                    peer
                );
                // Offers and answers both carry SDP that the WebView cannot
                // reconstruct from the persisted call session. Re-publish a
                // verified replay so bounded sender retries can close a
                // backend-to-frontend listener race without mutating state.
                if matches!(
                    &request.payload,
                    SignalingPayload::Offer(_) | SignalingPayload::Answer(_)
                ) {
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::CallSignalingReceived {
                            peer_id: peer.to_string(),
                            message: request.clone(),
                        })
                        .await;
                }
                SignalingResponse::accepted(request.call_id().to_string())
            }
            Err(error) => {
                warn!(
                    "Rejected signaling request {} from {}: {}",
                    request.call_id(),
                    peer,
                    error
                );
                SignalingResponse::rejected(call_id, error.to_string())
            }
        }
    }

    async fn handle_signaling_response(
        &mut self,
        peer: PeerId,
        request_id: request_response::OutboundRequestId,
        response: SignalingResponse,
    ) {
        if let Some(response_tx) = self.pending_signaling_requests.remove(&request_id) {
            if response.accepted {
                let _ = response_tx.send(NetworkResponse::Ok);
            } else {
                let error = response
                    .error
                    .unwrap_or_else(|| "remote peer rejected signaling request".to_string());
                let _ = response_tx.send(NetworkResponse::Error(format!(
                    "SIGNALING_REJECTED: {}",
                    error
                )));
            }
        } else {
            debug!(
                "Received signaling response from {} without pending request",
                peer
            );
        }
    }

    /// Handle an inbound media fetch request (serve media to a peer)
    fn handle_media_fetch_request(
        &self,
        peer: PeerId,
        request: &super::protocols::media_sync::MediaFetchRequest,
    ) -> super::protocols::media_sync::MediaFetchResponse {
        use super::protocols::media_sync::MediaFetchResponse;

        // Verify the requester_peer_id matches the actual peer
        if request.requester_peer_id != peer.to_string() {
            return MediaFetchResponse::Error {
                error: "peer_id mismatch".to_string(),
            };
        }

        let Some(contacts_service) = self.contacts_service.as_ref() else {
            return MediaFetchResponse::Error {
                error: "Authorization unavailable".to_string(),
            };
        };
        if !contacts_service
            .is_contact(&request.requester_peer_id)
            .unwrap_or(false)
            || contacts_service
                .is_blocked(&request.requester_peer_id)
                .unwrap_or(true)
        {
            info!("Media fetch denied for an unknown or blocked requester");
            return MediaFetchResponse::Error {
                error: "Not authorized".to_string(),
            };
        }
        if (chrono::Utc::now().timestamp() - request.timestamp).abs() > 300 {
            return MediaFetchResponse::Error {
                error: "Request expired".to_string(),
            };
        }
        let verified = contacts_service
            .get_public_key(&request.requester_peer_id)
            .ok()
            .flatten()
            .and_then(|key| <[u8; 32]>::try_from(key.as_slice()).ok())
            .and_then(|key| ed25519_dalek::VerifyingKey::from_bytes(&key).ok())
            .and_then(|key| {
                let signable = crate::services::SignableMediaFetchRequest {
                    media_hash: request.media_hash.clone(),
                    offset: request.offset,
                    max_bytes: request.max_bytes,
                    requester_peer_id: request.requester_peer_id.clone(),
                    timestamp: request.timestamp,
                };
                crate::services::verify(&key, &signable, &request.signature).ok()
            })
            .unwrap_or(false);
        if !verified {
            return MediaFetchResponse::Error {
                error: "Not authorized".to_string(),
            };
        }

        // Read media from storage
        let media_service = match &self.media_service {
            Some(s) => s,
            None => {
                return MediaFetchResponse::Error {
                    error: "Media service unavailable".to_string(),
                };
            }
        };

        if !media_service.has_media(&request.media_hash) {
            return MediaFetchResponse::Error {
                error: "Media not found".to_string(),
            };
        }

        match media_service.get_media_range(&request.media_hash, request.offset, request.max_bytes)
        {
            Ok(range) => {
                debug!(
                    "Serving bounded media range {}+{} of {} to {}",
                    range.offset,
                    range.data.len(),
                    range.total_bytes,
                    peer
                );
                MediaFetchResponse::MediaChunk {
                    media_hash: request.media_hash.clone(),
                    mime_type: range.mime_type,
                    offset: range.offset,
                    total_bytes: range.total_bytes,
                    data: range.data,
                    eof: range.eof,
                }
            }
            Err(e) => MediaFetchResponse::Error {
                error: format!("Failed to read media: {}", e),
            },
        }
    }

    fn queue_media_range(
        &mut self,
        peer_id: PeerId,
        media_hash: String,
        profile_id: String,
        offset: u64,
        increment_attempt: bool,
    ) -> std::result::Result<(), String> {
        use super::protocols::media_sync::MediaFetchRequest;
        if self.pending_media_fetch_by_hash.contains_key(&media_hash) {
            return Ok(());
        }
        let now = chrono::Utc::now().timestamp();
        let signable = crate::services::SignableMediaFetchRequest {
            media_hash: media_hash.clone(),
            offset,
            max_bytes: crate::services::media_service::DEFAULT_CHUNK_SIZE,
            requester_peer_id: profile_id.clone(),
            timestamp: now,
        };
        let signature = self
            .identity_service
            .sign(&signable)
            .map_err(|error| error.to_string())?;
        let request = MediaFetchRequest {
            media_hash: media_hash.clone(),
            offset,
            max_bytes: crate::services::media_service::DEFAULT_CHUNK_SIZE,
            requester_peer_id: profile_id.clone(),
            timestamp: now,
            signature,
        };
        self.transition_media(
            Some(&profile_id),
            &media_hash,
            MediaTransferUpdate {
                status: "transferring",
                bytes_received: Some(offset),
                total_bytes: None,
                error_code: None,
                error_message: None,
                increment_attempt,
            },
        );
        let request_id = self
            .swarm
            .behaviour_mut()
            .media_sync
            .send_request(&peer_id, request);
        self.pending_media_fetch_by_hash
            .insert(media_hash.clone(), request_id);
        self.pending_media_fetches.insert(
            request_id,
            PendingMediaFetch {
                peer_id,
                media_hash,
                profile_id,
                offset,
            },
        );
        Ok(())
    }

    async fn handle_media_chunk_response(
        &mut self,
        peer: PeerId,
        pending: Option<PendingMediaFetch>,
        response: super::protocols::media_sync::MediaFetchResponse,
    ) {
        use super::protocols::media_sync::MediaFetchResponse;
        let Some(pending) = pending else {
            warn!("Ignoring uncorrelated media response from {peer}");
            return;
        };
        match response {
            MediaFetchResponse::MediaChunk {
                media_hash,
                mime_type,
                offset,
                total_bytes,
                data,
                eof,
            } => {
                if pending.peer_id != peer
                    || pending.media_hash != media_hash
                    || pending.offset != offset
                    || data.len() > crate::services::media_service::DEFAULT_CHUNK_SIZE as usize
                    || eof != (offset.saturating_add(data.len() as u64) == total_bytes)
                {
                    self.transition_media(
                        Some(&pending.profile_id),
                        &pending.media_hash,
                        MediaTransferUpdate {
                            status: "failed",
                            bytes_received: Some(pending.offset),
                            total_bytes: None,
                            error_code: Some("response_mismatch"),
                            error_message: Some("The attachment response could not be verified."),
                            increment_attempt: false,
                        },
                    );
                    return;
                }
                let authorized = self
                    .contacts_service
                    .as_ref()
                    .and_then(|service| service.get_contact(&peer.to_string()).ok().flatten())
                    .is_some_and(|contact| !contact.is_blocked);
                if !authorized {
                    self.transition_media(
                        Some(&pending.profile_id),
                        &media_hash,
                        MediaTransferUpdate {
                            status: "failed",
                            bytes_received: Some(offset),
                            total_bytes: Some(total_bytes),
                            error_code: Some("authorization_revoked"),
                            error_message: Some(
                                "The attachment source is no longer an active contact.",
                            ),
                            increment_attempt: false,
                        },
                    );
                    return;
                }
                let Some(service) = self.media_service.clone() else {
                    self.transition_media(
                        Some(&pending.profile_id),
                        &media_hash,
                        MediaTransferUpdate {
                            status: "failed",
                            bytes_received: Some(offset),
                            total_bytes: Some(total_bytes),
                            error_code: Some("service_unavailable"),
                            error_message: Some(
                                "Media storage is unavailable. Restart Harbor and retry.",
                            ),
                            increment_attempt: false,
                        },
                    );
                    return;
                };
                match service.store_received_range(
                    &media_hash,
                    &mime_type,
                    offset,
                    total_bytes,
                    &data,
                ) {
                    Ok(true) => {
                        if let Some(contacts) = self.contacts_service.as_ref() {
                            let old = contacts
                                .get_contact(&peer.to_string())
                                .ok()
                                .flatten()
                                .and_then(|contact| contact.avatar_hash);
                            match contacts
                                .promote_verified_profile_avatar(&peer.to_string(), &media_hash)
                            {
                                Ok(true) => {
                                    if let Some(old) = old.filter(|old| old != &media_hash) {
                                        let _ = service.delete_media_if_orphaned(&old);
                                    }
                                }
                                Ok(false) => {}
                                Err(error) => {
                                    warn!("Could not promote verified profile avatar: {error}")
                                }
                            }
                        }
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::MediaFetched {
                                peer_id: peer.to_string(),
                                media_hash: media_hash.clone(),
                            })
                            .await;
                        if let Err(error) = service.enforce_cache_policy() {
                            warn!("Failed to enforce media cache policy after transfer: {error}");
                        }
                    }
                    Ok(false) => {
                        let next = offset + data.len() as u64;
                        if let Err(error) = self.queue_media_range(
                            peer,
                            media_hash.clone(),
                            pending.profile_id.clone(),
                            next,
                            false,
                        ) {
                            warn!("Failed to request next media range: {error}");
                            self.transition_media(
                                Some(&pending.profile_id),
                                &media_hash,
                                MediaTransferUpdate {
                                    status: "failed",
                                    bytes_received: Some(next),
                                    total_bytes: Some(total_bytes),
                                    error_code: Some("start_failed"),
                                    error_message: Some(
                                        "The next attachment range could not be requested.",
                                    ),
                                    increment_attempt: false,
                                },
                            );
                        }
                    }
                    Err(error) => {
                        warn!("Failed to persist media range from {peer}: {error}");
                        self.transition_media(
                            Some(&pending.profile_id),
                            &media_hash,
                            MediaTransferUpdate {
                                status: "failed",
                                bytes_received: Some(offset),
                                total_bytes: Some(total_bytes),
                                error_code: Some("storage_failed"),
                                error_message: Some(
                                    "Harbor could not save this attachment. You can retry.",
                                ),
                                increment_attempt: false,
                            },
                        );
                    }
                }
            }
            MediaFetchResponse::Error { error } => {
                let unavailable = error.to_ascii_lowercase().contains("not found");
                self.transition_media(
                    Some(&pending.profile_id),
                    &pending.media_hash,
                    MediaTransferUpdate {
                        status: if unavailable { "unavailable" } else { "failed" },
                        bytes_received: Some(pending.offset),
                        total_bytes: None,
                        error_code: Some(if unavailable {
                            "source_missing"
                        } else {
                            "remote_error"
                        }),
                        error_message: Some(if unavailable {
                            "The source does not currently have this attachment."
                        } else {
                            "The source could not provide this attachment. You can retry."
                        }),
                        increment_attempt: false,
                    },
                );
            }
        }
    }

    /// Handle relay client events
    async fn handle_relay_client_event(&mut self, event: relay::client::Event) {
        match event {
            relay::client::Event::ReservationReqAccepted {
                relay_peer_id,
                renewal,
                limit: _,
            } => {
                let local_peer_id = *self.swarm.local_peer_id();
                info!(
                    "Relay reservation accepted by {} (renewal: {})",
                    relay_peer_id, renewal
                );

                // Build full relay circuit address WITH transport prefix.
                // Look up the relay peer's transport address from connected peers
                // so other peers can actually reach us through this relay.
                let mut relay_circuit_addr: Option<Multiaddr> = None;

                if let Some(peer_info) = self.connected_peers.get(&relay_peer_id) {
                    for addr_str in &peer_info.addresses {
                        if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                            // Strip /p2p/ from the address to get transport-only
                            let transport_addr: Multiaddr = addr
                                .iter()
                                .filter(|p| !matches!(p, libp2p::multiaddr::Protocol::P2p(_)))
                                .collect();

                            if transport_addr.to_string().is_empty() {
                                continue;
                            }

                            // Build: TRANSPORT/p2p/RELAY_ID/p2p-circuit/p2p/LOCAL_ID
                            let circuit_str = format!(
                                "{}/p2p/{}/p2p-circuit/p2p/{}",
                                transport_addr, relay_peer_id, local_peer_id
                            );
                            match circuit_str.parse::<Multiaddr>() {
                                Ok(full_circuit_addr) => {
                                    relay_circuit_addr = Some(full_circuit_addr);
                                    break;
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to parse relay circuit multiaddr '{}': {}",
                                        circuit_str, e
                                    );
                                    continue;
                                }
                            }
                        }
                    }
                }

                // Fallback: if we couldn't find a transport address, use bare p2p form
                let relay_circuit_addr = if let Some(addr) = relay_circuit_addr {
                    addr
                } else {
                    let fallback_str =
                        format!("/p2p/{}/p2p-circuit/p2p/{}", relay_peer_id, local_peer_id);
                    match fallback_str.parse() {
                        Ok(addr) => addr,
                        Err(e) => {
                            error!(
                                "Failed to parse fallback relay circuit multiaddr '{}': {}",
                                fallback_str, e
                            );
                            return;
                        }
                    }
                };

                // Register as external address so Identify advertises it to other peers
                self.swarm.add_external_address(relay_circuit_addr.clone());
                info!(
                    "Added relay circuit as external address: {}",
                    relay_circuit_addr
                );

                // Store the relay address if not already present
                if !self.relay_addresses.contains(&relay_circuit_addr) {
                    self.relay_addresses.push(relay_circuit_addr.clone());
                    info!("Added relay address: {}", relay_circuit_addr);

                    // Emit event to frontend
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::RelayConnected {
                            relay_address: relay_circuit_addr.to_string(),
                        })
                        .await;
                }

                // Update NAT status to Private (we're behind NAT but reachable via relay)
                if self.nat_status != NatStatus::Public {
                    self.nat_status = NatStatus::Private;
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::NatStatusChanged {
                            status: self.nat_status,
                        })
                        .await;
                }

                // Probe the relay for community support.
                // Step 1: Send RegisterPeer so the relay has our public key.
                // Step 2 (after PeerRegistered response): Send ListBoards to detect boards.
                // If the relay responds with a BoardList, it's a community relay and we auto-join.
                // If it returns an error (non-community relay), the probe silently fails.
                if !self.community_relays.contains_key(&relay_peer_id) {
                    if let Some(ref board_service) = self.board_service {
                        // Reconstruct the relay's original multiaddr for storing later
                        let relay_addr_str =
                            if let Some(peer_info) = self.connected_peers.get(&relay_peer_id) {
                                peer_info.addresses.first().cloned().unwrap_or_default()
                            } else {
                                relay_peer_id.to_string()
                            };

                        // Store relay addr for later use when community is confirmed
                        self.pending_community_probes
                            .insert(relay_peer_id, relay_addr_str);

                        match board_service.create_peer_registration() {
                            Ok(reg) => {
                                info!(
                                    "Probing relay {} for community support (RegisterPeer first)",
                                    relay_peer_id
                                );
                                self.pending_board_registrations.insert(relay_peer_id);
                                let request = WireBoardSyncRequest::RegisterPeer {
                                    peer_id: reg.peer_id,
                                    public_key: reg.public_key,
                                    display_name: reg.display_name,
                                    timestamp: reg.timestamp,
                                    signature: reg.signature,
                                };
                                self.swarm
                                    .behaviour_mut()
                                    .board_sync
                                    .send_request(&relay_peer_id, request);
                            }
                            Err(e) => {
                                debug!(
                                    "Skipping community probe for relay {} (no identity?): {}",
                                    relay_peer_id, e
                                );
                                self.pending_community_probes.remove(&relay_peer_id);
                            }
                        }
                    }
                }
            }

            relay::client::Event::OutboundCircuitEstablished {
                relay_peer_id,
                limit: _,
            } => {
                debug!("Outbound circuit established via relay {}", relay_peer_id);
            }

            relay::client::Event::InboundCircuitEstablished {
                src_peer_id,
                limit: _,
            } => {
                debug!("Inbound circuit established from {}", src_peer_id);
            }
        }
    }

    /// Handle DCUtR (hole punching) events
    /// Note: dcutr::Event is a struct with remote_peer_id and result fields
    async fn handle_dcutr_event(&mut self, event: dcutr::Event) {
        let remote_peer_id = event.remote_peer_id;
        match event.result {
            Ok(_connection_id) => {
                info!(
                    "Direct connection upgrade succeeded with {}",
                    remote_peer_id
                );
                // Emit event to frontend
                let _ = self
                    .event_tx
                    .send(NetworkEvent::HolePunchSucceeded {
                        peer_id: remote_peer_id.to_string(),
                    })
                    .await;
            }
            Err(error) => {
                debug!(
                    "Direct connection upgrade failed with {}: {:?}",
                    remote_peer_id, error
                );
                // Connection stays relayed - this is fine
            }
        }
    }

    /// Handle AutoNAT events
    async fn handle_autonat_event(&mut self, event: autonat::Event) {
        match event {
            autonat::Event::StatusChanged { old, new } => {
                info!("AutoNAT status changed from {:?} to {:?}", old, new);

                let new_nat_status = match new {
                    autonat::NatStatus::Public(addr) => {
                        info!("AutoNAT: We have a public address: {}", addr);
                        // Store the external address
                        if !self.external_addresses.contains(&addr) {
                            self.external_addresses.push(addr.clone());
                            let _ = self
                                .event_tx
                                .send(NetworkEvent::ExternalAddressDiscovered {
                                    address: addr.to_string(),
                                })
                                .await;
                        }
                        NatStatus::Public
                    }
                    autonat::NatStatus::Private => {
                        info!("AutoNAT: We are behind NAT, attempting relay connection...");
                        // Try to connect to relays if we haven't already
                        if !self.relay_connection_attempted {
                            self.connect_to_relays().await;
                        }
                        NatStatus::Private
                    }
                    autonat::NatStatus::Unknown => NatStatus::Unknown,
                };

                if self.nat_status != new_nat_status {
                    self.nat_status = new_nat_status;
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::NatStatusChanged {
                            status: self.nat_status,
                        })
                        .await;
                }
            }

            autonat::Event::InboundProbe(_) | autonat::Event::OutboundProbe(_) => {
                // These are just probe events, no action needed
            }
        }
    }

    /// Connect to public relay servers for NAT traversal
    async fn connect_to_relays(&mut self) {
        self.relay_connection_attempted = true;
        info!("Attempting to connect to public relay servers...");

        for relay_addr_str in PUBLIC_RELAYS {
            match relay_addr_str.parse::<Multiaddr>() {
                Ok(relay_addr) => {
                    // Extract peer ID from the multiaddress
                    let peer_id = relay_addr.iter().find_map(|proto| {
                        if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                            Some(peer_id)
                        } else {
                            None
                        }
                    });

                    if let Some(relay_peer_id) = peer_id {
                        info!("Dialing relay server: {}", relay_addr);

                        // Extract transport-only address (without /p2p/...)
                        let addr_without_peer: Multiaddr = relay_addr
                            .iter()
                            .filter(|p| !matches!(p, libp2p::multiaddr::Protocol::P2p(_)))
                            .collect();

                        // Add to Kademlia for routing
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&relay_peer_id, addr_without_peer.clone());

                        // Dial the relay
                        if let Err(e) = self.swarm.dial(relay_addr.clone()) {
                            warn!("Failed to dial relay {}: {}", relay_addr, e);
                        } else {
                            info!(
                                "Dial initiated to relay: {} (waiting for connection...)",
                                relay_peer_id
                            );
                        }

                        // Queue relay reservation for after Identify completes.
                        // listen_on must be called AFTER the connection is fully negotiated
                        // (Identify::Received), not immediately after dial — otherwise the
                        // relay client transport doesn't know about the connection yet.
                        self.pending_relay_reservations
                            .insert(relay_peer_id, relay_addr.clone());
                        info!(
                            "Relay reservation queued for {} (will request after identify)",
                            relay_peer_id
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to parse relay address '{}': {}", relay_addr_str, e);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn emit_wall_sync_status(
        event_tx: mpsc::Sender<NetworkEvent>,
        scope: &str,
        status: &str,
        phase: &str,
        relay_peer_id: Option<String>,
        author_peer_id: Option<String>,
        post_id: Option<String>,
        media_hash: Option<String>,
        post_count: Option<usize>,
        cursor: Option<u64>,
        error: Option<String>,
    ) {
        tracing::info!(
            scope,
            status,
            phase,
            relay_peer_id = relay_peer_id.as_deref(),
            author_peer_id = author_peer_id.as_deref(),
            post_id = post_id.as_deref(),
            media_hash = media_hash.as_deref(),
            post_count,
            cursor,
            error = error.as_deref(),
            "wall sync status"
        );
        let _ = event_tx
            .send(NetworkEvent::WallSyncStatus {
                scope: scope.to_string(),
                status: status.to_string(),
                phase: phase.to_string(),
                relay_peer_id,
                author_peer_id,
                post_id,
                media_hash,
                post_count,
                cursor,
                error,
                occurred_at: Utc::now().timestamp(),
            })
            .await;
    }

    fn resolve_post_relay_response(
        &mut self,
        request_id: request_response::OutboundRequestId,
        peer: PeerId,
        response: &WireBoardSyncResponse,
    ) {
        let Some(pending) = self.pending_post_relay_requests.remove(&request_id) else {
            return;
        };
        if pending.relay_peer_id != peer {
            self.fail_post_relay_request(
                pending,
                "Relay response came from a different transport peer",
                true,
            );
            return;
        }
        let matches = match response {
            WireBoardSyncResponse::WallPostStored { post_id } => {
                pending.mutation_type != "delete" && post_id == &pending.post_id
            }
            WireBoardSyncResponse::WallPostDeleted { post_id } => {
                pending.mutation_type == "delete" && post_id == &pending.post_id
            }
            WireBoardSyncResponse::Error { error } => {
                let conflict = post_relay_error_is_conflict(error);
                self.fail_post_relay_request(pending, error, conflict);
                return;
            }
            _ => false,
        };
        if !matches {
            self.fail_post_relay_request(
                pending,
                "Relay returned a response that did not match the persisted mutation",
                false,
            );
            return;
        }
        let Some(posts_service) = self.posts_service.as_ref() else {
            return;
        };
        match posts_service.acknowledge_relay_outbox(
            &pending.event_id,
            &peer.to_string(),
            Utc::now().timestamp(),
        ) {
            Ok(true) => {
                let status = posts_service
                    .get_post(&pending.post_id)
                    .ok()
                    .flatten()
                    .map(|post| post.relay_status)
                    .unwrap_or_else(|| "relay_acknowledged".to_string());
                let _ = self
                    .event_tx
                    .try_send(NetworkEvent::PostRelayStatusChanged {
                        post_id: pending.post_id,
                        event_id: pending.event_id,
                        status,
                        error: None,
                    });
            }
            Ok(false) => debug!("Ignored duplicate post relay acknowledgement"),
            Err(error) => warn!("Could not persist post relay acknowledgement: {error}"),
        }
    }

    fn fail_pending_relay_auth(&mut self, peer: PeerId, error: &str) {
        if let Some(pending) = self.pending_introduction_submit.remove(&peer) {
            pending.pow_cancel.store(true, Ordering::Relaxed);
            let _ = pending
                .response_tx
                .send(NetworkResponse::Error(error.to_string()));
        }
        if let Some(pending) = self.pending_delivery_resolution.remove(&peer) {
            let _ = pending
                .response_tx
                .send(NetworkResponse::Error(error.to_string()));
        }
        if let Some(pending) = self.pending_introduction_fetch.remove(&peer) {
            let _ = pending
                .response_tx
                .send(NetworkResponse::Error(error.to_string()));
        }
        if let Some(pending) = self.pending_name_registration.remove(&peer) {
            let _ = pending
                .response_tx
                .send(NetworkResponse::Error(error.to_string()));
        }
    }

    async fn handle_board_sync_response(&mut self, peer: PeerId, response: WireBoardSyncResponse) {
        let Some(ref board_service) = self.board_service else {
            return;
        };
        let relay_peer_id = peer.to_string();

        match response {
            WireBoardSyncResponse::RelayAuthChallenge { challenge } => {
                let expected_audience = if self.pending_introduction_submit.contains_key(&peer)
                    || self.pending_delivery_resolution.contains_key(&peer)
                {
                    "introduce"
                } else if self.pending_introduction_fetch.contains_key(&peer) {
                    "introductions:read"
                } else if self.pending_name_registration.contains_key(&peer) {
                    "name:register"
                } else {
                    return;
                };
                let local_peer_id = self.identity_service.get_peer_id().unwrap_or_default();
                if let Err(error) = validate_relay_auth_challenge(
                    peer,
                    &local_peer_id,
                    expected_audience,
                    &challenge,
                    Utc::now().timestamp(),
                ) {
                    self.fail_pending_relay_auth(peer, error);
                    return;
                }
                if let Some(pending) = self.pending_name_registration.get_mut(&peer) {
                    pending.relay_public_key = challenge.relay_public_key.clone();
                }
                if let Some(pending) = self.pending_introduction_submit.get_mut(&peer) {
                    pending.relay_name = Some(challenge.relay.clone());
                    pending.relay_key_id = Some(challenge.key_id.clone());
                    pending.relay_public_key = challenge.relay_public_key.clone();
                }
                if let Some(pending) = self.pending_delivery_resolution.get_mut(&peer) {
                    pending.relay_name = Some(challenge.relay.clone());
                    pending.relay_key_id = Some(challenge.key_id.clone());
                    pending.relay_public_key = challenge.relay_public_key.clone();
                }
                let mut unsigned = challenge.clone();
                unsigned.relay_signature.clear();
                let mut bytes = Vec::new();
                if ciborium::ser::into_writer(&unsigned, &mut bytes).is_err() {
                    return;
                }
                let Ok(signature) = self.identity_service.sign_raw(&bytes) else {
                    return;
                };
                let Ok(keys) = self.identity_service.get_unlocked_keys() else {
                    return;
                };
                let Ok(lp) = libp2p::identity::ed25519::PublicKey::try_from_bytes(
                    &keys.ed25519_signing.verifying_key().to_bytes(),
                ) else {
                    return;
                };
                self.swarm.behaviour_mut().board_sync.send_request(
                    &peer,
                    WireBoardSyncRequest::RelayAuthComplete {
                        challenge,
                        public_key: libp2p::identity::PublicKey::from(lp).encode_protobuf(),
                        signature,
                    },
                );
            }
            WireBoardSyncResponse::RelaySession { token } => {
                if let Some(pending) = self.pending_introduction_submit.get_mut(&peer) {
                    pending.session_token = Some(token.clone());
                    self.swarm.behaviour_mut().board_sync.send_request(
                        &peer,
                        WireBoardSyncRequest::RequestIntroductionWork {
                            session_token: token,
                            target: pending.target.clone(),
                        },
                    );
                    return;
                }
                if let Some(pending) = self.pending_introduction_fetch.get_mut(&peer) {
                    pending.session_token = Some(token.clone());
                    self.swarm.behaviour_mut().board_sync.send_request(
                        &peer,
                        WireBoardSyncRequest::FetchIntroductions {
                            session_token: token,
                            limit: pending.limit,
                        },
                    );
                    return;
                }
                if let Some(pending) = self.pending_delivery_resolution.get(&peer) {
                    self.swarm.behaviour_mut().board_sync.send_request(
                        &peer,
                        WireBoardSyncRequest::RequestIntroductionWork {
                            session_token: token,
                            target: pending.target.clone(),
                        },
                    );
                    return;
                }
                let Some(pending) = self.pending_name_registration.get_mut(&peer) else {
                    return;
                };
                pending.session_token = Some(token.clone());
                let Ok(keys) = self.identity_service.get_unlocked_keys() else {
                    return;
                };
                let request = NameClaimRequest {
                    domain: "harbor/name-claim-request/1".into(),
                    version: 1,
                    local_name: pending.local_name.clone(),
                    relay: pending.namespace.clone(),
                    peer_id: self.identity_service.get_peer_id().unwrap_or_default(),
                    ed25519_public_key: keys.ed25519_signing.verifying_key().to_bytes().to_vec(),
                    x25519_public_key: x25519_dalek::PublicKey::from(&keys.x25519_secret)
                        .to_bytes()
                        .to_vec(),
                    sequence: 1,
                    issued_at: Utc::now().timestamp(),
                    nonce: uuid::Uuid::new_v4().as_bytes().to_vec(),
                };
                let mut bytes = Vec::new();
                if ciborium::ser::into_writer(&request, &mut bytes).is_err() {
                    return;
                };
                let Ok(user_signature) = self.identity_service.sign_raw(&bytes) else {
                    return;
                };
                self.swarm.behaviour_mut().board_sync.send_request(
                    &peer,
                    WireBoardSyncRequest::RegisterRelayName {
                        session_token: token,
                        signed_request: SignedNameClaimRequest {
                            request,
                            user_signature,
                        },
                    },
                );
            }
            WireBoardSyncResponse::RelayNameRegistered { claim } => {
                if let Some(p) = self.pending_name_registration.remove(&peer) {
                    let _ = p.response_tx.send(NetworkResponse::RelayNameClaim {
                        claim: Box::new(claim),
                        relay_public_key: p.relay_public_key,
                    });
                }
            }
            WireBoardSyncResponse::IntroductionWork { challenge } => {
                let local_peer_id = self.identity_service.get_peer_id().unwrap_or_default();
                if self.pending_delivery_resolution.contains_key(&peer) {
                    let validation = self
                        .pending_delivery_resolution
                        .get(&peer)
                        .ok_or("INTRODUCTION_WORK_INVALID")
                        .and_then(|pending| {
                            validate_work_challenge(
                                peer,
                                &local_peer_id,
                                &pending.target,
                                pending.relay_name.as_deref().unwrap_or_default(),
                                pending.relay_key_id.as_deref().unwrap_or_default(),
                                &pending.relay_public_key,
                                &challenge,
                                Utc::now().timestamp(),
                            )
                        });
                    let pending = self
                        .pending_delivery_resolution
                        .remove(&peer)
                        .expect("pending delivery resolution checked above");
                    let response = match validation {
                        Ok(()) => NetworkResponse::DeliveryKey {
                            target: pending.target,
                            key: challenge.delivery_key,
                            expires_at: challenge.expires_at,
                        },
                        Err(error) => NetworkResponse::Error(error.to_string()),
                    };
                    let _ = pending.response_tx.send(response);
                    return;
                }
                let Some(pending) = self.pending_introduction_submit.get_mut(&peer) else {
                    return;
                };
                let validation = validate_work_challenge(
                    peer,
                    &local_peer_id,
                    &pending.target,
                    pending.relay_name.as_deref().unwrap_or_default(),
                    pending.relay_key_id.as_deref().unwrap_or_default(),
                    &pending.relay_public_key,
                    &challenge,
                    Utc::now().timestamp(),
                );
                if let Err(error) = validation {
                    let pending = self
                        .pending_introduction_submit
                        .remove(&peer)
                        .expect("pending introduction checked above");
                    pending.pow_cancel.store(true, Ordering::Relaxed);
                    let _ = pending
                        .response_tx
                        .send(NetworkResponse::Error(error.to_string()));
                    return;
                }
                if pending.pow_challenge_id.is_some() {
                    return;
                }
                pending.pow_challenge_id = Some(challenge.id.clone());
                let cancelled = pending.pow_cancel.clone();
                let completion_tx = self.pow_result_tx.clone();
                let solve_challenge = challenge.clone();
                let remaining = (challenge.expires_at - Utc::now().timestamp()).max(0) as u64;
                let deadline =
                    Instant::now() + POW_MAX_DURATION.min(Duration::from_secs(remaining));
                tokio::spawn(async move {
                    let result = spawn_work_solver(solve_challenge, cancelled, deadline)
                        .await
                        .unwrap_or(Err(WorkSolveError::Cancelled));
                    let _ = completion_tx
                        .send(PowCompletion {
                            peer,
                            challenge,
                            result,
                        })
                        .await;
                });
            }
            WireBoardSyncResponse::IntroductionAccepted {
                request_id,
                retry_after,
            } => {
                if let Some(p) = self.pending_introduction_submit.remove(&peer) {
                    let _ = p.response_tx.send(NetworkResponse::IntroductionAccepted {
                        request_id,
                        retry_after,
                    });
                }
            }
            WireBoardSyncResponse::Introductions { envelopes } => {
                if let Some(pending) = self.pending_introduction_fetch.remove(&peer) {
                    let mut ack = Vec::new();
                    if let Some(service) = &self.mentions_service {
                        for e in &envelopes {
                            let incoming = IncomingMentionEnvelope {
                                request_id: e.request_id.clone(),
                                requester_peer_id: e.requester_peer_id.clone(),
                                ephemeral_public_key: e.requester_ephemeral_x25519_key.clone(),
                                ciphertext: e.message_ciphertext.clone(),
                                issued_at: e.issued_at,
                                expires_at: e.expires_at,
                            };
                            if should_ack_ingest(
                                &service.ingest_queued_envelope(&incoming, Utc::now().timestamp()),
                            ) {
                                ack.push(e.request_id.clone())
                            }
                        }
                    }
                    if !ack.is_empty() {
                        if let Some(token) = pending.session_token {
                            self.swarm.behaviour_mut().board_sync.send_request(
                                &peer,
                                WireBoardSyncRequest::AckIntroductions {
                                    session_token: token,
                                    request_ids: ack,
                                },
                            );
                        }
                    }
                    let _ = pending
                        .response_tx
                        .send(NetworkResponse::Introductions(envelopes));
                }
            }
            WireBoardSyncResponse::IntroductionsAcked { .. } => {}
            WireBoardSyncResponse::BoardList { boards, .. } => {
                let board_count = boards.len();
                let board_data: Vec<(String, String, Option<String>, bool)> = boards
                    .iter()
                    .map(|b| {
                        (
                            b.board_id.clone(),
                            b.name.clone(),
                            b.description.clone(),
                            b.is_default,
                        )
                    })
                    .collect();

                // Check if this is a response to a community probe
                let is_community_probe = self.pending_community_probes.contains_key(&peer);
                if is_community_probe {
                    let relay_addr = self
                        .pending_community_probes
                        .remove(&peer)
                        .unwrap_or_default();
                    info!(
                        "Community relay detected: {} ({} boards) - auto-joining",
                        peer, board_count
                    );

                    // Mark as community relay
                    self.community_relays.insert(peer, relay_addr.clone());

                    // Auto-join: store community locally
                    if let Err(e) = board_service.join_community(&relay_peer_id, &relay_addr, None)
                    {
                        warn!("Failed to auto-join community on {}: {}", peer, e);
                    }

                    // Note: RegisterPeer was already sent during the probe phase
                    // (before ListBoards), so no need to register again.

                    // Store boards from probe response
                    if let Err(e) = board_service.store_boards(&relay_peer_id, &board_data) {
                        warn!("Failed to store boards from {}: {}", peer, e);
                    }

                    // Emit auto-join event to frontend
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::CommunityAutoJoined {
                            relay_peer_id: relay_peer_id.clone(),
                            relay_address: relay_addr,
                            community_name: None,
                            board_count,
                        })
                        .await;

                    // Also emit the standard board list event
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::BoardListReceived {
                            relay_peer_id,
                            board_count,
                        })
                        .await;
                } else {
                    // Normal board list response (not a probe)
                    match board_service.store_boards(&relay_peer_id, &board_data) {
                        Ok(()) => {
                            let _ = self
                                .event_tx
                                .send(NetworkEvent::BoardListReceived {
                                    relay_peer_id,
                                    board_count,
                                })
                                .await;
                        }
                        Err(e) => {
                            warn!("Failed to store boards from {}: {}", peer, e);
                        }
                    }
                }
            }
            WireBoardSyncResponse::BoardPosts {
                board_id, posts, ..
            } => {
                let storable: Vec<StorableBoardPost> = posts
                    .iter()
                    .map(|p| StorableBoardPost {
                        post_id: p.post_id.clone(),
                        board_id: p.board_id.clone(),
                        author_peer_id: p.author_peer_id.clone(),
                        author_display_name: p.author_display_name.clone(),
                        content_type: p.content_type.clone(),
                        content_text: p.content_text.clone(),
                        lamport_clock: p.lamport_clock as i64,
                        created_at: p.created_at,
                        deleted_at: p.deleted_at,
                        signature: p.signature.clone(),
                    })
                    .collect();
                let post_count = storable.len();
                match board_service.store_board_posts(&relay_peer_id, &storable) {
                    Ok(()) => {
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::BoardPostsReceived {
                                relay_peer_id,
                                board_id,
                                post_count,
                            })
                            .await;
                    }
                    Err(e) => {
                        warn!("Failed to store board posts from {}: {}", peer, e);
                    }
                }
            }
            WireBoardSyncResponse::PostAccepted { post_id } => {
                info!("Board post {} accepted by relay {}", post_id, peer);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::BoardPostSubmitted {
                        relay_peer_id,
                        post_id,
                    })
                    .await;
            }
            WireBoardSyncResponse::PeerRegistered { peer_id } => {
                info!("Registered with relay {} as {}", peer, peer_id);

                // If we were waiting for registration to complete before listing boards,
                // send the ListBoards request now.
                if self.pending_board_registrations.remove(&peer) {
                    info!(
                        "Registration complete for {}, now requesting board list",
                        peer
                    );
                    if let Some(ref board_service) = self.board_service {
                        match board_service.create_list_boards_request() {
                            Ok(list_req) => {
                                let request = WireBoardSyncRequest::ListBoards {
                                    requester_peer_id: list_req.requester_peer_id,
                                    timestamp: list_req.timestamp,
                                    signature: list_req.signature,
                                };
                                self.swarm
                                    .behaviour_mut()
                                    .board_sync
                                    .send_request(&peer, request);
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to create list boards request after registration: {}",
                                    e
                                );
                            }
                        }
                    }
                }
            }
            WireBoardSyncResponse::PostDeleted { post_id } => {
                info!("Board post {} deleted on relay {}", post_id, peer);
            }
            WireBoardSyncResponse::WallPostStored { post_id } => {
                info!("Wall post {} stored on relay {}", post_id, peer);
                Self::emit_wall_sync_status(
                    self.event_tx.clone(),
                    "author_wall",
                    "success",
                    "posts_stored",
                    Some(relay_peer_id.clone()),
                    None,
                    Some(post_id.clone()),
                    None,
                    Some(1),
                    None,
                    None,
                )
                .await;
                let _ = self
                    .event_tx
                    .send(NetworkEvent::WallPostSynced {
                        relay_peer_id: relay_peer_id.clone(),
                        post_id,
                    })
                    .await;
            }
            WireBoardSyncResponse::WallPosts { posts, has_more } => {
                let post_count = posts.len();
                // Determine the author from the first post (all posts should be from same author)
                let author_peer_id = posts
                    .first()
                    .map(|p| p.author_peer_id.clone())
                    .unwrap_or_default();

                let total_media_items: usize = posts.iter().map(|p| p.media_items.len()).sum();
                info!(
                    "Received {} wall posts for author {} from relay {} (has_more: {}, media_items: {})",
                    post_count, author_peer_id, peer, has_more, total_media_items
                );
                Self::emit_wall_sync_status(
                    self.event_tx.clone(),
                    if author_peer_id.is_empty() {
                        "feed"
                    } else {
                        "contact_wall"
                    },
                    "in_progress",
                    "posts_received",
                    Some(relay_peer_id.clone()),
                    if author_peer_id.is_empty() {
                        None
                    } else {
                        Some(author_peer_id.clone())
                    },
                    None,
                    None,
                    Some(post_count),
                    None,
                    None,
                )
                .await;

                // Store received posts in local SQLite via content_sync_service. Advance the
                // relay cursor only after each post and signed media metadata are verified and
                // stored; stop on the first failure so a later page cannot skip a failed lamport.
                let mut advanced_relay_cursor: Option<u64> = None;
                let mut stored_all_relay_posts = true;
                if let Some(ref content_sync_service) = self.content_sync_service {
                    for post in &posts {
                        let signed_media_items: Vec<crate::services::SignedPostMediaMetadata> =
                            post.media_items
                                .iter()
                                .map(|m| crate::services::SignedPostMediaMetadata {
                                    media_hash: m.media_hash.clone(),
                                    media_type: m.media_type.clone(),
                                    mime_type: m.mime_type.clone(),
                                    file_name: m.file_name.clone(),
                                    file_size: m.file_size,
                                    width: m.width,
                                    height: m.height,
                                    duration_seconds: m.duration_seconds,
                                    sort_order: m.sort_order,
                                    signature: m.signature.clone(),
                                })
                                .collect();

                        let store_result = if let Some(deleted_at) = post.deleted_at {
                            content_sync_service.store_remote_post_delete(
                                &post.post_id,
                                &post.author_peer_id,
                                post.lamport_clock as u64,
                                deleted_at,
                                &post.signature,
                            )
                        } else {
                            content_sync_service.store_remote_post(&RemotePostParams {
                                post_id: &post.post_id,
                                author_peer_id: &post.author_peer_id,
                                content_type: &post.content_type,
                                content_text: post.content_text.as_deref(),
                                visibility: &post.visibility,
                                lamport_clock: post.lamport_clock as u64,
                                created_at: post.created_at,
                                signature: &post.signature,
                                media_hashes: &post.media_hashes,
                                media_items: &signed_media_items,
                            })
                        };

                        match store_result {
                            Ok(_) => {
                                let lamport_clock = post.lamport_clock as u64;
                                if let Err(e) = content_sync_service.store_author_sync_cursor(
                                    &relay_peer_id,
                                    &post.author_peer_id,
                                    lamport_clock,
                                ) {
                                    warn!(
                                        "Failed to advance relay wall cursor for {} via {}: {}",
                                        post.author_peer_id, relay_peer_id, e
                                    );
                                    stored_all_relay_posts = false;
                                    break;
                                }
                                advanced_relay_cursor = Some(lamport_clock);
                                Self::emit_wall_sync_status(
                                    self.event_tx.clone(),
                                    "contact_wall",
                                    "in_progress",
                                    "cursor_advanced",
                                    Some(relay_peer_id.clone()),
                                    Some(post.author_peer_id.clone()),
                                    Some(post.post_id.clone()),
                                    None,
                                    Some(1),
                                    Some(lamport_clock),
                                    None,
                                )
                                .await;
                                for media_hash in &post.media_hashes {
                                    Self::emit_wall_sync_status(
                                        self.event_tx.clone(),
                                        "contact_wall",
                                        "in_progress",
                                        "media_queued",
                                        Some(relay_peer_id.clone()),
                                        Some(post.author_peer_id.clone()),
                                        Some(post.post_id.clone()),
                                        Some(media_hash.clone()),
                                        None,
                                        None,
                                        None,
                                    )
                                    .await;
                                }
                                debug!(
                                    "Stored wall post {} from {} via relay",
                                    post.post_id, post.author_peer_id
                                );
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to store wall post {} from relay: {}",
                                    post.post_id, e
                                );
                                let error = e.to_string();
                                let phase = if error.to_ascii_lowercase().contains("permission") {
                                    "permission_denied"
                                } else {
                                    "posts_stored"
                                };
                                Self::emit_wall_sync_status(
                                    self.event_tx.clone(),
                                    "contact_wall",
                                    "partial_failure",
                                    phase,
                                    Some(relay_peer_id.clone()),
                                    Some(post.author_peer_id.clone()),
                                    Some(post.post_id.clone()),
                                    None,
                                    None,
                                    None,
                                    Some(error),
                                )
                                .await;
                                stored_all_relay_posts = false;
                                break;
                            }
                        }
                    }

                    if has_more && stored_all_relay_posts {
                        if let (Some(next_since), Some(next_author)) = (
                            advanced_relay_cursor,
                            posts.first().map(|p| p.author_peer_id.clone()),
                        ) {
                            match self.identity_service.get_identity() {
                                Ok(Some(identity)) => {
                                    let now = chrono::Utc::now().timestamp();
                                    let signable = SignableGetWallPosts {
                                        requester_peer_id: identity.peer_id.clone(),
                                        author_peer_id: next_author.clone(),
                                        since_lamport_clock: next_since as i64,
                                        limit: post_count as u32,
                                        timestamp: now,
                                    };
                                    match self.identity_service.sign(&signable) {
                                        Ok(signature) => {
                                            self.swarm.behaviour_mut().board_sync.send_request(
                                                &peer,
                                                WireBoardSyncRequest::GetWallPosts {
                                                    requester_peer_id: identity.peer_id,
                                                    author_peer_id: next_author,
                                                    since_lamport_clock: next_since as i64,
                                                    limit: post_count as u32,
                                                    timestamp: now,
                                                    signature,
                                                },
                                            );
                                        }
                                        Err(e) => {
                                            warn!("Failed to sign next wall page request: {}", e)
                                        }
                                    }
                                }
                                Ok(None) => warn!("Cannot request next wall page without identity"),
                                Err(e) => warn!("Cannot request next wall page: {}", e),
                            }
                        }
                    }
                } else {
                    warn!("Content sync service unavailable, cannot store wall posts from relay");
                    Self::emit_wall_sync_status(
                        self.event_tx.clone(),
                        "contact_wall",
                        "partial_failure",
                        "posts_stored",
                        Some(relay_peer_id.clone()),
                        if author_peer_id.is_empty() {
                            None
                        } else {
                            Some(author_peer_id.clone())
                        },
                        None,
                        None,
                        Some(post_count),
                        advanced_relay_cursor,
                        Some("Content sync service unavailable".to_string()),
                    )
                    .await;
                }

                if stored_all_relay_posts {
                    Self::emit_wall_sync_status(
                        self.event_tx.clone(),
                        if author_peer_id.is_empty() {
                            "feed"
                        } else {
                            "contact_wall"
                        },
                        "success",
                        "posts_stored",
                        Some(relay_peer_id.clone()),
                        if author_peer_id.is_empty() {
                            None
                        } else {
                            Some(author_peer_id.clone())
                        },
                        None,
                        None,
                        Some(post_count),
                        advanced_relay_cursor,
                        None,
                    )
                    .await;
                }

                // Emit event to refresh feed
                let _ = self
                    .event_tx
                    .send(NetworkEvent::WallPostsReceived {
                        relay_peer_id: relay_peer_id.clone(),
                        author_peer_id,
                        post_count,
                    })
                    .await;
            }
            WireBoardSyncResponse::WallPostDeleted { post_id } => {
                info!("Wall post {} deleted on relay {}", post_id, peer);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::WallPostDeletedOnRelay {
                        relay_peer_id: relay_peer_id.clone(),
                        post_id,
                    })
                    .await;
            }
            WireBoardSyncResponse::WallSocialEventStored { event_id } => {
                info!("Wall social event {} stored on relay {}", event_id, peer);
            }
            WireBoardSyncResponse::WallSocialEvents { events, .. } => {
                let count = events.len();
                if let Some(ref wall_social_service) = self.wall_social_service {
                    for event in events {
                        match crate::db::WallSocialEventType::parse_event_type(&event.event_type) {
                            Some(event_type) => {
                                if let Err(e) = wall_social_service.process_incoming_event(
                                    &IncomingWallSocialEventParams {
                                        event_id: &event.event_id,
                                        event_type,
                                        post_id: &event.post_id,
                                        actor_peer_id: &event.actor_peer_id,
                                        author_name: event.author_name.as_deref(),
                                        comment_id: event.comment_id.as_deref(),
                                        content: event.content.as_deref(),
                                        reaction_type: event.reaction_type.as_deref(),
                                        timestamp: event.timestamp,
                                        signature: &event.signature,
                                    },
                                ) {
                                    warn!(
                                        "Failed to apply wall social event {} from relay {}: {}",
                                        event.event_id, peer, e
                                    );
                                }
                            }
                            None => warn!(
                                "Ignoring unknown wall social event type {} from relay {}",
                                event.event_type, peer
                            ),
                        }
                    }
                } else {
                    warn!(
                        "Wall social service unavailable; cannot apply {} relay social events",
                        count
                    );
                }
                debug!("Received {} wall social events from relay {}", count, peer);
            }
            WireBoardSyncResponse::Error { error } => {
                if self.pending_introduction_submit.contains_key(&peer)
                    || self.pending_introduction_fetch.contains_key(&peer)
                    || self.pending_delivery_resolution.contains_key(&peer)
                    || self.pending_name_registration.contains_key(&peer)
                {
                    self.fail_pending_relay_auth(peer, &error);
                    return;
                }
                // If this was a community probe that failed (either RegisterPeer or
                // ListBoards), just clean up silently. Non-community relays will return
                // an error and that's expected.
                let was_probe = self.pending_community_probes.remove(&peer).is_some();
                let was_registration = self.pending_board_registrations.remove(&peer);
                if was_probe || was_registration {
                    debug!(
                        "Relay {} is not a community relay (probe returned error: {})",
                        peer, error
                    );
                } else {
                    warn!("Board sync error from {}: {}", peer, error);
                    let status_phase = if error.to_ascii_lowercase().contains("permission") {
                        "permission_denied"
                    } else {
                        "relay_unavailable"
                    };
                    Self::emit_wall_sync_status(
                        self.event_tx.clone(),
                        "feed",
                        "partial_failure",
                        status_phase,
                        Some(relay_peer_id.clone()),
                        None,
                        None,
                        None,
                        None,
                        None,
                        Some(error.clone()),
                    )
                    .await;
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::BoardSyncError {
                            relay_peer_id,
                            error,
                        })
                        .await;
                }
            }
        }
    }

    async fn handle_identity_request(
        &mut self,
        peer: PeerId,
        _request_id: request_response::InboundRequestId,
        request: IdentityExchangeRequest,
        channel: ResponseChannel<IdentityExchangeResponse>,
    ) {
        let now = chrono::Utc::now().timestamp();
        if !verify_identity_request(peer, &request, now) {
            warn!("Rejected invalid signed contact request from {peer}");
            return;
        }
        let Some(service) = self.contacts_service.clone() else {
            return;
        };

        let mut committed_response_grants = None;
        let status = match request.action.as_str() {
            "request" => {
                if let Err(error) = service.record_contact_request(
                    &request.request_id,
                    &request.requester_peer_id,
                    "incoming",
                    Some(&request.display_name),
                    Some(&request.public_key),
                    Some(&request.x25519_public),
                    request.avatar_hash.as_deref(),
                    request.bio.as_deref(),
                    "review",
                    None,
                    None,
                    request.timestamp,
                ) {
                    warn!("Failed to persist incoming contact request from {peer}: {error}");
                    return;
                }
                "review"
            }
            "accepted" => {
                let correlated_acceptance = service
                    .get_contact(&request.requester_peer_id)
                    .ok()
                    .flatten()
                    .is_some_and(|contact| !contact.is_blocked)
                    || service
                        .contact_request_for_peer(&request.requester_peer_id, "outgoing")
                        .ok()
                        .flatten()
                        .is_some_and(|existing| {
                            existing.request_id == request.request_id
                                && matches!(
                                    existing.status.as_str(),
                                    "pending" | "review" | "failed"
                                )
                        });
                if !correlated_acceptance {
                    warn!("Rejected uncorrelated contact acceptance from {peer}");
                    return;
                }
                if let Err(error) = self.validate_contact_acceptance_grants(
                    &request.requester_peer_id,
                    &request.public_key,
                    &request.permission_grants,
                ) {
                    warn!("Rejected contact acceptance grants from {peer}: {error}");
                    return;
                }
                let Some(permissions) = self.permissions_service.as_ref() else {
                    warn!("Permissions service unavailable during contact acceptance");
                    return;
                };
                let outgoing_grants = match permissions.prepare_permission_grants(
                    &request.requester_peer_id,
                    &[Capability::Chat, Capability::WallRead, Capability::Call],
                ) {
                    Ok(grants) => grants,
                    Err(error) => {
                        warn!("Could not prepare reciprocal contact grants for {peer}: {error}");
                        return;
                    }
                };
                let mut all_grants = request.permission_grants.clone();
                all_grants.extend(outgoing_grants.iter().cloned());
                let contact = crate::db::ContactData {
                    peer_id: request.requester_peer_id.clone(),
                    public_key: request.public_key.clone(),
                    x25519_public: request.x25519_public.clone(),
                    display_name: request.display_name.clone(),
                    avatar_hash: None,
                    bio: request.bio.clone(),
                };
                let commit = service.accept_contact_request_atomically(
                    &request.request_id,
                    "outgoing",
                    &contact,
                    &all_grants,
                    now,
                );
                if let Err(error) = commit {
                    warn!("Failed atomic contact acceptance from {peer}: {error}");
                    return;
                }
                if request.profile_revision > 0 {
                    if let Err(error) = self.stage_signed_contact_profile(
                        peer,
                        &request.requester_peer_id,
                        request.profile_revision,
                        &request.display_name,
                        request.avatar_hash.as_deref(),
                        request.avatar_mime_type.as_deref(),
                        request.bio.as_deref(),
                    ) {
                        warn!("Could not stage accepted contact profile from {peer}: {error}");
                    }
                }
                committed_response_grants = Some(outgoing_grants);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::ContactAdded {
                        peer_id: request.requester_peer_id.clone(),
                        display_name: request.display_name.clone(),
                    })
                    .await;
                "accepted"
            }
            "declined" => {
                if let Ok(Some(existing)) =
                    service.contact_request_for_peer(&request.requester_peer_id, "outgoing")
                {
                    if existing.request_id == request.request_id {
                        let _ = service.update_contact_request(
                            &existing.request_id,
                            "declined",
                            None,
                            None,
                            now,
                        );
                    }
                }
                "declined"
            }
            "revoked" => {
                let Some(permissions) = self.permissions_service.as_ref() else {
                    return;
                };
                // The already-verified identity envelope binds this key to the
                // transport peer. Do not depend on the contact row here: a
                // duplicate delivery after a lost acknowledgement arrives
                // after that row has intentionally been removed.
                if request.permission_revocations.iter().any(|revoke| {
                    permissions
                        .validate_incoming_revoke(revoke, &request.public_key)
                        .is_err()
                }) {
                    warn!("Rejected invalid signed contact revocation from {peer}");
                    return;
                }
                if let Err(error) = service.apply_incoming_contact_revocation_atomically(
                    &request.requester_peer_id,
                    &request.permission_revocations,
                    now,
                ) {
                    warn!("Failed atomic incoming contact revocation from {peer}: {error}");
                    return;
                }
                "revoked"
            }
            "profile" => {
                if let Err(error) = self.stage_signed_contact_profile(
                    peer,
                    &request.requester_peer_id,
                    request.profile_revision,
                    &request.display_name,
                    request.avatar_hash.as_deref(),
                    request.avatar_mime_type.as_deref(),
                    request.bio.as_deref(),
                ) {
                    warn!("Rejected profile revision from {peer}: {error}");
                    return;
                }
                "profile_updated"
            }
            _ => return,
        };

        if let Ok(Some(stored)) = service.contact_request_for_peer(
            &request.requester_peer_id,
            if request.action == "request" {
                "incoming"
            } else {
                "outgoing"
            },
        ) {
            let _ = self
                .event_tx
                .send(NetworkEvent::ContactRequestChanged {
                    request_id: stored.request_id,
                    peer_id: stored.peer_id,
                    display_name: stored.display_name,
                    direction: stored.direction,
                    status: stored.status,
                })
                .await;
        }

        match self.create_identity_response(
            request.request_id,
            status.into(),
            &request.requester_peer_id,
            committed_response_grants,
        ) {
            Ok(response) => {
                if let Err(error) = self
                    .swarm
                    .behaviour_mut()
                    .identity_exchange
                    .send_response(channel, response)
                {
                    warn!("Failed to send contact-request response: {error:?}");
                }
            }
            Err(error) => warn!("Failed to create contact-request response: {error}"),
        }
        if status == "revoked" {
            let _ = self.swarm.disconnect_peer_id(peer);
        }
    }

    async fn handle_identity_response(
        &mut self,
        peer: PeerId,
        outbound_id: request_response::OutboundRequestId,
        response: IdentityExchangeResponse,
    ) {
        let Some((request_id, action)) = self.pending_identity_requests.remove(&outbound_id) else {
            warn!("Ignoring uncorrelated identity response from {peer}");
            return;
        };
        if response.request_id != request_id {
            if action == "revoked" {
                if let Some(service) = &self.contacts_service {
                    let _ = service.retry_contact_revocation(
                        &request_id,
                        "revocation acknowledgement request ID mismatch",
                        chrono::Utc::now().timestamp(),
                    );
                }
            }
            warn!("Ignoring identity response with mismatched request ID from {peer}");
            return;
        }
        let response_now = chrono::Utc::now().timestamp();
        if !verify_identity_response(peer, &response, response_now) {
            if action == "revoked" {
                if let Some(service) = &self.contacts_service {
                    let _ = service.retry_contact_revocation(
                        &request_id,
                        "invalid or forged revocation acknowledgement",
                        response_now,
                    );
                }
                let _ = self.swarm.disconnect_peer_id(peer);
            }
            warn!("Ignoring invalid signed identity response from {peer}");
            return;
        }
        if action == "revoked" && response.status != "revoked" {
            if let Some(service) = &self.contacts_service {
                let _ = service.retry_contact_revocation(
                    &request_id,
                    "peer did not acknowledge revocation",
                    response_now,
                );
            }
            let _ = self.swarm.disconnect_peer_id(peer);
            return;
        }
        info!(
            "Got identity from {}: {} ({})",
            peer, response.display_name, response.peer_id
        );

        // Store in contacts database if we have the contacts service
        if let Some(contacts_service) = self.contacts_service.clone() {
            let now = chrono::Utc::now().timestamp();
            if !matches!(
                response.status.as_str(),
                "review" | "accepted" | "declined" | "revoked" | "failed" | "profile_updated"
            ) || response.timestamp > now + 30
                || now - response.timestamp > 300
            {
                warn!("Rejected stale or invalid contact-request response from {peer}");
                return;
            }
            // Verify the response peer ID matches the peer we received from
            if response.peer_id != peer.to_string() {
                warn!(
                    "Identity response peer ID mismatch: expected {}, got {}",
                    peer, response.peer_id
                );
                return;
            }

            // Step 1: Parse the Ed25519 public key from the response.
            let public_key_bytes: [u8; 32] = match response.public_key.clone().try_into() {
                Ok(bytes) => bytes,
                Err(_) => {
                    warn!(
                        "Identity response from {} has invalid public key length (expected 32, got {})",
                        peer,
                        response.public_key.len()
                    );
                    return;
                }
            };

            let verifying_key = match ed25519_dalek::VerifyingKey::from_bytes(&public_key_bytes) {
                Ok(key) => key,
                Err(error) => {
                    warn!(
                        "Identity response from {} has invalid Ed25519 public key: {}",
                        peer, error
                    );
                    return;
                }
            };

            // Step 2: Verify that the Ed25519 public key actually derives
            // the claimed peer ID. Without this check, an attacker could
            // include an arbitrary public key and sign the payload with
            // the corresponding private key while claiming someone else's
            // peer ID. The transport-level peer ID check (above) mitigates
            // this for direct connections, but this provides defense-in-depth.
            let derived_peer_id =
                match crate::services::CryptoService::derive_peer_id_from_verifying_key(
                    &verifying_key,
                ) {
                    Ok(id) => id,
                    Err(e) => {
                        warn!(
                        "Identity response from {}: failed to derive peer ID from public key: {}",
                        peer, e
                    );
                        return;
                    }
                };
            if derived_peer_id != response.peer_id {
                warn!(
                    "Identity response from {}: public key derives peer ID {} but response claims {} - rejecting identity",
                    peer, derived_peer_id, response.peer_id
                );
                return;
            }

            // Step 3: Verify the entire identity response, including its status
            // and any capability grants, against the bound Ed25519 public key.
            let signature_is_valid = {
                let signature = match ed25519_dalek::Signature::from_slice(&response.signature) {
                    Ok(sig) => sig,
                    Err(error) => {
                        warn!(
                            "Identity response from {} has invalid signature format: {}",
                            peer, error
                        );
                        return;
                    }
                };

                use ed25519_dalek::Verifier;
                verifying_key
                    .verify(
                        &match identity_response_signing_bytes(&response) {
                            Ok(bytes) => bytes,
                            Err(error) => {
                                warn!("Could not encode identity response from {peer}: {error}");
                                return;
                            }
                        },
                        &signature,
                    )
                    .is_ok()
            };

            if !signature_is_valid {
                warn!(
                    "Identity response from {} failed signature verification - rejecting identity",
                    peer
                );
                return;
            }

            if action == "revoked" {
                if let Err(error) =
                    contacts_service.mark_contact_revocation_delivered(&request_id, now)
                {
                    warn!("Could not acknowledge durable contact revocation: {error}");
                    return;
                }
                let _ = self.swarm.disconnect_peer_id(peer);
            }

            info!(
                "Identity response from {} passed all verification: peer ID binding and signature",
                peer
            );

            match response.status.as_str() {
                "review" if action == "request" => {
                    if let Err(error) = contacts_service.record_contact_request(
                        &request_id,
                        &response.peer_id,
                        "outgoing",
                        Some(&response.display_name),
                        Some(&response.public_key),
                        Some(&response.x25519_public),
                        response.avatar_hash.as_deref(),
                        response.bio.as_deref(),
                        "pending",
                        None,
                        None,
                        now,
                    ) {
                        warn!("Failed to persist contact request response from {peer}: {error}");
                        return;
                    }
                }
                "accepted" => {
                    if let Err(error) = self.validate_contact_acceptance_grants(
                        &response.peer_id,
                        &response.public_key,
                        &response.permission_grants,
                    ) {
                        warn!("Rejected contact acceptance grants from {peer}: {error}");
                        return;
                    }
                    let contact = crate::db::ContactData {
                        peer_id: response.peer_id.clone(),
                        public_key: response.public_key.clone(),
                        x25519_public: response.x25519_public.clone(),
                        display_name: response.display_name.clone(),
                        avatar_hash: None,
                        bio: response.bio.clone(),
                    };
                    let commit = match contacts_service.contact_request(&request_id) {
                        Ok(Some(stored)) => contacts_service.accept_contact_request_atomically(
                            &request_id,
                            &stored.direction,
                            &contact,
                            &response.permission_grants,
                            now,
                        ),
                        Ok(None)
                            if contacts_service
                                .is_contact(&response.peer_id)
                                .unwrap_or(false) =>
                        {
                            contacts_service.add_contact_with_grants_atomically(
                                &contact,
                                &response.permission_grants,
                                now,
                            )
                        }
                        Ok(None) => Err(AppError::Validation(
                            "Uncorrelated accepted contact response".into(),
                        )),
                        Err(error) => Err(error),
                    };
                    if let Err(error) = commit {
                        warn!("Failed atomic contact response from {peer}: {error}");
                        return;
                    }
                    if response.profile_revision > 0 {
                        if let Err(error) = self.stage_signed_contact_profile(
                            peer,
                            &response.peer_id,
                            response.profile_revision,
                            &response.display_name,
                            response.avatar_hash.as_deref(),
                            response.avatar_mime_type.as_deref(),
                            response.bio.as_deref(),
                        ) {
                            warn!("Could not stage accepted contact profile from {peer}: {error}");
                        }
                    }
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::ContactAdded {
                            peer_id: response.peer_id.clone(),
                            display_name: response.display_name.clone(),
                        })
                        .await;
                }
                "declined" => {
                    let _ = contacts_service.update_contact_request(
                        &request_id,
                        "declined",
                        None,
                        None,
                        now,
                    );
                }
                "revoked" => {
                    // The durable local teardown preceded this acknowledgement.
                }
                "profile_updated" if action == "profile" => {}
                status => warn!("Unexpected contact-request response status: {status}"),
            }
            if let Ok(Some(stored)) = contacts_service.contact_request(&request_id) {
                let _ = self
                    .event_tx
                    .send(NetworkEvent::ContactRequestChanged {
                        request_id: stored.request_id,
                        peer_id: stored.peer_id,
                        display_name: stored.display_name,
                        direction: stored.direction,
                        status: stored.status,
                    })
                    .await;
            }
        } else {
            warn!("No contacts service configured, cannot store identity");
        }
    }

    async fn handle_messaging_request(
        &mut self,
        peer: PeerId,
        _request_id: request_response::InboundRequestId,
        request: MessagingRequest,
        channel: ResponseChannel<MessagingResponse>,
    ) {
        // Decode the message payload
        let msg_result = MessagingCodec::decode(&request.payload);

        let (success, message_id, error) = match msg_result {
            Ok(MessagingMessage::MessageV2(direct_msg)) => {
                info!(
                    "Received direct message {} from {}",
                    direct_msg.message_id, peer
                );

                if let Err(error) = direct_msg.validate_shape() {
                    (false, Some(direct_msg.message_id), Some(error.to_string()))
                } else if !messaging_sender_matches_transport(&peer, &direct_msg.sender_peer_id) {
                    (
                        false,
                        Some(direct_msg.message_id),
                        Some("Message sender does not match transport peer".to_string()),
                    )
                // Process the message if we have a messaging service
                } else if let Some(ref messaging_service) = self.messaging_service {
                    match messaging_service.process_incoming_message(&IncomingMessageParams {
                        protocol_version: direct_msg.protocol_version,
                        message_id: &direct_msg.message_id,
                        event_id: &direct_msg.event_id,
                        conversation_id: &direct_msg.conversation_id,
                        sender_peer_id: &direct_msg.sender_peer_id,
                        recipient_peer_id: &direct_msg.recipient_peer_id,
                        nonce_id: &direct_msg.nonce_id,
                        content_encrypted: &direct_msg.content_encrypted,
                        content_type: &direct_msg.content_type,
                        reply_to: direct_msg.reply_to.as_deref(),
                        nonce_counter: direct_msg.nonce_counter,
                        lamport_clock: direct_msg.lamport_clock,
                        timestamp: direct_msg.timestamp,
                        signature: &direct_msg.signature,
                    }) {
                        Ok(_) => {
                            info!("Message {} processed successfully", direct_msg.message_id);
                            (true, Some(direct_msg.message_id.clone()), None)
                        }
                        Err(e) => {
                            warn!("Failed to process message {}: {}", direct_msg.message_id, e);
                            (
                                false,
                                Some(direct_msg.message_id.clone()),
                                Some(e.to_string()),
                            )
                        }
                    }
                } else {
                    warn!("No messaging service configured, cannot process message");
                    (
                        false,
                        Some(direct_msg.message_id),
                        Some("Messaging service not available".to_string()),
                    )
                }
            }
            Ok(MessagingMessage::Ack(ack)) => {
                info!("Received message ack for {} from {}", ack.message_id, peer);

                // Convert AckStatus to string for the messaging service
                let status_str = match ack.status {
                    super::protocols::messaging::AckStatus::Delivered => "delivered",
                    super::protocols::messaging::AckStatus::Read => "read",
                };

                // Process acknowledgment (update message status in database)
                if let Some(ref messaging_service) = self.messaging_service {
                    match messaging_service.process_incoming_ack(
                        &ack.message_id,
                        &ack.conversation_id,
                        &ack.peer_id,
                        status_str,
                        ack.timestamp,
                        &ack.signature,
                    ) {
                        Ok(_) => {
                            info!(
                                "Message ack processed: {} is now {}",
                                ack.message_id, status_str
                            );

                            // Emit event for the frontend to update UI
                            let _ = self
                                .event_tx
                                .send(NetworkEvent::MessageAckReceived {
                                    message_id: ack.message_id.clone(),
                                    conversation_id: ack.conversation_id.clone(),
                                    status: status_str.to_string(),
                                    timestamp: ack.timestamp,
                                })
                                .await;

                            (true, Some(ack.message_id), None)
                        }
                        Err(e) => {
                            warn!("Failed to process ack for {}: {}", ack.message_id, e);
                            (false, Some(ack.message_id), Some(e.to_string()))
                        }
                    }
                } else {
                    warn!("No messaging service configured, cannot process ack");
                    (
                        false,
                        Some(ack.message_id),
                        Some("Messaging service not available".to_string()),
                    )
                }
            }
            Ok(MessagingMessage::EditV2(edit)) => {
                info!(
                    "Received edit for message {} from {} at {}",
                    edit.message_id, peer, edit.authored_at
                );

                if let Err(error) = edit.validate_shape() {
                    (false, Some(edit.message_id), Some(error.to_string()))
                } else if !messaging_sender_matches_transport(&peer, &edit.author_peer_id) {
                    (
                        false,
                        Some(edit.message_id),
                        Some("Message-edit author does not match transport peer".to_string()),
                    )
                } else if let Some(ref messaging_service) = self.messaging_service {
                    match messaging_service.apply_incoming_edit(&IncomingMessageEditParams {
                        protocol_version: edit.protocol_version,
                        event_id: &edit.event_id,
                        message_id: &edit.message_id,
                        conversation_id: &edit.conversation_id,
                        author_peer_id: &edit.author_peer_id,
                        recipient_peer_id: &edit.recipient_peer_id,
                        revision: edit.revision,
                        nonce_id: &edit.nonce_id,
                        content_encrypted: &edit.content_encrypted,
                        nonce_counter: edit.nonce_counter,
                        lamport_clock: edit.lamport_clock,
                        authored_at: edit.authored_at,
                        signature: &edit.signature,
                    }) {
                        Ok(()) => {
                            info!("Successfully applied edit for message {}", edit.message_id);
                            (true, Some(edit.message_id), None)
                        }
                        Err(e) => {
                            warn!(
                                "Failed to apply edit for message {}: {}",
                                edit.message_id, e
                            );
                            (false, Some(edit.message_id), Some(e.to_string()))
                        }
                    }
                } else {
                    warn!("No messaging service configured, cannot process edit");
                    (
                        false,
                        Some(edit.message_id),
                        Some("Messaging service not available".to_string()),
                    )
                }
            }
            Err(e) => {
                warn!("Failed to decode messaging payload: {}", e);
                (false, None, Some(format!("Failed to decode: {}", e)))
            }
        };

        // Send response
        let response = MessagingResponse {
            success,
            message_id,
            error,
        };

        if let Err(e) = self
            .swarm
            .behaviour_mut()
            .messaging
            .send_response(channel, response)
        {
            warn!("Failed to send messaging response: {:?}", e);
        }

        // Emit event for the application layer (for UI updates)
        let _ = self
            .event_tx
            .send(NetworkEvent::MessageReceived {
                peer_id: peer.to_string(),
                protocol: "messaging".to_string(),
                payload: request.payload,
            })
            .await;
    }

    async fn handle_command(&mut self, command: NetworkCommand) -> NetworkResponse {
        match command {
            NetworkCommand::ResolveDeliveryKey {
                relay_peer_id,
                target,
                response_tx,
            } => {
                if self
                    .pending_delivery_resolution
                    .contains_key(&relay_peer_id)
                {
                    let _ = response_tx.send(NetworkResponse::Error(
                        "DELIVERY_KEY_RESOLUTION_IN_PROGRESS".into(),
                    ));
                    return NetworkResponse::Ok;
                }
                let peer_id = self.identity_service.get_peer_id().unwrap_or_default();
                self.pending_delivery_resolution.insert(
                    relay_peer_id,
                    PendingDeliveryResolution {
                        target,
                        relay_name: None,
                        relay_key_id: None,
                        relay_public_key: Vec::new(),
                        response_tx,
                    },
                );
                self.swarm.behaviour_mut().board_sync.send_request(
                    &relay_peer_id,
                    WireBoardSyncRequest::RelayAuthChallenge {
                        peer_id,
                        audience: "introduce".into(),
                    },
                );
                NetworkResponse::Ok
            }
            NetworkCommand::GetActiveRelay => self
                .community_relays
                .keys()
                .next()
                .copied()
                .or_else(|| {
                    PUBLIC_RELAYS
                        .iter()
                        .filter_map(|a| a.rsplit("/p2p/").next()?.parse().ok())
                        .find(|p| self.connected_peers.contains_key(p))
                })
                .map(NetworkResponse::ActiveRelay)
                .unwrap_or_else(|| NetworkResponse::Error("NO_ACTIVE_RELAY".into())),
            NetworkCommand::FetchIntroductions {
                relay_peer_id,
                limit,
                response_tx,
            } => {
                if self.pending_introduction_fetch.contains_key(&relay_peer_id) {
                    let _ = response_tx.send(NetworkResponse::Error(
                        "INTRODUCTION_FETCH_IN_PROGRESS".into(),
                    ));
                    return NetworkResponse::Ok;
                }
                let peer_id = self.identity_service.get_peer_id().unwrap_or_default();
                self.pending_introduction_fetch.insert(
                    relay_peer_id,
                    PendingIntroductionFetch {
                        limit: limit.clamp(1, 100),
                        session_token: None,
                        response_tx,
                    },
                );
                self.swarm.behaviour_mut().board_sync.send_request(
                    &relay_peer_id,
                    WireBoardSyncRequest::RelayAuthChallenge {
                        peer_id,
                        audience: "introductions:read".into(),
                    },
                );
                NetworkResponse::Ok
            }
            NetworkCommand::SubmitIntroduction {
                relay_peer_id,
                target,
                request_id,
                ephemeral_public_key,
                ciphertext,
                expires_at,
                response_tx,
            } => {
                if self
                    .pending_introduction_submit
                    .contains_key(&relay_peer_id)
                {
                    let _ =
                        response_tx.send(NetworkResponse::Error("INTRODUCTION_IN_PROGRESS".into()));
                    return NetworkResponse::Ok;
                }
                let peer_id = self.identity_service.get_peer_id().unwrap_or_default();
                self.pending_introduction_submit.insert(
                    relay_peer_id,
                    PendingIntroductionSubmit {
                        target,
                        request_id,
                        ephemeral_public_key,
                        ciphertext,
                        expires_at,
                        session_token: None,
                        relay_name: None,
                        relay_key_id: None,
                        relay_public_key: Vec::new(),
                        pow_challenge_id: None,
                        pow_cancel: Arc::new(AtomicBool::new(false)),
                        response_tx,
                    },
                );
                self.swarm.behaviour_mut().board_sync.send_request(
                    &relay_peer_id,
                    WireBoardSyncRequest::RelayAuthChallenge {
                        peer_id,
                        audience: "introduce".into(),
                    },
                );
                NetworkResponse::Ok
            }
            NetworkCommand::RegisterRelayName {
                relay_peer_id,
                local_name,
                namespace,
                response_tx,
            } => {
                // A timed-out caller drops its receiver. Remove that abandoned operation so a
                // user retry cannot be trapped behind NAME_REGISTRATION_IN_PROGRESS forever.
                if self
                    .pending_name_registration
                    .get(&relay_peer_id)
                    .is_some_and(|pending| pending.response_tx.is_closed())
                {
                    self.pending_name_registration.remove(&relay_peer_id);
                }
                if self.pending_name_registration.contains_key(&relay_peer_id) {
                    let _ = response_tx.send(NetworkResponse::Error(
                        "NAME_REGISTRATION_IN_PROGRESS".into(),
                    ));
                    return NetworkResponse::Ok;
                }
                let peer_id = self.identity_service.get_peer_id().unwrap_or_default();
                self.pending_name_registration.insert(
                    relay_peer_id,
                    PendingNameRegistration {
                        local_name,
                        namespace,
                        session_token: None,
                        relay_public_key: Vec::new(),
                        response_tx,
                    },
                );
                self.swarm.behaviour_mut().board_sync.send_request(
                    &relay_peer_id,
                    WireBoardSyncRequest::RelayAuthChallenge {
                        peer_id,
                        audience: "name:register".into(),
                    },
                );
                NetworkResponse::Ok
            }
            NetworkCommand::Dial { peer_id, addresses } => {
                for addr in addresses {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr.clone());
                }
                match self.swarm.dial(peer_id) {
                    Ok(_) => NetworkResponse::Ok,
                    Err(e) => NetworkResponse::Error(format!("Failed to dial: {}", e)),
                }
            }

            NetworkCommand::Disconnect { peer_id } => {
                let failures = self.pending_messaging_requests.fail_peer(
                    &peer_id,
                    MessageDeliveryFailureKind::Cancelled,
                    "peer was explicitly disconnected",
                );
                self.apply_message_attempt_failures(failures);
                match self.swarm.disconnect_peer_id(peer_id) {
                    Ok(_) => NetworkResponse::Ok,
                    Err(e) => NetworkResponse::Error(format!("Failed to disconnect: {:?}", e)),
                }
            }

            NetworkCommand::SendMessage {
                peer_id,
                protocol,
                event_id,
                message_id,
                payload,
                response_tx,
            } => {
                self.enqueue_message_attempt(
                    peer_id,
                    protocol,
                    event_id,
                    message_id,
                    payload,
                    Some(response_tx),
                );
                NetworkResponse::Ok
            }

            NetworkCommand::SendSignaling {
                peer_id,
                envelope,
                response_tx,
            } => {
                if !self.connected_peers.contains_key(&peer_id) {
                    let _ = response_tx.send(NetworkResponse::Error(
                        "SIGNALING_PEER_OFFLINE: peer is not connected".to_string(),
                    ));
                    return NetworkResponse::Ok;
                }

                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .signaling
                    .send_request(&peer_id, envelope);
                self.pending_signaling_requests
                    .insert(request_id, response_tx);
                NetworkResponse::Ok
            }

            NetworkCommand::RequestIdentity {
                peer_id,
                request_id,
                action,
                permission_grants,
                permission_revocations,
            } => {
                // Create identity request
                match self.create_identity_request(
                    request_id.clone(),
                    action.clone(),
                    &peer_id.to_string(),
                    permission_grants,
                    permission_revocations,
                ) {
                    Ok(request) => {
                        let outbound_id = self
                            .swarm
                            .behaviour_mut()
                            .identity_exchange
                            .send_request(&peer_id, request);
                        self.pending_identity_requests
                            .insert(outbound_id, (request_id, action));
                        NetworkResponse::IdentityQueued {
                            connected: self.connected_peers.contains_key(&peer_id),
                        }
                    }
                    Err(e) => {
                        NetworkResponse::Error(format!("Failed to create identity request: {}", e))
                    }
                }
            }

            NetworkCommand::GetStats => {
                let mut stats = self.stats.clone();
                stats.uptime_seconds = self.start_time.elapsed().as_secs();
                stats.nat_status = self.nat_status;
                stats.relay_addresses =
                    self.relay_addresses.iter().map(|a| a.to_string()).collect();
                stats.external_addresses = self
                    .external_addresses
                    .iter()
                    .map(|a| a.to_string())
                    .collect();
                NetworkResponse::Stats(stats)
            }

            NetworkCommand::GetConnectedPeers => {
                let peers: Vec<PeerInfo> = self.connected_peers.values().cloned().collect();
                NetworkResponse::Peers(peers)
            }

            NetworkCommand::GetListeningAddresses => {
                let local_peer_id = self.swarm.local_peer_id();
                let mut addresses: Vec<String> = Vec::new();

                // Add relay addresses first (most important for remote connections)
                for addr in &self.relay_addresses {
                    addresses.push(addr.to_string());
                }

                // Add external addresses discovered via AutoNAT
                for addr in &self.external_addresses {
                    addresses.push(format!("{}/p2p/{}", addr, local_peer_id));
                }

                // Add local listening addresses
                for addr in &self.listening_addresses {
                    addresses.push(format!("{}/p2p/{}", addr, local_peer_id));
                }

                NetworkResponse::Addresses(addresses)
            }

            NetworkCommand::AddBootstrapNode { address } => {
                // Parse the multiaddress to extract peer ID if present
                if let Some(peer_id) = address.iter().find_map(|proto| {
                    if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                        Some(peer_id)
                    } else {
                        None
                    }
                }) {
                    // Add to Kademlia routing table
                    let addr_without_peer: Multiaddr = address
                        .iter()
                        .filter(|p| !matches!(p, libp2p::multiaddr::Protocol::P2p(_)))
                        .collect();
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr_without_peer);
                    info!("Added bootstrap node: {} at {}", peer_id, address);

                    // Try to dial the bootstrap node
                    match self.swarm.dial(address.clone()) {
                        Ok(_) => {
                            info!("Dialing bootstrap node: {}", address);
                            NetworkResponse::Ok
                        }
                        Err(e) => {
                            NetworkResponse::Error(format!("Failed to dial bootstrap node: {}", e))
                        }
                    }
                } else {
                    NetworkResponse::Error(
                        "Multiaddress must contain peer ID (/p2p/...)".to_string(),
                    )
                }
            }

            NetworkCommand::Bootstrap => {
                if let Err(e) = self.swarm.behaviour_mut().kademlia.bootstrap() {
                    NetworkResponse::Error(format!("Bootstrap failed: {:?}", e))
                } else {
                    NetworkResponse::Ok
                }
            }

            NetworkCommand::AddRelayServer { address } => {
                // Parse the multiaddress to extract peer ID if present
                if let Some(relay_peer_id) = address.iter().find_map(|proto| {
                    if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                        Some(peer_id)
                    } else {
                        None
                    }
                }) {
                    // Extract transport-only address (without /p2p/...)
                    let addr_without_peer: Multiaddr = address
                        .iter()
                        .filter(|p| !matches!(p, libp2p::multiaddr::Protocol::P2p(_)))
                        .collect();

                    if addr_without_peer.is_empty() {
                        // No transport components (e.g. address is just /p2p/<peer_id>), so skip Kademlia
                        info!(
                            "Relay server {} has no non-P2p components in address {}; skipping Kademlia add_address",
                            relay_peer_id, address
                        );
                    } else {
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&relay_peer_id, addr_without_peer.clone());
                        info!("Added relay server: {} at {}", relay_peer_id, address);
                    }

                    // Try to dial the relay server using the full multiaddr (including /p2p)
                    match self.swarm.dial(address.clone()) {
                        Ok(_) => {
                            info!("Dialing relay server: {}", address);
                        }
                        Err(e) => {
                            return NetworkResponse::Error(format!(
                                "Failed to dial relay server: {}",
                                e
                            ));
                        }
                    }

                    // Queue relay reservation for after Identify completes.
                    // listen_on must be called AFTER the connection is fully negotiated
                    // (Identify::Received), not immediately after dial.
                    self.pending_relay_reservations
                        .insert(relay_peer_id, address.clone());
                    info!(
                        "Relay reservation queued for {} (will request after identify)",
                        relay_peer_id
                    );

                    NetworkResponse::Ok
                } else {
                    NetworkResponse::Error(
                        "Relay address must contain peer ID (/p2p/...)".to_string(),
                    )
                }
            }

            NetworkCommand::ConnectToPublicRelays => {
                // Reset the flag to allow reconnection and actually connect
                self.relay_connection_attempted = false;
                info!("Manually triggering connection to public relay servers...");
                self.connect_to_relays().await;
                NetworkResponse::Ok
            }

            NetworkCommand::SyncFeed { limit } => {
                // Clamp the limit to avoid pathological or abusive requests.
                const MAX_MANIFEST_LIMIT: u32 = 1000;
                let clamped_limit = limit.min(MAX_MANIFEST_LIMIT);

                let Some(ref content_sync_service) = self.content_sync_service else {
                    return NetworkResponse::Error("Content sync service unavailable".to_string());
                };

                // Avoid borrow issues: collect peer ids first.
                let connected_peer_ids: Vec<PeerId> =
                    self.connected_peers.keys().cloned().collect();

                // Request manifest from each connected peer (excluding ourselves, if present).
                for peer_id in connected_peer_ids {
                    let peer_id_string = peer_id.to_string();
                    let cursor = match content_sync_service.get_sync_cursor(&peer_id_string) {
                        Ok(cursor_value) => cursor_value,
                        Err(error) => {
                            warn!("Failed to load sync cursor for {}: {}", peer_id, error);
                            HashMap::new()
                        }
                    };

                    let manifest_request =
                        match content_sync_service.create_manifest_request(cursor, clamped_limit) {
                            Ok(request_value) => request_value,
                            Err(error) => {
                                warn!(
                                    "Failed to create manifest request for {}: {}",
                                    peer_id, error
                                );
                                continue;
                            }
                        };

                    let wire_message = ContentSyncRequest::Manifest {
                        requester_peer_id: manifest_request.requester_peer_id,
                        cursor: manifest_request.cursor,
                        limit: manifest_request.limit,
                        timestamp: manifest_request.timestamp,
                        signature: manifest_request.signature,
                    };

                    self.swarm
                        .behaviour_mut()
                        .content_sync
                        .send_request(&peer_id, wire_message);
                }

                NetworkResponse::Ok
            }

            NetworkCommand::RequestContentManifest {
                peer_id,
                cursor,
                limit,
            } => {
                const MAX_MANIFEST_LIMIT: u32 = 1000;
                let clamped_limit = limit.min(MAX_MANIFEST_LIMIT);

                let Some(ref content_sync_service) = self.content_sync_service else {
                    return NetworkResponse::Error("Content sync service unavailable".to_string());
                };

                let manifest_request =
                    match content_sync_service.create_manifest_request(cursor, clamped_limit) {
                        Ok(request_value) => request_value,
                        Err(error) => {
                            return NetworkResponse::Error(format!(
                                "Failed to create manifest request: {}",
                                error
                            ));
                        }
                    };

                let wire_message = ContentSyncRequest::Manifest {
                    requester_peer_id: manifest_request.requester_peer_id,
                    cursor: manifest_request.cursor,
                    limit: manifest_request.limit,
                    timestamp: manifest_request.timestamp,
                    signature: manifest_request.signature,
                };

                self.swarm
                    .behaviour_mut()
                    .content_sync
                    .send_request(&peer_id, wire_message);

                NetworkResponse::Ok
            }

            NetworkCommand::RequestContentFetch {
                peer_id,
                post_id,
                include_media,
            } => {
                let Some(ref content_sync_service) = self.content_sync_service else {
                    return NetworkResponse::Error("Content sync service unavailable".to_string());
                };

                let fetch_request = match content_sync_service
                    .create_fetch_request(post_id.clone(), include_media)
                {
                    Ok(request_value) => request_value,
                    Err(error) => {
                        return NetworkResponse::Error(format!(
                            "Failed to create content fetch request: {}",
                            error
                        ));
                    }
                };

                let wire_message = ContentSyncRequest::FetchPost {
                    requester_peer_id: fetch_request.requester_peer_id,
                    post_id: fetch_request.post_id,
                    include_media: fetch_request.include_media,
                    timestamp: fetch_request.timestamp,
                    signature: fetch_request.signature,
                };

                self.swarm
                    .behaviour_mut()
                    .content_sync
                    .send_request(&peer_id, wire_message);

                NetworkResponse::Ok
            }

            NetworkCommand::JoinCommunity {
                relay_peer_id,
                relay_address,
            } => {
                let Some(ref board_service) = self.board_service else {
                    return NetworkResponse::Error("Board service unavailable".to_string());
                };

                // Store community locally
                if let Err(e) =
                    board_service.join_community(&relay_peer_id.to_string(), &relay_address, None)
                {
                    return NetworkResponse::Error(format!("Failed to join community: {}", e));
                }

                // Register peer with relay first, then ListBoards will be sent
                // after the PeerRegistered response is received (to avoid race condition
                // where ListBoards arrives before the relay has stored our public key).
                match board_service.create_peer_registration() {
                    Ok(reg) => {
                        let request = WireBoardSyncRequest::RegisterPeer {
                            peer_id: reg.peer_id,
                            public_key: reg.public_key,
                            display_name: reg.display_name,
                            timestamp: reg.timestamp,
                            signature: reg.signature,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);

                        // Track that we're waiting for registration to complete
                        // before sending ListBoards
                        self.pending_board_registrations.insert(relay_peer_id);

                        NetworkResponse::Ok
                    }
                    Err(e) => {
                        NetworkResponse::Error(format!("Failed to create registration: {}", e))
                    }
                }
            }

            NetworkCommand::ListBoards { relay_peer_id } => {
                let Some(ref board_service) = self.board_service else {
                    return NetworkResponse::Error("Board service unavailable".to_string());
                };

                match board_service.create_list_boards_request() {
                    Ok(req) => {
                        let request = WireBoardSyncRequest::ListBoards {
                            requester_peer_id: req.requester_peer_id,
                            timestamp: req.timestamp,
                            signature: req.signature,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);
                        NetworkResponse::Ok
                    }
                    Err(e) => NetworkResponse::Error(format!(
                        "Failed to create list boards request: {}",
                        e
                    )),
                }
            }

            NetworkCommand::GetBoardPosts {
                relay_peer_id,
                board_id,
                after_timestamp,
                limit,
            } => {
                let Some(ref board_service) = self.board_service else {
                    return NetworkResponse::Error("Board service unavailable".to_string());
                };

                match board_service.create_get_board_posts_request(
                    &board_id,
                    after_timestamp,
                    limit,
                ) {
                    Ok(req) => {
                        let request = WireBoardSyncRequest::GetBoardPosts {
                            requester_peer_id: req.requester_peer_id,
                            board_id: req.board_id,
                            after_timestamp: req.after_timestamp,
                            limit: req.limit,
                            timestamp: req.timestamp,
                            signature: req.signature,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);
                        NetworkResponse::Ok
                    }
                    Err(e) => NetworkResponse::Error(format!(
                        "Failed to create get board posts request: {}",
                        e
                    )),
                }
            }

            NetworkCommand::SubmitBoardPost {
                relay_peer_id,
                board_id,
                content_text,
            } => {
                let Some(ref board_service) = self.board_service else {
                    return NetworkResponse::Error("Board service unavailable".to_string());
                };

                match board_service.create_board_post(&board_id, &content_text) {
                    Ok(post) => {
                        let request = WireBoardSyncRequest::SubmitPost {
                            post_id: post.post_id,
                            board_id: post.board_id,
                            author_peer_id: post.author_peer_id,
                            content_type: post.content_type,
                            content_text: post.content_text,
                            lamport_clock: post.lamport_clock,
                            created_at: post.created_at,
                            signature: post.signature,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);
                        NetworkResponse::Ok
                    }
                    Err(e) => NetworkResponse::Error(format!("Failed to create board post: {}", e)),
                }
            }

            NetworkCommand::DeleteBoardPost {
                relay_peer_id,
                post_id,
            } => {
                let Some(ref board_service) = self.board_service else {
                    return NetworkResponse::Error("Board service unavailable".to_string());
                };

                match board_service.create_delete_post_request(&post_id) {
                    Ok(req) => {
                        let request = WireBoardSyncRequest::DeletePost {
                            post_id: req.post_id,
                            author_peer_id: req.author_peer_id,
                            timestamp: req.timestamp,
                            signature: req.signature,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);
                        NetworkResponse::Ok
                    }
                    Err(e) => {
                        NetworkResponse::Error(format!("Failed to create delete request: {}", e))
                    }
                }
            }

            NetworkCommand::SyncBoard {
                relay_peer_id,
                board_id,
            } => {
                let Some(ref board_service) = self.board_service else {
                    return NetworkResponse::Error("Board service unavailable".to_string());
                };

                let after_timestamp = board_service
                    .get_sync_cursor(&relay_peer_id.to_string(), &board_id)
                    .unwrap_or(None);

                match board_service.create_get_board_posts_request(&board_id, after_timestamp, 50) {
                    Ok(req) => {
                        let request = WireBoardSyncRequest::GetBoardPosts {
                            requester_peer_id: req.requester_peer_id,
                            board_id: req.board_id,
                            after_timestamp: req.after_timestamp,
                            limit: req.limit,
                            timestamp: req.timestamp,
                            signature: req.signature,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);
                        NetworkResponse::Ok
                    }
                    Err(e) => {
                        NetworkResponse::Error(format!("Failed to create sync request: {}", e))
                    }
                }
            }

            NetworkCommand::SubmitWallPostToRelay {
                relay_peer_id,
                post_id,
                content_type,
                content_text,
                visibility,
                lamport_clock,
                created_at,
                signature,
                media_hashes,
                media_items,
            } => {
                let identity = match self.identity_service.get_identity() {
                    Ok(Some(id)) => id,
                    Ok(None) => {
                        return NetworkResponse::Error("No identity available".to_string());
                    }
                    Err(e) => {
                        return NetworkResponse::Error(format!("Identity error: {}", e));
                    }
                };

                let now = chrono::Utc::now().timestamp();
                let signable = SignableWallPostSubmit {
                    author_peer_id: identity.peer_id.clone(),
                    post_id: post_id.clone(),
                    content_type: content_type.clone(),
                    content_text: content_text.clone(),
                    visibility: visibility.clone(),
                    lamport_clock,
                    created_at,
                    signature: signature.clone(),
                    media_hashes: media_hashes.clone(),
                    media_items: media_items
                        .iter()
                        .map(|m| crate::services::SignedPostMediaMetadata {
                            media_hash: m.media_hash.clone(),
                            media_type: m.media_type.clone(),
                            mime_type: m.mime_type.clone(),
                            file_name: m.file_name.clone(),
                            file_size: m.file_size,
                            width: m.width,
                            height: m.height,
                            duration_seconds: m.duration_seconds,
                            sort_order: m.sort_order,
                            signature: m.signature.clone(),
                        })
                        .collect(),
                    timestamp: now,
                };

                match self.identity_service.sign(&signable) {
                    Ok(request_signature) => {
                        Self::emit_wall_sync_status(
                            self.event_tx.clone(),
                            "author_wall",
                            "in_progress",
                            "sync_started",
                            Some(relay_peer_id.to_string()),
                            Some(identity.peer_id.clone()),
                            Some(post_id.clone()),
                            None,
                            None,
                            Some(lamport_clock as u64),
                            None,
                        )
                        .await;
                        let request = WireBoardSyncRequest::SubmitWallPost {
                            author_peer_id: identity.peer_id,
                            post_id,
                            content_type,
                            content_text,
                            visibility,
                            lamport_clock,
                            created_at,
                            signature,
                            media_hashes,
                            timestamp: now,
                            request_signature,
                            media_items,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);
                        NetworkResponse::Ok
                    }
                    Err(e) => NetworkResponse::Error(format!(
                        "Failed to sign wall post submission: {}",
                        e
                    )),
                }
            }

            NetworkCommand::FetchMedia {
                peer_id,
                media_hash,
            } => {
                let source_is_authorized = self
                    .contacts_service
                    .as_ref()
                    .and_then(|service| service.get_contact(&peer_id.to_string()).ok().flatten())
                    .is_some_and(|contact| !contact.is_blocked);
                if !source_is_authorized {
                    return NetworkResponse::Error(
                        "Attachment source is not an active contact".to_string(),
                    );
                }

                let identity = match self.identity_service.get_identity() {
                    Ok(Some(id)) => id,
                    Ok(None) => {
                        return NetworkResponse::Error("No identity available".to_string());
                    }
                    Err(e) => {
                        return NetworkResponse::Error(format!("Identity error: {}", e));
                    }
                };

                if self.pending_media_fetch_by_hash.contains_key(&media_hash) {
                    return NetworkResponse::Ok;
                }
                let offset = self
                    .media_service
                    .as_ref()
                    .and_then(|service| service.partial_bytes(&media_hash).ok())
                    .unwrap_or(0);
                match self.queue_media_range(peer_id, media_hash, identity.peer_id, offset, true) {
                    Ok(()) => NetworkResponse::Ok,
                    Err(error) => {
                        NetworkResponse::Error(format!("Failed to start media fetch: {error}"))
                    }
                }
            }

            NetworkCommand::GetWallPostsFromRelay {
                relay_peer_id,
                author_peer_id,
                since_lamport_clock,
                limit,
            } => {
                let identity = match self.identity_service.get_identity() {
                    Ok(Some(id)) => id,
                    Ok(None) => {
                        return NetworkResponse::Error("No identity available".to_string());
                    }
                    Err(e) => {
                        return NetworkResponse::Error(format!("Identity error: {}", e));
                    }
                };

                let now = chrono::Utc::now().timestamp();
                let signable = SignableGetWallPosts {
                    requester_peer_id: identity.peer_id.clone(),
                    author_peer_id: author_peer_id.clone(),
                    since_lamport_clock,
                    limit,
                    timestamp: now,
                };

                match self.identity_service.sign(&signable) {
                    Ok(signature) => {
                        Self::emit_wall_sync_status(
                            self.event_tx.clone(),
                            if author_peer_id.is_empty() {
                                "feed"
                            } else {
                                "contact_wall"
                            },
                            "in_progress",
                            "author_requested",
                            Some(relay_peer_id.to_string()),
                            Some(author_peer_id.clone()),
                            None,
                            None,
                            None,
                            Some(since_lamport_clock.max(0) as u64),
                            None,
                        )
                        .await;
                        let request = WireBoardSyncRequest::GetWallPosts {
                            requester_peer_id: identity.peer_id,
                            author_peer_id,
                            since_lamport_clock,
                            limit,
                            timestamp: now,
                            signature,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);
                        NetworkResponse::Ok
                    }
                    Err(e) => {
                        NetworkResponse::Error(format!("Failed to sign wall posts request: {}", e))
                    }
                }
            }

            NetworkCommand::DeleteWallPostOnRelay {
                relay_peer_id,
                post_id,
                lamport_clock,
                deleted_at,
                signature,
            } => {
                let identity = match self.identity_service.get_identity() {
                    Ok(Some(id)) => id,
                    Ok(None) => {
                        return NetworkResponse::Error("No identity available".to_string());
                    }
                    Err(e) => {
                        return NetworkResponse::Error(format!("Identity error: {}", e));
                    }
                };

                let request = WireBoardSyncRequest::DeleteWallPost {
                    author_peer_id: identity.peer_id,
                    post_id,
                    lamport_clock,
                    deleted_at,
                    signature,
                };
                self.swarm
                    .behaviour_mut()
                    .board_sync
                    .send_request(&relay_peer_id, request);
                NetworkResponse::Ok
            }

            NetworkCommand::SubmitWallSocialEventsToRelay {
                relay_peer_id,
                events,
            } => {
                let identity = match self.identity_service.get_identity() {
                    Ok(Some(id)) => id,
                    Ok(None) => return NetworkResponse::Error("No identity available".to_string()),
                    Err(e) => return NetworkResponse::Error(format!("Identity error: {}", e)),
                };
                let now = chrono::Utc::now().timestamp();
                for event in events {
                    if event.actor_peer_id != identity.peer_id {
                        return NetworkResponse::Error(
                            "Cannot submit another peer's wall social event".to_string(),
                        );
                    }
                    let signable = SignableWallSocialEventSubmit {
                        event_id: event.event_id.clone(),
                        event_type: event.event_type.clone(),
                        post_id: event.post_id.clone(),
                        actor_peer_id: event.actor_peer_id.clone(),
                        author_name: event.author_name.clone(),
                        comment_id: event.comment_id.clone(),
                        content: event.content.clone(),
                        reaction_type: event.reaction_type.clone(),
                        timestamp: event.timestamp,
                        payload_cbor: event.payload_cbor.clone(),
                        signature: event.signature.clone(),
                        request_timestamp: now,
                    };
                    let request_signature = match self.identity_service.sign(&signable) {
                        Ok(sig) => sig,
                        Err(e) => {
                            return NetworkResponse::Error(format!(
                                "Failed to sign wall social event submission: {}",
                                e
                            ))
                        }
                    };
                    self.swarm.behaviour_mut().board_sync.send_request(
                        &relay_peer_id,
                        WireBoardSyncRequest::SubmitWallSocialEvent {
                            event,
                            timestamp: now,
                            request_signature,
                        },
                    );
                }
                NetworkResponse::Ok
            }

            NetworkCommand::GetWallSocialEventsFromRelay {
                relay_peer_id,
                author_peer_id,
                post_ids,
                after_timestamp,
                limit,
            } => {
                let identity = match self.identity_service.get_identity() {
                    Ok(Some(id)) => id,
                    Ok(None) => return NetworkResponse::Error("No identity available".to_string()),
                    Err(e) => return NetworkResponse::Error(format!("Identity error: {}", e)),
                };
                let now = chrono::Utc::now().timestamp();
                let signable = SignableGetWallSocialEvents {
                    requester_peer_id: identity.peer_id.clone(),
                    author_peer_id: author_peer_id.clone(),
                    post_ids: post_ids.clone(),
                    after_timestamp,
                    limit,
                    timestamp: now,
                };
                match self.identity_service.sign(&signable) {
                    Ok(signature) => {
                        self.swarm.behaviour_mut().board_sync.send_request(
                            &relay_peer_id,
                            WireBoardSyncRequest::GetWallSocialEvents {
                                requester_peer_id: identity.peer_id,
                                author_peer_id,
                                post_ids,
                                after_timestamp,
                                limit,
                                timestamp: now,
                                signature,
                            },
                        );
                        NetworkResponse::Ok
                    }
                    Err(e) => NetworkResponse::Error(format!(
                        "Failed to sign wall social events request: {}",
                        e
                    )),
                }
            }

            NetworkCommand::Shutdown => NetworkResponse::Ok,
        }
    }

    /// Attempt to connect to public relay servers
    /// This is called when we detect we're behind NAT or when manually requested
    pub async fn try_connect_to_relays(&mut self) {
        if self.relay_connection_attempted {
            info!("Already attempted to connect to relays, skipping");
            return;
        }
        self.connect_to_relays().await;
    }
}

#[cfg(test)]
mod signaling_replay_bridge_tests {
    use super::*;
    use crate::db::{Capability, ContactData, ContactsRepository};
    use crate::models::CreateIdentityRequest;
    use crate::p2p::protocols::signaling::{
        SignalingAnswer, SignalingEnvelope, SignalingOffer, SignalingPayload,
    };

    struct TestPeer {
        db: Arc<crate::Database>,
        identity: Arc<IdentityService>,
        permissions: Arc<PermissionsService>,
        calling: Arc<CallingService>,
        peer_id: String,
    }

    fn test_peer(display_name: &str) -> TestPeer {
        let db = Arc::new(crate::Database::in_memory().unwrap());
        let identity = Arc::new(IdentityService::new(db.clone()));
        let contacts = Arc::new(ContactsService::new(db.clone(), identity.clone()));
        let permissions = Arc::new(PermissionsService::new(db.clone(), identity.clone()));
        let peer_id = identity
            .create_identity(CreateIdentityRequest {
                display_name: display_name.into(),
                passphrase: "test-passphrase".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap()
            .peer_id;
        let calling = Arc::new(CallingService::new(
            db.clone(),
            identity.clone(),
            contacts,
            permissions.clone(),
        ));
        TestPeer {
            db,
            identity,
            permissions,
            calling,
            peer_id,
        }
    }

    fn add_call_relationship(local: &TestPeer, remote: &TestPeer) {
        let identity = remote.identity.get_identity().unwrap().unwrap();
        ContactsRepository::add_contact(
            &local.db,
            &ContactData {
                peer_id: identity.peer_id,
                public_key: identity.public_key,
                x25519_public: identity.x25519_public,
                display_name: identity.display_name,
                avatar_hash: None,
                bio: None,
            },
        )
        .unwrap();
        local
            .permissions
            .create_permission_grant(&remote.peer_id, Capability::Call, None)
            .unwrap();
    }

    #[tokio::test]
    async fn duplicate_answer_is_accepted_and_republished_to_frontend() {
        let caller = test_peer("Caller");
        let callee = test_peer("Callee");
        add_call_relationship(&caller, &callee);
        add_call_relationship(&callee, &caller);

        let offer = caller
            .calling
            .create_offer(&callee.peer_id, "v=0\r\ns=Harbor test\r\n")
            .unwrap();
        let offer_envelope = SignalingEnvelope {
            sender_peer_id: offer.caller_peer_id.clone(),
            recipient_peer_id: offer.callee_peer_id.clone(),
            payload: SignalingPayload::Offer(SignalingOffer {
                call_id: offer.call_id.clone(),
                caller_peer_id: offer.caller_peer_id.clone(),
                callee_peer_id: offer.callee_peer_id.clone(),
                sdp: offer.sdp,
                timestamp: offer.timestamp,
                signature: offer.signature,
            }),
        };
        callee
            .calling
            .process_incoming_signaling(&offer_envelope)
            .unwrap();
        let answer = callee
            .calling
            .create_answer(
                &offer.call_id,
                &caller.peer_id,
                "v=0\r\ns=Harbor test answer\r\n",
            )
            .unwrap();
        let answer_envelope = SignalingEnvelope {
            sender_peer_id: answer.callee_peer_id.clone(),
            recipient_peer_id: answer.caller_peer_id.clone(),
            payload: SignalingPayload::Answer(SignalingAnswer {
                call_id: answer.call_id,
                caller_peer_id: answer.caller_peer_id,
                callee_peer_id: answer.callee_peer_id,
                sdp: answer.sdp,
                timestamp: answer.timestamp,
                signature: answer.signature,
            }),
        };

        let network_key = libp2p::identity::Keypair::generate_ed25519();
        let (mut network, _handle, mut events) = NetworkService::new(
            NetworkConfig::default(),
            caller.identity.clone(),
            network_key,
        )
        .unwrap();
        network.set_calling_service(caller.calling.clone());
        let callee_transport: PeerId = callee.peer_id.parse().unwrap();

        for _ in 0..2 {
            let response = network
                .handle_signaling_request(callee_transport, answer_envelope.clone())
                .await;
            assert!(response.accepted, "answer replay must be acknowledged");
            let event = tokio::time::timeout(Duration::from_secs(1), events.recv())
                .await
                .expect("frontend answer event timed out")
                .expect("frontend event channel closed");
            assert!(matches!(
                event,
                NetworkEvent::CallSignalingReceived { peer_id, message }
                    if peer_id == callee.peer_id && message == answer_envelope
            ));
        }
    }
}

#[cfg(test)]
mod messaging_request_correlation_tests {
    use super::*;

    fn peer() -> PeerId {
        libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id()
    }

    fn pending(
        tracker: &mut PendingMessagingRequests<u64>,
        request_id: u64,
        event_id: &str,
        message_id: &str,
        peer_id: PeerId,
        deadline: Instant,
    ) -> oneshot::Receiver<NetworkResponse> {
        let (tx, rx) = oneshot::channel();
        tracker.insert(
            request_id,
            event_id.into(),
            message_id.into(),
            peer_id,
            deadline,
            Some(tx),
        );
        rx
    }

    #[tokio::test]
    async fn reordered_same_peer_responses_complete_only_the_matching_waiter() {
        let peer = peer();
        let mut tracker = PendingMessagingRequests::default();
        let first = pending(
            &mut tracker,
            11,
            "event-1",
            "message-1",
            peer,
            Instant::now() + Duration::from_secs(1),
        );
        let second = pending(
            &mut tracker,
            12,
            "event-2",
            "message-2",
            peer,
            Instant::now() + Duration::from_secs(1),
        );

        assert!(tracker
            .complete_remote(
                &12,
                &peer,
                MessagingResponse {
                    success: true,
                    message_id: Some("message-2".into()),
                    error: None,
                },
            )
            .is_some());
        assert_eq!(tracker.len(), 1);
        assert!(matches!(
            second.await.unwrap(),
            NetworkResponse::MessageDelivered(MessageDeliveryReceipt { event_id, message_id })
                if event_id == "event-2" && message_id == "message-2"
        ));

        assert!(tracker
            .complete_remote(
                &11,
                &peer,
                MessagingResponse {
                    success: true,
                    message_id: Some("message-1".into()),
                    error: None,
                },
            )
            .is_some());
        assert!(matches!(
            first.await.unwrap(),
            NetworkResponse::MessageDelivered(MessageDeliveryReceipt { event_id, message_id })
                if event_id == "event-1" && message_id == "message-1"
        ));
        assert_eq!(tracker.len(), 0);
        assert!(tracker
            .complete_remote(
                &11,
                &peer,
                MessagingResponse {
                    success: true,
                    message_id: Some("message-1".into()),
                    error: None,
                },
            )
            .is_none());
    }

    #[tokio::test]
    async fn outbound_failure_and_bad_ack_are_typed_and_cleaned() {
        let peer = peer();
        let mut tracker = PendingMessagingRequests::default();
        let failed = pending(
            &mut tracker,
            21,
            "event-failed",
            "message-failed",
            peer,
            Instant::now() + Duration::from_secs(1),
        );
        assert!(tracker
            .fail(&21, MessageDeliveryFailureKind::Network, "connection reset",)
            .is_some());
        assert!(matches!(
            failed.await.unwrap(),
            NetworkResponse::MessageDeliveryFailed(MessageDeliveryFailure {
                kind: MessageDeliveryFailureKind::Network,
                event_id,
                ..
            }) if event_id == "event-failed"
        ));

        let mismatch = pending(
            &mut tracker,
            22,
            "event-mismatch",
            "expected-message",
            peer,
            Instant::now() + Duration::from_secs(1),
        );
        assert!(tracker
            .complete_remote(
                &22,
                &peer,
                MessagingResponse {
                    success: true,
                    message_id: Some("other-message".into()),
                    error: None,
                },
            )
            .is_some());
        assert!(matches!(
            mismatch.await.unwrap(),
            NetworkResponse::MessageDeliveryFailed(MessageDeliveryFailure {
                kind: MessageDeliveryFailureKind::InvalidResponse,
                ..
            })
        ));
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn background_attempt_without_waiter_still_produces_a_correlated_outcome() {
        let peer = peer();
        let mut tracker = PendingMessagingRequests::default();
        tracker.insert(
            25,
            "background-event".into(),
            "background-message".into(),
            peer,
            Instant::now() + Duration::from_secs(1),
            None,
        );

        let outcome = tracker
            .complete_remote(
                &25,
                &peer,
                MessagingResponse {
                    success: true,
                    message_id: Some("background-message".into()),
                    error: None,
                },
            )
            .unwrap();
        assert_eq!(
            outcome,
            MessageDeliveryAttemptOutcome::Delivered(MessageDeliveryReceipt {
                event_id: "background-event".into(),
                message_id: "background-message".into(),
            })
        );
        assert_eq!(tracker.len(), 0);
    }

    #[tokio::test]
    async fn timeout_and_cancelled_caller_remove_pending_state() {
        let peer = peer();
        let mut tracker = PendingMessagingRequests::default();
        let timed_out = pending(
            &mut tracker,
            31,
            "event-timeout",
            "message-timeout",
            peer,
            Instant::now() - Duration::from_millis(1),
        );
        let cancelled = pending(
            &mut tracker,
            32,
            "event-cancelled",
            "message-cancelled",
            peer,
            Instant::now() + Duration::from_secs(1),
        );
        drop(cancelled);

        let (cancelled_count, expired) = tracker.maintain(Instant::now());
        assert_eq!(cancelled_count, 1);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].kind, MessageDeliveryFailureKind::Timeout);
        assert!(matches!(
            timed_out.await.unwrap(),
            NetworkResponse::MessageDeliveryFailed(MessageDeliveryFailure {
                kind: MessageDeliveryFailureKind::Timeout,
                ..
            })
        ));
        assert_eq!(tracker.len(), 0);
    }

    #[tokio::test]
    async fn disconnect_and_shutdown_complete_each_live_waiter_once() {
        let disconnected_peer = peer();
        let other_peer = peer();
        let mut tracker = PendingMessagingRequests::default();
        let disconnected = pending(
            &mut tracker,
            41,
            "event-disconnected",
            "message-disconnected",
            disconnected_peer,
            Instant::now() + Duration::from_secs(1),
        );
        let shutdown = pending(
            &mut tracker,
            42,
            "event-shutdown",
            "message-shutdown",
            other_peer,
            Instant::now() + Duration::from_secs(1),
        );

        let disconnected_failures = tracker.fail_peer(
            &disconnected_peer,
            MessageDeliveryFailureKind::Disconnected,
            "connection closed",
        );
        assert_eq!(disconnected_failures.len(), 1);
        assert!(matches!(
            disconnected.await.unwrap(),
            NetworkResponse::MessageDeliveryFailed(MessageDeliveryFailure {
                kind: MessageDeliveryFailureKind::Disconnected,
                ..
            })
        ));
        assert_eq!(tracker.fail_all("runtime stopped").len(), 1);
        assert!(matches!(
            shutdown.await.unwrap(),
            NetworkResponse::MessageDeliveryFailed(MessageDeliveryFailure {
                kind: MessageDeliveryFailureKind::Shutdown,
                ..
            })
        ));
        assert_eq!(tracker.len(), 0);
        assert!(tracker.fail_all("second shutdown").is_empty());
    }
}

#[cfg(test)]
mod introduction_ack_tests {
    use super::{await_name_registration, should_ack_ingest};
    use std::time::Duration;
    use tokio::sync::oneshot;
    #[test]
    fn success_duplicate_and_blocked_are_acked() {
        assert!(should_ack_ingest(&Ok::<bool, &str>(true)));
        assert!(should_ack_ingest(&Ok::<bool, &str>(false)));
    }
    #[test]
    fn tamper_or_validation_error_is_not_acked() {
        assert!(!should_ack_ingest(&Err::<bool, &str>("tampered")));
    }

    #[tokio::test]
    async fn relay_name_registration_has_a_bounded_wait() {
        // Keep the response sender alive to model a relay request that never completes.
        let (response_tx, response_rx) = oneshot::channel();
        let error = await_name_registration(response_rx, Duration::from_millis(10))
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("timed out"));
        assert!(
            response_tx.is_closed(),
            "timeout must abandon stale progress"
        );
    }
}

#[cfg(test)]
mod contact_request_protocol_tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey, Verifier};

    fn signed_request(action: &str, now: i64) -> (PeerId, IdentityExchangeRequest) {
        let key = SigningKey::from_bytes(&[42; 32]);
        let peer_id =
            crate::services::CryptoService::derive_peer_id_from_verifying_key(&key.verifying_key())
                .unwrap();
        let peer: PeerId = peer_id.parse().unwrap();
        let mut request = IdentityExchangeRequest {
            request_id: uuid::Uuid::new_v4().to_string(),
            action: action.into(),
            requester_peer_id: peer_id,
            public_key: key.verifying_key().to_bytes().to_vec(),
            x25519_public: vec![7; 32],
            display_name: "Alice".into(),
            avatar_hash: None,
            avatar_mime_type: None,
            profile_revision: if action == "profile" { 1 } else { 0 },
            bio: None,
            timestamp: now,
            permission_grants: Vec::new(),
            permission_revocations: Vec::new(),
            signature: Vec::new(),
        };
        request.signature = key
            .sign(&identity_request_signing_bytes(&request).unwrap())
            .to_bytes()
            .to_vec();
        (peer, request)
    }

    #[test]
    fn signed_request_actions_are_bound_and_tamper_evident() {
        let now = 10_000;
        let (peer, request) = signed_request("request", now);
        assert!(verify_identity_request(peer, &request, now));
        let mut altered = request.clone();
        altered.action = "accepted".into();
        assert!(!verify_identity_request(peer, &altered, now));
        let mut stale = request;
        stale.timestamp = now - 301;
        assert!(!verify_identity_request(peer, &stale, now));

        let (peer, profile) = signed_request("profile", now);
        assert!(verify_identity_request(peer, &profile, now));
        let mut altered_revision = profile;
        altered_revision.profile_revision += 1;
        assert!(!verify_identity_request(peer, &altered_revision, now));
    }

    #[test]
    fn signed_response_status_is_bound_and_tamper_evident() {
        let key = SigningKey::from_bytes(&[43; 32]);
        let peer_id =
            crate::services::CryptoService::derive_peer_id_from_verifying_key(&key.verifying_key())
                .unwrap();
        let mut response = IdentityExchangeResponse {
            request_id: uuid::Uuid::new_v4().to_string(),
            status: "review".into(),
            peer_id,
            public_key: key.verifying_key().to_bytes().to_vec(),
            x25519_public: vec![8; 32],
            display_name: "Bob".into(),
            avatar_hash: None,
            avatar_mime_type: None,
            profile_revision: 0,
            bio: None,
            timestamp: 10_000,
            permission_grants: Vec::new(),
            permission_revocations: Vec::new(),
            signature: Vec::new(),
        };
        response.signature = key
            .sign(&identity_response_signing_bytes(&response).unwrap())
            .to_bytes()
            .to_vec();
        let signature = ed25519_dalek::Signature::from_slice(&response.signature).unwrap();
        assert!(key
            .verifying_key()
            .verify(
                &identity_response_signing_bytes(&response).unwrap(),
                &signature
            )
            .is_ok());

        response.status = "accepted".into();
        assert!(key
            .verifying_key()
            .verify(
                &identity_response_signing_bytes(&response).unwrap(),
                &signature
            )
            .is_err());
    }

    #[test]
    fn forged_revocation_ack_is_not_authenticated() {
        let now = 10_000;
        let key = SigningKey::from_bytes(&[44; 32]);
        let peer_id =
            crate::services::CryptoService::derive_peer_id_from_verifying_key(&key.verifying_key())
                .unwrap();
        let peer: PeerId = peer_id.parse().unwrap();
        let mut response = IdentityExchangeResponse {
            request_id: uuid::Uuid::new_v4().to_string(),
            status: "revoked".into(),
            peer_id,
            public_key: key.verifying_key().to_bytes().to_vec(),
            x25519_public: vec![9; 32],
            display_name: "Revoked peer".into(),
            avatar_hash: None,
            avatar_mime_type: None,
            profile_revision: 0,
            bio: None,
            timestamp: now,
            permission_grants: Vec::new(),
            permission_revocations: Vec::new(),
            signature: Vec::new(),
        };
        response.signature = key
            .sign(&identity_response_signing_bytes(&response).unwrap())
            .to_bytes()
            .to_vec();
        assert!(verify_identity_response(peer, &response, now));

        let attacker = SigningKey::from_bytes(&[45; 32]);
        response.signature = attacker
            .sign(&identity_response_signing_bytes(&response).unwrap())
            .to_bytes()
            .to_vec();
        assert!(!verify_identity_response(peer, &response, now));
    }
}

#[cfg(test)]
mod media_authorization_tests {
    use super::*;
    use crate::p2p::protocols::media_sync::{MediaFetchRequest, MediaFetchResponse};

    #[tokio::test]
    async fn media_request_fails_closed_without_authorized_contact_context() {
        let db = Arc::new(crate::db::Database::in_memory().unwrap());
        let identity = Arc::new(IdentityService::new(db));
        let network_key = libp2p::identity::Keypair::generate_ed25519();
        let requester_key = libp2p::identity::Keypair::generate_ed25519();
        let requester = requester_key.public().to_peer_id();
        let (service, _, _) =
            NetworkService::new(NetworkConfig::default(), identity, network_key).unwrap();
        let response = service.handle_media_fetch_request(
            requester,
            &MediaFetchRequest {
                media_hash: "d".repeat(64),
                offset: 0,
                max_bytes: crate::services::media_service::DEFAULT_CHUNK_SIZE,
                requester_peer_id: requester.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
                signature: vec![0; 64],
            },
        );
        assert!(matches!(
            response,
            MediaFetchResponse::Error { error } if error == "Authorization unavailable"
        ));
    }
}

#[cfg(test)]
mod messaging_ingress_tests {
    use super::*;

    #[test]
    fn claimed_message_author_must_be_the_transport_peer() {
        let transport = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let attacker = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();

        assert!(messaging_sender_matches_transport(
            &transport,
            &transport.to_string()
        ));
        assert!(!messaging_sender_matches_transport(
            &transport,
            &attacker.to_string()
        ));
        assert!(!messaging_sender_matches_transport(&transport, ""));
    }
}
