//! Harbor Relay Server
//!
//! A libp2p relay server that enables NAT traversal for Harbor chat app users.
//! Run with `--community` to enable community boards with SQLite storage.

mod abuse;
mod admission;
mod auth;
mod board_service;
mod db;
mod introduction;
mod name_registration;
mod peer_binding;
mod read_auth;

pub use harbor_relay_server::resource_limits;

use board_service::{BoardService, WallReadGrantProof};
use clap::Parser;
use db::RelayDatabase;
use futures::StreamExt;
use harbor_relay_server::identity_key::load_or_generate_identity;
use libp2p::{
    identify, noise, ping, relay,
    request_response::{self, ProtocolSupport},
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, StreamProtocol, SwarmBuilder,
};
use read_auth::{ReadReplayToken, RelayReadGuard};
use resource_limits::{ResourceLimitArgs, ResourceLimits};
use std::collections::HashMap;
use std::fs;
use std::net::Ipv4Addr;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

/// Board sync protocol version
const BOARD_SYNC_PROTOCOL: &str = "/harbor/board/1.0.0";

fn is_routable_announce_ip(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    !ip.is_unspecified()
        && !ip.is_loopback()
        && !ip.is_private()
        && !ip.is_link_local()
        && !ip.is_multicast()
        && !ip.is_broadcast()
        && !ip.is_documentation()
        && octets[0] != 0
        && octets[0] < 240
        && !(octets[0] == 100 && (64..=127).contains(&octets[1]))
        && !(octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        && !(octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
}

fn validated_external_addresses(
    announce_ip: Ipv4Addr,
    port: u16,
    peer_id: &PeerId,
) -> Result<[Multiaddr; 2], String> {
    if !is_routable_announce_ip(announce_ip) {
        return Err(format!(
            "{announce_ip} is not a publicly routable IPv4 address"
        ));
    }
    let tcp = format!("/ip4/{announce_ip}/tcp/{port}/p2p/{peer_id}")
        .parse()
        .map_err(|error| format!("could not construct TCP address: {error}"))?;
    let quic = format!("/ip4/{announce_ip}/udp/{port}/quic-v1/p2p/{peer_id}")
        .parse()
        .map_err(|error| format!("could not construct QUIC address: {error}"))?;
    Ok([tcp, quic])
}

fn source_bucket(addr: &Multiaddr) -> String {
    use libp2p::multiaddr::Protocol;
    for p in addr.iter() {
        match p {
            Protocol::Ip4(ip) => {
                let o = ip.octets();
                return format!("{}.{}.{}.0/24", o[0], o[1], o[2]);
            }
            Protocol::Ip6(ip) => {
                let s = ip.segments();
                return format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3]);
            }
            _ => {}
        }
    }
    "unknown".into()
}
fn response_delay(request_id: &str) -> Duration {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    request_id.hash(&mut h);
    Duration::from_millis(20 + h.finish() % 21)
}
fn opaque_delivery_key(
    database: &RelayDatabase,
    target: &str,
    key: &libp2p::identity::Keypair,
) -> Vec<u8> {
    database.with_connection(|conn|conn.query_row("SELECT claim_cbor FROM relay_name_claims WHERE ('@' || local_name || '@' || relay)=? AND status='active' ORDER BY sequence DESC LIMIT 1",[target],|r|r.get::<_,Vec<u8>>(0)).ok().and_then(|b|ciborium::de::from_reader::<name_registration::NameClaim,_>(b.as_slice()).ok()).map(|c|c.request.x25519_public_key)).unwrap_or_else(||{let signature=key.sign(format!("harbor/decoy-delivery-key/1:{target}").as_bytes()).unwrap_or_default();let mut seed=[0u8;32];if signature.len()>=32{seed.copy_from_slice(&signature[..32])}x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from(seed)).to_bytes().to_vec()})
}

struct IdentityTransport {
    auth: auth::AuthService,
    abuse: abuse::AbuseGuard,
    admission: admission::SourceAdmissionGuard,
    database: RelayDatabase,
    relay_name: String,
    relay_key_id: String,
    relay_signing_key: ed25519_dalek::SigningKey,
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    #[test]
    fn source_network_uses_privacy_preserving_prefix() {
        assert_eq!(
            source_bucket(&"/ip4/203.0.113.47/tcp/4001".parse().unwrap()),
            "203.0.113.0/24"
        );
        assert_eq!(
            source_bucket(&"/ip6/2001:db8:1:2::7/tcp/4001".parse().unwrap()),
            "2001:db8:1:2::/64"
        );
    }
    #[test]
    fn response_jitter_is_deterministic_and_bounded() {
        for id in ["known", "unknown", "blocked", "offline"] {
            let d = response_delay(id);
            assert!(d >= Duration::from_millis(20) && d <= Duration::from_millis(40));
            assert_eq!(d, response_delay(id));
        }
    }

    #[test]
    fn external_addresses_only_use_validated_routable_ip() {
        let peer = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let addresses =
            validated_external_addresses("8.8.8.8".parse().unwrap(), 4001, &peer).unwrap();
        assert!(addresses
            .iter()
            .all(|address| !address.to_string().contains("0.0.0.0")));
        assert!(addresses
            .iter()
            .all(|address| address.to_string().contains("/ip4/8.8.8.8/")));

        for rejected in [
            "0.0.0.0",
            "127.0.0.1",
            "10.0.0.1",
            "169.254.1.1",
            "192.0.2.1",
            "224.0.0.1",
        ] {
            assert!(validated_external_addresses(rejected.parse().unwrap(), 4001, &peer).is_err());
        }
    }

    #[test]
    fn relay_only_mode_keeps_identity_and_wall_protocols_but_rejects_boards() {
        let peer = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id()
            .to_string();
        let identity = BoardSyncRequest::RelayAuthChallenge {
            peer_id: peer.clone(),
            audience: "name:register".into(),
        };
        let wall = BoardSyncRequest::GetWallPosts {
            requester_peer_id: peer.clone(),
            author_peer_id: peer.clone(),
            since_lamport_clock: 0,
            limit: 20,
            timestamp: 1,
            signature: vec![0; 64],
            grant_proof: None,
        };
        let boards = BoardSyncRequest::ListBoards {
            requester_peer_id: peer,
            timestamp: 1,
            signature: vec![0; 64],
        };

        assert!(request_enabled_in_mode(&identity, false));
        assert!(request_enabled_in_mode(&wall, false));
        assert!(!request_enabled_in_mode(&boards, false));
        assert!(request_enabled_in_mode(&boards, true));
    }
    #[test]
    fn transport_peer_churn_is_hard_bounded_and_expiry_reclaims_slots() {
        let start = Instant::now();
        let mut limiter = PeerRateLimiter::new(10, Duration::from_secs(10), 2);
        let first = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let second = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let third = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        assert!(limiter.check_rate_limit_at(&first, start).is_ok());
        assert!(limiter.check_rate_limit_at(&second, start).is_ok());
        assert!(limiter.check_rate_limit_at(&third, start).is_err());
        assert_eq!(limiter.peers.len(), 2);

        limiter.cleanup_stale_entries_at(start + Duration::from_secs(21));
        assert_eq!(limiter.peers.len(), 0);
        assert!(limiter
            .check_rate_limit_at(&third, start + Duration::from_secs(21))
            .is_ok());
    }
    #[test]
    fn delivery_key_is_real_for_a_claim_and_stable_decoy_otherwise() {
        let database = RelayDatabase::open(":memory:").unwrap();
        let relay_key = libp2p::identity::Keypair::generate_ed25519();
        let target_secret = x25519_dalek::StaticSecret::from([19; 32]);
        let target_public = x25519_dalek::PublicKey::from(&target_secret)
            .to_bytes()
            .to_vec();
        let claim = name_registration::NameClaim {
            request: name_registration::NameClaimRequest {
                domain: "harbor/name-claim-request/1".into(),
                version: 1,
                local_name: "alice".into(),
                relay: "alpha.test".into(),
                peer_id: "peer-alice".into(),
                ed25519_public_key: vec![1; 32],
                x25519_public_key: target_public.clone(),
                sequence: 1,
                issued_at: 100,
                nonce: vec![2; 16],
            },
            user_signature: vec![3; 64],
            status: "active".into(),
            not_before: 100,
            not_after: 1_000,
            relay_key_id: "key-1".into(),
            relay_signature: vec![4; 64],
        };
        let mut claim_cbor = Vec::new();
        ciborium::ser::into_writer(&claim, &mut claim_cbor).unwrap();
        database.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO relay_name_claims VALUES(?,?,?,?,?,?,?,?, 'active',?,NULL)",
                    rusqlite::params![
                        "alice",
                        "alpha.test",
                        "peer-alice",
                        1,
                        claim_cbor,
                        100,
                        1_000,
                        "key-1",
                        100
                    ],
                )
                .unwrap();
        });

        assert_eq!(
            opaque_delivery_key(&database, "@alice@alpha.test", &relay_key),
            target_public
        );
        let first = opaque_delivery_key(&database, "@nobody@alpha.test", &relay_key);
        let second = opaque_delivery_key(&database, "@nobody@alpha.test", &relay_key);
        assert_eq!(first.len(), 32);
        assert_eq!(first, second);
        assert_ne!(first, target_public);
    }

    #[test]
    fn read_handler_rejects_claimed_requester_before_service_authorization() {
        let service =
            BoardService::new(RelayDatabase::open(":memory:").unwrap(), "test".to_string());
        let transport_peer = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let claimed_peer = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let local_peer = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let mut guard = RelayReadGuard::default();

        let response = handle_board_request(
            &service,
            &mut guard,
            None,
            "test-network",
            &local_peer,
            &transport_peer,
            BoardSyncRequest::ListBoards {
                requester_peer_id: claimed_peer.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
                signature: vec![1; 64],
            },
        );

        assert!(matches!(
            response,
            BoardSyncResponse::Error { error }
                if error == "RELAY_READ_REQUESTER_MISMATCH"
        ));
    }

    #[test]
    fn registration_handler_rejects_cross_transport_peer_claim() {
        let service =
            BoardService::new(RelayDatabase::open(":memory:").unwrap(), "test".to_string());
        let transport = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let claimed_key = libp2p::identity::Keypair::generate_ed25519();
        let claimed = claimed_key.public().to_peer_id();
        let local = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let mut guard = RelayReadGuard::default();
        let response = handle_board_request(
            &service,
            &mut guard,
            None,
            "test-network",
            &local,
            &transport,
            BoardSyncRequest::RegisterPeer {
                peer_id: claimed.to_string(),
                public_key: claimed_key.public().encode_protobuf(),
                display_name: "Mallory".into(),
                timestamp: chrono::Utc::now().timestamp(),
                signature: vec![0; 64],
            },
        );
        assert!(matches!(
            response,
            BoardSyncResponse::Error { error }
                if error == "RELAY_PEER_TRANSPORT_MISMATCH"
        ));
    }

    #[test]
    fn wall_post_handler_rejects_cross_transport_author_before_storage() {
        let service =
            BoardService::new(RelayDatabase::open(":memory:").unwrap(), "test".to_string());
        let transport = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let claimed = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let local = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let mut guard = RelayReadGuard::default();
        let response = handle_board_request(
            &service,
            &mut guard,
            None,
            "test-network",
            &local,
            &transport,
            BoardSyncRequest::SubmitWallPost {
                author_peer_id: claimed.to_string(),
                post_id: "spoofed-post".into(),
                content_type: "text".into(),
                content_text: Some("spoofed".into()),
                visibility: "public".into(),
                lamport_clock: 1,
                created_at: 1,
                signature: vec![0; 64],
                media_hashes: Vec::new(),
                timestamp: chrono::Utc::now().timestamp(),
                request_signature: vec![0; 64],
                media_items: Vec::new(),
            },
        );
        assert!(matches!(
            response,
            BoardSyncResponse::Error { error }
                if error == "RELAY_POST_TRANSPORT_MISMATCH"
        ));
    }

    #[test]
    fn source_capacity_preserves_generic_introduction_response() {
        let database = RelayDatabase::open(":memory:").unwrap();
        let service = BoardService::new(database.clone(), "test".into());
        let relay_key = libp2p::identity::Keypair::generate_ed25519();
        let ed = relay_key.clone().try_into_ed25519().unwrap().to_bytes();
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&ed[..32]);
        let mut identity = IdentityTransport {
            auth: auth::AuthService::new("relay.test", "k1", relay_key),
            abuse: abuse::AbuseGuard::new(abuse::Limits {
                peer: 10,
                network: 10,
                target: 10,
                action: 10,
                global: 10,
                window_secs: 60,
            }),
            admission: admission::SourceAdmissionGuard::new(admission::Limits {
                per_source: 1,
                global: 10,
                max_sources: 10,
                window_secs: 60,
            }),
            database,
            relay_name: "relay.test".into(),
            relay_key_id: "k1".into(),
            relay_signing_key: ed25519_dalek::SigningKey::from_bytes(&secret),
        };
        let peer = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let local = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        let mut read_guard = RelayReadGuard::default();
        let _ = handle_board_request(
            &service,
            &mut read_guard,
            Some(&mut identity),
            "203.0.113.0/24",
            &local,
            &peer,
            BoardSyncRequest::RelayAuthChallenge {
                peer_id: peer.to_string(),
                audience: "introduce".into(),
            },
        );
        let request_id = uuid::Uuid::new_v4().to_string();
        let response = handle_board_request(
            &service,
            &mut read_guard,
            Some(&mut identity),
            "203.0.113.0/24",
            &local,
            &peer,
            BoardSyncRequest::SubmitIntroduction {
                session_token: "invalid".into(),
                envelope: introduction::IntroductionEnvelope {
                    version: 1,
                    request_id: request_id.clone(),
                    target: "@unknown@relay.test".into(),
                    requester_peer_id: peer.to_string(),
                    requester_ephemeral_x25519_key: vec![0; 32],
                    message_ciphertext: vec![1],
                    issued_at: 1,
                    expires_at: 2,
                    work_challenge: abuse::WorkChallenge {
                        id: "work".into(),
                        relay: "relay.test".into(),
                        requester: peer.to_string(),
                        target: "@unknown@relay.test".into(),
                        action: "introduce".into(),
                        audience: "introduce".into(),
                        expires_at: 2,
                        difficulty: 0,
                        key_id: "k1".into(),
                        relay_signature: vec![],
                        delivery_key: vec![0; 32],
                    },
                    work_nonce: 0,
                },
            },
        );
        assert!(matches!(
            response,
            BoardSyncResponse::IntroductionAccepted {
                request_id: returned,
                retry_after: 3_600
            } if returned == request_id
        ));
    }
}

/// Per-peer rate limiter for board sync requests.
///
/// Tracks the number of requests each peer has made within a sliding window.
/// When a peer exceeds `max_requests` within `window_duration`, subsequent
/// requests are rejected until the window resets.
struct PeerRateLimiter {
    /// Maps each peer to (request_count, window_start_time)
    peers: HashMap<PeerId, (u64, Instant)>,
    /// Maximum number of requests allowed per window
    max_requests: u64,
    /// Duration of the rate limit window
    window_duration: Duration,
    /// Hard bound for transport identities retained in memory.
    max_tracked_peers: usize,
}

impl PeerRateLimiter {
    fn new(max_requests: u64, window_duration: Duration, max_tracked_peers: usize) -> Self {
        assert!(max_tracked_peers > 0);
        Self {
            peers: HashMap::new(),
            max_requests,
            window_duration,
            max_tracked_peers,
        }
    }

    /// Check whether a peer is allowed to make a request.
    ///
    /// Returns `Ok(())` if the request is permitted, or `Err(message)` if the
    /// peer has exceeded their rate limit for the current window.
    fn check_rate_limit(&mut self, peer_id: &PeerId) -> Result<(), String> {
        self.check_rate_limit_at(peer_id, Instant::now())
    }

    fn check_rate_limit_at(&mut self, peer_id: &PeerId, now: Instant) -> Result<(), String> {
        if !self.peers.contains_key(peer_id) && self.peers.len() >= self.max_tracked_peers {
            return Err("Rate limit capacity reached. Try again later.".to_string());
        }

        let (request_count, window_start) = self.peers.entry(*peer_id).or_insert((0, now));

        // If the current window has expired, reset the counter
        if now.duration_since(*window_start) >= self.window_duration {
            *request_count = 0;
            *window_start = now;
        }

        // Check if the peer has exceeded the limit
        if *request_count >= self.max_requests {
            warn!(
                "Rate limit exceeded for peer {}: {} requests in {}s window",
                peer_id,
                request_count,
                self.window_duration.as_secs()
            );
            return Err("Rate limit exceeded. Try again later.".to_string());
        }

        *request_count += 1;
        Ok(())
    }

    /// Remove entries for peers whose windows have long since expired.
    ///
    /// This prevents unbounded memory growth from peers that connect once
    /// and never return. An entry is considered stale if its window started
    /// more than `2 * window_duration` ago.
    fn cleanup_stale_entries(&mut self) {
        self.cleanup_stale_entries_at(Instant::now());
    }

    fn cleanup_stale_entries_at(&mut self, now: Instant) {
        let stale_threshold = self.window_duration * 2;
        let initial_count = self.peers.len();

        self.peers.retain(|_peer_id, (_count, window_start)| {
            now.duration_since(*window_start) < stale_threshold
        });

        let removed_count = initial_count - self.peers.len();
        if removed_count > 0 {
            info!(
                "Rate limiter cleanup: removed {} stale entries, {} remaining",
                removed_count,
                self.peers.len()
            );
        }
    }
}

/// Board sync request (wire protocol) — matches client types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BoardSyncRequest {
    RelayAuthChallenge {
        peer_id: String,
        audience: String,
    },
    RelayAuthComplete {
        challenge: auth::AuthChallenge,
        public_key: Vec<u8>,
        signature: Vec<u8>,
    },
    RegisterRelayName {
        session_token: String,
        signed_request: name_registration::SignedNameClaimRequest,
    },
    SubmitIntroduction {
        session_token: String,
        envelope: introduction::IntroductionEnvelope,
    },
    RequestIntroductionWork {
        session_token: String,
        target: String,
    },
    FetchIntroductions {
        session_token: String,
        limit: u32,
    },
    AckIntroductions {
        session_token: String,
        request_ids: Vec<String>,
    },
    ListBoards {
        requester_peer_id: String,
        timestamp: i64,
        signature: Vec<u8>,
    },
    GetBoardPosts {
        requester_peer_id: String,
        board_id: String,
        after_timestamp: Option<i64>,
        limit: u32,
        timestamp: i64,
        signature: Vec<u8>,
    },
    GetOlderBoardPosts {
        requester_peer_id: String,
        board_id: String,
        before: Option<db::BoardPostCursor>,
        limit: u32,
        timestamp: i64,
        signature: Vec<u8>,
    },
    SubmitPost {
        post_id: String,
        board_id: String,
        author_peer_id: String,
        content_type: String,
        content_text: Option<String>,
        lamport_clock: u64,
        created_at: i64,
        signature: Vec<u8>,
    },
    RegisterPeer {
        peer_id: String,
        public_key: Vec<u8>,
        display_name: String,
        timestamp: i64,
        signature: Vec<u8>,
    },
    DeletePost {
        post_id: String,
        author_peer_id: String,
        timestamp: i64,
        signature: Vec<u8>,
    },
    SubmitWallPost {
        author_peer_id: String,
        post_id: String,
        content_type: String,
        content_text: Option<String>,
        visibility: String,
        lamport_clock: i64,
        created_at: i64,
        signature: Vec<u8>,
        #[serde(default)]
        media_hashes: Vec<String>,
        timestamp: i64,
        request_signature: Vec<u8>,
        #[serde(default)]
        media_items: Vec<WallPostMediaItemProto>,
    },
    GetWallPosts {
        requester_peer_id: String,
        author_peer_id: String,
        since_lamport_clock: i64,
        limit: u32,
        timestamp: i64,
        signature: Vec<u8>,
        #[serde(default)]
        grant_proof: Option<WallReadGrantProof>,
    },
    RegisterWallReadGrant {
        grant: WallReadGrantProof,
    },
    RevokeWallReadGrant {
        grant_id: String,
        issuer_peer_id: String,
        lamport_clock: u64,
        revoked_at: i64,
        signature: Vec<u8>,
    },
    DeleteWallPost {
        author_peer_id: String,
        post_id: String,
        lamport_clock: u64,
        deleted_at: i64,
        signature: Vec<u8>,
    },
    SubmitWallSocialEvent {
        event: WallSocialEventItemProto,
        timestamp: i64,
        request_signature: Vec<u8>,
    },
    GetWallSocialEvents {
        requester_peer_id: String,
        author_peer_id: String,
        post_ids: Vec<String>,
        after_timestamp: i64,
        limit: u32,
        timestamp: i64,
        signature: Vec<u8>,
    },
}

impl BoardSyncRequest {
    fn requires_community(&self) -> bool {
        matches!(
            self,
            Self::ListBoards { .. }
                | Self::GetBoardPosts { .. }
                | Self::SubmitPost { .. }
                | Self::DeletePost { .. }
        )
    }
}

fn request_enabled_in_mode(request: &BoardSyncRequest, community_mode: bool) -> bool {
    community_mode || !request.requires_community()
}

/// Board info in responses
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BoardInfoProto {
    pub board_id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
}

/// Board post in responses
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BoardPostInfoProto {
    pub post_id: String,
    pub board_id: String,
    pub author_peer_id: String,
    pub author_display_name: Option<String>,
    pub content_type: String,
    pub content_text: Option<String>,
    pub lamport_clock: u64,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
    pub signature: Vec<u8>,
}

/// Media metadata attached to a wall post
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WallPostMediaItemProto {
    pub media_hash: String,
    pub media_type: String,
    pub mime_type: String,
    pub file_name: String,
    pub file_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub sort_order: i32,
    pub signature: Vec<u8>,
}

/// Wall post data in responses
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WallPostData {
    pub post_id: String,
    pub author_peer_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub visibility: String,
    pub lamport_clock: i64,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
    pub signature: Vec<u8>,
    #[serde(default)]
    pub media_hashes: Vec<String>,
    pub stored_at: i64,
    #[serde(default)]
    pub media_items: Vec<WallPostMediaItemProto>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WallSocialEventItemProto {
    pub event_id: String,
    pub event_type: String,
    pub post_id: String,
    pub actor_peer_id: String,
    pub author_name: Option<String>,
    pub comment_id: Option<String>,
    pub content: Option<String>,
    pub reaction_type: Option<String>,
    pub timestamp: i64,
    pub payload_cbor: Vec<u8>,
    pub signature: Vec<u8>,
}

/// Board sync response (wire protocol)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BoardSyncResponse {
    RelayAuthChallenge {
        challenge: auth::AuthChallenge,
    },
    RelaySession {
        token: String,
    },
    RelayNameRegistered {
        claim: name_registration::NameClaim,
    },
    IntroductionAccepted {
        request_id: String,
        retry_after: u32,
    },
    IntroductionWork {
        challenge: abuse::WorkChallenge,
    },
    Introductions {
        envelopes: Vec<introduction::QueuedEnvelope>,
    },
    IntroductionsAcked {
        count: u32,
    },
    BoardList {
        boards: Vec<BoardInfoProto>,
        relay_peer_id: String,
    },
    BoardPosts {
        board_id: String,
        posts: Vec<BoardPostInfoProto>,
        has_more: bool,
    },
    PostAccepted {
        post_id: String,
    },
    PeerRegistered {
        peer_id: String,
    },
    PostDeleted {
        post_id: String,
    },
    WallPosts {
        posts: Vec<WallPostData>,
        has_more: bool,
    },
    WallPostStored {
        post_id: String,
    },
    WallPostDeleted {
        post_id: String,
    },
    WallReadGrantStored {
        grant_id: String,
    },
    WallReadGrantRevoked {
        grant_id: String,
    },
    WallSocialEventStored {
        event_id: String,
    },
    WallSocialEvents {
        events: Vec<WallSocialEventItemProto>,
        has_more: bool,
        next_timestamp: i64,
    },
    Error {
        error: String,
    },
}

/// Harbor Relay Server - Enables NAT traversal and optionally hosts community boards
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// DNS namespace used for relay-unique names
    #[arg(long, default_value = "harbor.social")]
    identity_namespace: String,
    /// Port to listen on
    #[arg(short, long, default_value_t = 4001)]
    port: u16,

    /// Public IP address to announce (optional, for NAT scenarios)
    #[arg(long)]
    announce_ip: Option<Ipv4Addr>,

    #[command(flatten)]
    resource_limits: ResourceLimitArgs,

    /// Path to the persistent identity key (generated if missing)
    #[arg(long, default_value_t = default_identity_path())]
    identity_key_path: String,

    /// Enable community mode (boards, posts, SQLite storage)
    #[arg(long, default_value_t = false)]
    community: bool,

    /// Directory for persistent identity and optional community storage
    #[arg(long)]
    data_dir: Option<String>,

    /// Community name for this relay (only used with --community)
    #[arg(long, default_value = "Harbor Community")]
    community_name: String,
}

/// Combined behaviour for the relay server
#[derive(NetworkBehaviour)]
struct RelayServerBehaviour {
    relay: relay::Behaviour,
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    board_sync: Toggle<request_response::cbor::Behaviour<BoardSyncRequest, BoardSyncResponse>>,
}

fn default_identity_path() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/harbor-relay/id.key")
        .display()
        .to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let resource_limits = ResourceLimits::try_from(args.resource_limits.clone())?;

    // Warn if community-only options are used without --community
    if !args.community {
        if args.community_name != "Harbor Community" {
            warn!("--community-name has no effect without --community");
        }
    }

    info!("Starting Harbor Relay Server...");
    if args.community {
        info!("Mode: Community (boards + relay)");
        info!("Community: {}", args.community_name);
    } else {
        info!("Mode: Relay only (NAT traversal pass-through)");
    }
    info!("Port: {}", args.port);
    info!(?resource_limits, "Effective relay resource limits");

    let keypair = load_or_generate_identity(Path::new(&args.identity_key_path))?;
    info!("Using identity key at {}", args.identity_key_path);

    // Relay-scoped identity, introductions, and wall data are available in
    // every mode and require durable state. `--community` additionally
    // enables the community-board request variants on the shared protocol.
    let db_path = if let Some(ref data_dir) = args.data_dir {
        fs::create_dir_all(data_dir)?;
        format!("{}/relay.db", data_dir)
    } else {
        let default_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/harbor-relay");
        fs::create_dir_all(&default_dir)?;
        default_dir.join("relay.db").display().to_string()
    };

    let relay_db = RelayDatabase::open_with_max_bytes(&db_path, resource_limits.max_storage_bytes)?;
    relay_db.configure_retention(
        db::RetentionLimits {
            record_retention_secs: resource_limits.record_retention_secs as i64,
            max_known_peers: resource_limits.max_known_peers as u64,
            max_posts: resource_limits.max_posts as u64,
            max_grants: resource_limits.max_grants as u64,
            max_introductions: resource_limits.max_introductions as u64,
            max_social_events: resource_limits.max_social_events as u64,
        },
        chrono::Utc::now().timestamp(),
    )?;
    let identity_database = relay_db.clone();
    let board_service = Some(BoardService::new(relay_db, args.community_name.clone()));
    info!("Persistent relay database initialized at {}", db_path);

    let ed = keypair
        .clone()
        .try_into_ed25519()
        .map_err(|_| "relay identity must be Ed25519")?;
    let ed_bytes = ed.to_bytes();
    let mut secret = [0u8; 32];
    secret.copy_from_slice(&ed_bytes[..32]);
    let mut identity_transport = Some(IdentityTransport {
        auth: auth::AuthService::new_with_limits(
            args.identity_namespace.clone(),
            "relay-key-1",
            keypair.clone(),
            auth::StateLimits {
                max_entries: resource_limits.max_ephemeral_entries,
                replay_retention_secs: resource_limits.ephemeral_retention_secs as i64,
            },
        ),
        abuse: abuse::AbuseGuard::new_with_capacities(
            abuse::Limits {
                peer: resource_limits.abuse_peer_limit,
                network: resource_limits.abuse_network_limit,
                target: resource_limits.abuse_target_limit,
                action: resource_limits.abuse_action_limit,
                global: resource_limits.abuse_global_limit,
                window_secs: resource_limits.abuse_window_secs as i64,
            },
            abuse::CapacityLimits {
                max_entries: resource_limits.max_ephemeral_entries,
                retention_secs: resource_limits.ephemeral_retention_secs as i64,
            },
        ),
        admission: admission::SourceAdmissionGuard::new(admission::Limits {
            per_source: resource_limits.abuse_network_limit,
            global: resource_limits.abuse_global_limit,
            max_sources: resource_limits.max_admission_sources,
            window_secs: resource_limits.abuse_window_secs as i64,
        }),
        database: identity_database,
        relay_name: args.identity_namespace.clone(),
        relay_key_id: "relay-key-1".into(),
        relay_signing_key: ed25519_dalek::SigningKey::from_bytes(&secret),
    });

    // The shared identity/board protocol is exposed in every mode.
    let mut rate_limiter = PeerRateLimiter::new(
        resource_limits.rate_limit_max_requests,
        Duration::from_secs(resource_limits.rate_limit_window_secs),
        resource_limits.max_admission_sources,
    );
    info!(
        "Identity protocol rate limiter enabled: {} requests per {}s window",
        resource_limits.rate_limit_max_requests, resource_limits.rate_limit_window_secs
    );
    let mut relay_read_guard = RelayReadGuard::default();

    let community_mode = args.community;

    // Build the swarm
    let mut swarm = SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(|_| {
            let local_peer_id = PeerId::from(keypair.public());
            let local_public_key = keypair.public();

            let relay_config = relay::Config {
                max_reservations: resource_limits.max_reservations,
                max_reservations_per_peer: resource_limits.max_reservations_per_peer,
                reservation_duration: Duration::from_secs(
                    resource_limits.reservation_duration_secs,
                ),
                reservation_rate_limiters: Vec::new(),
                max_circuits: resource_limits.max_circuits,
                max_circuits_per_peer: resource_limits.max_circuits_per_peer,
                max_circuit_duration: Duration::from_secs(
                    resource_limits.max_circuit_duration_secs,
                ),
                max_circuit_bytes: resource_limits.max_circuit_bytes,
                circuit_src_rate_limiters: Vec::new(),
            }
            .reservation_rate_per_peer(
                NonZeroU32::new(resource_limits.reservation_admission_per_peer)
                    .expect("validated nonzero reservation peer admission limit"),
                Duration::from_secs(resource_limits.admission_window_secs),
            )
            .reservation_rate_per_ip(
                NonZeroU32::new(resource_limits.reservation_admission_per_ip)
                    .expect("validated nonzero reservation IP admission limit"),
                Duration::from_secs(resource_limits.admission_window_secs),
            )
            .circuit_src_per_peer(
                NonZeroU32::new(resource_limits.circuit_admission_per_peer)
                    .expect("validated nonzero circuit peer admission limit"),
                Duration::from_secs(resource_limits.admission_window_secs),
            )
            .circuit_src_per_ip(
                NonZeroU32::new(resource_limits.circuit_admission_per_ip)
                    .expect("validated nonzero circuit IP admission limit"),
                Duration::from_secs(resource_limits.admission_window_secs),
            );

            let relay = relay::Behaviour::new(local_peer_id, relay_config);

            let ping = ping::Behaviour::new(
                ping::Config::new()
                    .with_interval(Duration::from_secs(15))
                    .with_timeout(Duration::from_secs(60)),
            );

            let identify = identify::Behaviour::new(identify::Config::new(
                "/harbor-relay/1.0.0".to_string(),
                local_public_key,
            ));

            // Identity and wall operations share this protocol with optional
            // community boards, so the transport must exist in every mode.
            let board_sync = Toggle::from(Some(request_response::cbor::Behaviour::new(
                [(
                    StreamProtocol::new(BOARD_SYNC_PROTOCOL),
                    ProtocolSupport::Full,
                )],
                request_response::Config::default(),
            )));

            RelayServerBehaviour {
                relay,
                ping,
                identify,
                board_sync,
            }
        })?
        .with_swarm_config(|cfg| {
            cfg.with_idle_connection_timeout(Duration::from_secs(
                resource_limits.idle_connection_timeout_secs,
            ))
        })
        .build();

    let local_peer_id = *swarm.local_peer_id();
    let mut source_networks: HashMap<PeerId, String> = HashMap::new();
    info!("Local Peer ID: {}", local_peer_id);

    // Listen on all interfaces
    let listen_addr_tcp: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", args.port).parse()?;
    let listen_addr_quic: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", args.port).parse()?;

    swarm.listen_on(listen_addr_tcp.clone())?;
    swarm.listen_on(listen_addr_quic.clone())?;

    info!("Listening on TCP: {}", listen_addr_tcp);
    info!("Listening on QUIC: {}", listen_addr_quic);

    // If announce IP is provided, add external addresses
    if let Some(announce_ip) = args.announce_ip {
        let external_addresses =
            validated_external_addresses(announce_ip, args.port, &local_peer_id)
                .map_err(|error| format!("invalid --announce-ip: {error}"))?;
        for address in &external_addresses {
            swarm.add_external_address(address.clone());
        }

        info!("========================================");
        info!("YOUR RELAY ADDRESSES:");
        info!("  TCP:  {}", external_addresses[0]);
        info!("  QUIC: {}", external_addresses[1]);
        info!("========================================");
        info!("Copy the TCP address and paste it into Harbor!");
    } else {
        info!("========================================");
        info!("Peer ID: {}", local_peer_id);
        info!("Tip: Use --announce-ip YOUR_PUBLIC_IP to see full relay address");
        info!("========================================");
    }

    // Periodic cleanup timer for the rate limiter
    let mut cleanup_interval = tokio::time::interval(Duration::from_secs(
        resource_limits.rate_limiter_cleanup_interval_secs,
    ));
    // The first tick completes immediately; consume it so we don't
    // run cleanup at startup.
    cleanup_interval.tick().await;

    // Run the event loop
    loop {
        tokio::select! {
            _ = cleanup_interval.tick() => {
                rate_limiter.cleanup_stale_entries();
                if let Some(ref mut identity) = identity_transport {
                    let now = chrono::Utc::now().timestamp();
                    identity.admission.prune(now);
                    if let Err(error) = identity.database.enforce_retention(now) {
                        warn!(%error, "Relay retention cleanup failed");
                    }
                }
            }
            event = swarm.select_next_some() => match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("Listening on: {}/p2p/{}", address, local_peer_id);
                }
                SwarmEvent::Behaviour(RelayServerBehaviourEvent::Relay(event)) => {
                    info!("Relay event: {:?}", event);
                }
                SwarmEvent::Behaviour(RelayServerBehaviourEvent::Identify(identify::Event::Received {
                    peer_id,
                    info,
                    ..
                })) => {
                    info!("Identified peer {}: {}", peer_id, info.agent_version);
                }
                SwarmEvent::Behaviour(RelayServerBehaviourEvent::BoardSync(
                    request_response::Event::Message { peer, message, .. },
                )) => match message {
                    request_response::Message::Request {
                        request, channel, ..
                    } => {
                        if let Some(ref service) = board_service {
                            // Rate-limit every request, including variants that are
                            // disabled in this deployment mode.
                            let response = if let Err(rate_limit_error) =
                                rate_limiter.check_rate_limit(&peer)
                            {
                                BoardSyncResponse::Error {
                                    error: rate_limit_error,
                                }
                            } else if !request_enabled_in_mode(&request, community_mode) {
                                BoardSyncResponse::Error {
                                    error: "COMMUNITY_BOARDS_DISABLED".into(),
                                }
                            } else {
                                handle_board_request(
                                    service,
                                    &mut relay_read_guard,
                                    identity_transport.as_mut(),
                                    source_networks
                                        .get(&peer)
                                        .map(String::as_str)
                                        .unwrap_or("unknown"),
                                    &local_peer_id,
                                    &peer,
                                    request,
                                )
                            };

                            if let BoardSyncResponse::IntroductionAccepted{request_id,..}=&response{tokio::time::sleep(response_delay(request_id)).await;}

                            if let Err(send_error) = swarm
                                .behaviour_mut()
                                .board_sync
                                .as_mut()
                                .expect("identity protocol is enabled in every mode")
                                .send_response(channel, response)
                            {
                                warn!("Failed to send board sync response: {:?}", send_error);
                            }
                        }
                    }
                    request_response::Message::Response { .. } => {
                        // Relay server doesn't send requests, so we shouldn't get responses
                    }
                },
                SwarmEvent::ConnectionEstablished { peer_id, connection_id, endpoint, .. } => {
                    if source_networks.contains_key(&peer_id)
                        || source_networks.len() < resource_limits.max_admission_sources
                    {
                        source_networks.insert(peer_id,source_bucket(endpoint.get_remote_address()));
                    } else {
                        warn!(%peer_id, "Source attribution capacity reached; using shared unknown-source budget");
                    }
                    info!("Connection established with: {} via {:?} ({:?})", peer_id, connection_id, endpoint);
                }
                SwarmEvent::ConnectionClosed { peer_id, connection_id, cause, endpoint, .. } => {
                    source_networks.remove(&peer_id);
                    info!("Connection closed with: {} via {:?} ({:?}), cause: {:?}", peer_id, connection_id, endpoint, cause);
                }
                _ => {}
            }
        }
    }
}

fn begin_relay_read(
    guard: &mut RelayReadGuard,
    peer: &PeerId,
    requester_peer_id: &str,
    timestamp: i64,
    signature: &[u8],
    server_now: i64,
) -> Result<ReadReplayToken, BoardSyncResponse> {
    guard
        .authorize(peer, requester_peer_id, timestamp, signature, server_now)
        .map_err(|error| BoardSyncResponse::Error {
            error: error.code().to_string(),
        })
}

fn deny_relay_read(
    guard: &mut RelayReadGuard,
    token: ReadReplayToken,
    internal_error: String,
) -> BoardSyncResponse {
    guard.discard(token);
    warn!("Relay read denied: {}", internal_error);
    let stable = match internal_error.as_str() {
        board_service::RELAY_READ_DATABASE
        | board_service::RELAY_READ_SIGNATURE_INVALID
        | board_service::RELAY_READ_GRANT_INVALID
        | board_service::RELAY_READ_SCOPE_UNSUPPORTED => internal_error,
        _ if internal_error.starts_with("RELAY_PEER_") => internal_error,
        _ => board_service::RELAY_READ_DENIED.to_string(),
    };
    BoardSyncResponse::Error { error: stable }
}

fn handle_board_request(
    service: &BoardService,
    relay_read_guard: &mut RelayReadGuard,
    mut identity: Option<&mut IdentityTransport>,
    source_network: &str,
    local_peer_id: &PeerId,
    peer: &PeerId,
    request: BoardSyncRequest,
) -> BoardSyncResponse {
    if let Some(state) = identity.as_deref_mut() {
        let now = chrono::Utc::now().timestamp();
        if let Err(error) = state
            .admission
            .check_and_record(source_network, &peer.to_string(), now)
        {
            warn!(%error, %source_network, "Relay source admission rejected request");
            return match &request {
                // Preserve the 0845 indistinguishable submission response. A
                // source-budget rejection must not become a target oracle.
                BoardSyncRequest::SubmitIntroduction { envelope, .. } => {
                    BoardSyncResponse::IntroductionAccepted {
                        request_id: envelope.request_id.clone(),
                        retry_after: 3_600,
                    }
                }
                _ => BoardSyncResponse::Error {
                    error: error.to_string(),
                },
            };
        }
        if let Err(error) = state.database.enforce_retention(now) {
            warn!(%error, "Relay retention cleanup failed before request");
        }
    }
    match request {
        BoardSyncRequest::RelayAuthChallenge { peer_id, audience } => {
            let Some(state) = identity else {
                return BoardSyncResponse::Error {
                    error: "IDENTITY_SERVICE_DISABLED".into(),
                };
            };
            if peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "AUTH_PEER_MISMATCH".into(),
                };
            }
            match state
                .auth
                .issue_challenge(peer, &audience, chrono::Utc::now().timestamp())
            {
                Ok(challenge) => BoardSyncResponse::RelayAuthChallenge { challenge },
                Err(_) => BoardSyncResponse::Error {
                    error: "AUTH_CHALLENGE_REJECTED".into(),
                },
            }
        }
        BoardSyncRequest::RelayAuthComplete {
            challenge,
            public_key,
            signature,
        } => {
            let Some(state) = identity else {
                return BoardSyncResponse::Error {
                    error: "IDENTITY_SERVICE_DISABLED".into(),
                };
            };
            if challenge.peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "AUTH_PEER_MISMATCH".into(),
                };
            }
            match state.auth.complete(
                &challenge,
                &public_key,
                &signature,
                chrono::Utc::now().timestamp(),
            ) {
                Ok(token) => BoardSyncResponse::RelaySession { token },
                Err(_) => BoardSyncResponse::Error {
                    error: "AUTH_RESPONSE_REJECTED".into(),
                },
            }
        }
        BoardSyncRequest::RegisterRelayName {
            session_token,
            signed_request,
        } => {
            let Some(state) = identity else {
                return BoardSyncResponse::Error {
                    error: "IDENTITY_SERVICE_DISABLED".into(),
                };
            };
            let Ok(authenticated) = state.auth.authorize(
                &session_token,
                "name:register",
                chrono::Utc::now().timestamp(),
            ) else {
                return BoardSyncResponse::Error {
                    error: "NAME_REGISTRATION_REJECTED".into(),
                };
            };
            if authenticated != *peer || signed_request.request.peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "NAME_REGISTRATION_REJECTED".into(),
                };
            }
            let result = state.database.with_connection(|conn| {
                name_registration::register(
                    conn,
                    &state.relay_name,
                    &state.relay_key_id,
                    &state.relay_signing_key,
                    signed_request,
                    chrono::Utc::now().timestamp(),
                )
            });
            match result {
                Ok(claim) => BoardSyncResponse::RelayNameRegistered { claim },
                Err(_) => BoardSyncResponse::Error {
                    error: "NAME_REGISTRATION_REJECTED".into(),
                },
            }
        }
        BoardSyncRequest::SubmitIntroduction {
            session_token,
            envelope,
        } => {
            let Some(state) = identity else {
                return BoardSyncResponse::Error {
                    error: "IDENTITY_SERVICE_DISABLED".into(),
                };
            };
            let now = chrono::Utc::now().timestamp();
            if state
                .auth
                .authorize(&session_token, "introduce", now)
                .ok()
                .as_ref()
                != Some(peer)
            {
                return BoardSyncResponse::Error {
                    error: "INTRODUCTION_UNAVAILABLE".into(),
                };
            }
            let response = state.database.with_connection(|conn| {
                introduction::IntroductionService::new(conn, &state.auth, &mut state.abuse).map(
                    |mut s| {
                        s.submit_with_outcome(
                            &session_token,
                            source_network,
                            envelope,
                            chrono::Utc::now().timestamp(),
                            false,
                        )
                    },
                )
            });
            match response {
                Ok(outcome) => {
                    let admission_code = outcome.code.as_str();
                    let request_id = outcome.response.request_id.clone();
                    match outcome.code {
                        introduction::AdmissionCode::StorageFailure
                        | introduction::AdmissionCode::CapacityRejected => warn!(
                            admission_code,
                            request_id, "Introduction admission was not queued"
                        ),
                        _ => debug!(
                            admission_code,
                            request_id, "Introduction admission decision"
                        ),
                    }
                    BoardSyncResponse::IntroductionAccepted {
                        request_id: outcome.response.request_id,
                        retry_after: outcome.response.retry_after,
                    }
                }
                Err(_) => BoardSyncResponse::Error {
                    error: "INTRODUCTION_UNAVAILABLE".into(),
                },
            }
        }
        BoardSyncRequest::RequestIntroductionWork {
            session_token,
            target,
        } => {
            let Some(state) = identity else {
                return BoardSyncResponse::Error {
                    error: "IDENTITY_SERVICE_DISABLED".into(),
                };
            };
            let now = chrono::Utc::now().timestamp();
            let Ok(requester) = state.auth.authorize(&session_token, "introduce", now) else {
                return BoardSyncResponse::Error {
                    error: "INTRODUCTION_WORK_REJECTED".into(),
                };
            };
            if requester != *peer {
                return BoardSyncResponse::Error {
                    error: "INTRODUCTION_WORK_REJECTED".into(),
                };
            }
            let signing_key = state.auth.signing_key();
            let delivery_key = opaque_delivery_key(&state.database, &target, &signing_key);
            match state.abuse.issue_with_delivery_key(
                &state.relay_name,
                &requester.to_string(),
                &target,
                "introduce",
                now,
                &state.relay_key_id,
                &signing_key,
                delivery_key,
            ) {
                Ok(challenge) => BoardSyncResponse::IntroductionWork { challenge },
                Err(_) => BoardSyncResponse::Error {
                    error: "INTRODUCTION_WORK_REJECTED".into(),
                },
            }
        }
        BoardSyncRequest::FetchIntroductions {
            session_token,
            limit,
        } => {
            let Some(state) = identity else {
                return BoardSyncResponse::Error {
                    error: "IDENTITY_SERVICE_DISABLED".into(),
                };
            };
            let now = chrono::Utc::now().timestamp();
            if state
                .auth
                .authorize(&session_token, "introductions:read", now)
                .ok()
                .as_ref()
                != Some(peer)
            {
                return BoardSyncResponse::Error {
                    error: "INTRODUCTION_FETCH_REJECTED".into(),
                };
            }
            let response = state.database.with_connection(|conn| {
                introduction::IntroductionService::new(conn, &state.auth, &mut state.abuse)
                    .map_err(|e| e.to_string())?
                    .take(&session_token, chrono::Utc::now().timestamp(), limit)
            });
            match response {
                Ok(envelopes) => BoardSyncResponse::Introductions { envelopes },
                Err(_) => BoardSyncResponse::Error {
                    error: "INTRODUCTION_FETCH_REJECTED".into(),
                },
            }
        }
        BoardSyncRequest::AckIntroductions {
            session_token,
            request_ids,
        } => {
            let Some(state) = identity else {
                return BoardSyncResponse::Error {
                    error: "IDENTITY_SERVICE_DISABLED".into(),
                };
            };
            let now = chrono::Utc::now().timestamp();
            if state
                .auth
                .authorize(&session_token, "introductions:read", now)
                .ok()
                .as_ref()
                != Some(peer)
            {
                return BoardSyncResponse::Error {
                    error: "INTRODUCTION_ACK_REJECTED".into(),
                };
            }
            let result = state.database.with_connection(|conn| {
                introduction::IntroductionService::new(conn, &state.auth, &mut state.abuse)
                    .map_err(|e| e.to_string())?
                    .acknowledge(&session_token, &request_ids, now)
            });
            match result {
                Ok(count) => BoardSyncResponse::IntroductionsAcked { count },
                Err(_) => BoardSyncResponse::Error {
                    error: "INTRODUCTION_ACK_REJECTED".into(),
                },
            }
        }
        BoardSyncRequest::RegisterPeer {
            peer_id,
            public_key,
            display_name,
            timestamp,
            signature,
        } => {
            if peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "RELAY_PEER_TRANSPORT_MISMATCH".to_string(),
                };
            }
            let server_now = chrono::Utc::now().timestamp();
            match service.process_register_peer_at(
                &peer_id,
                &public_key,
                &display_name,
                timestamp,
                &signature,
                server_now,
            ) {
                Ok(_) => BoardSyncResponse::PeerRegistered { peer_id },
                Err(e) => BoardSyncResponse::Error { error: e },
            }
        }
        BoardSyncRequest::ListBoards {
            requester_peer_id,
            timestamp,
            signature,
        } => {
            let server_now = chrono::Utc::now().timestamp();
            let token = match begin_relay_read(
                relay_read_guard,
                peer,
                &requester_peer_id,
                timestamp,
                &signature,
                server_now,
            ) {
                Ok(token) => token,
                Err(response) => return response,
            };
            match service.process_list_boards(&requester_peer_id, timestamp, &signature) {
                Ok(boards) => {
                    info!(
                        "Serving board list for community: {}",
                        service.community_name()
                    );
                    BoardSyncResponse::BoardList {
                        boards: boards
                            .into_iter()
                            .map(|b| BoardInfoProto {
                                board_id: b.board_id,
                                name: b.name,
                                description: b.description,
                                is_default: b.is_default,
                            })
                            .collect(),
                        relay_peer_id: local_peer_id.to_string(),
                    }
                }
                Err(error) => deny_relay_read(relay_read_guard, token, error),
            }
        }
        BoardSyncRequest::GetBoardPosts {
            requester_peer_id,
            board_id,
            after_timestamp,
            limit,
            timestamp,
            signature,
        } => {
            let server_now = chrono::Utc::now().timestamp();
            let token = match begin_relay_read(
                relay_read_guard,
                peer,
                &requester_peer_id,
                timestamp,
                &signature,
                server_now,
            ) {
                Ok(token) => token,
                Err(response) => return response,
            };
            match service.process_get_board_posts(
                &requester_peer_id,
                &board_id,
                after_timestamp,
                limit,
                timestamp,
                &signature,
            ) {
                Ok((posts, has_more)) => BoardSyncResponse::BoardPosts {
                    board_id,
                    posts: posts
                        .into_iter()
                        .map(|p| BoardPostInfoProto {
                            post_id: p.post_id,
                            board_id: p.board_id,
                            author_peer_id: p.author_peer_id,
                            author_display_name: p.author_display_name,
                            content_type: p.content_type,
                            content_text: p.content_text,
                            lamport_clock: p.lamport_clock,
                            created_at: p.created_at,
                            deleted_at: p.deleted_at,
                            signature: p.signature,
                        })
                        .collect(),
                    has_more,
                },
                Err(error) => deny_relay_read(relay_read_guard, token, error),
            }
        }
        BoardSyncRequest::GetOlderBoardPosts {
            requester_peer_id,
            board_id,
            before,
            limit,
            timestamp,
            signature,
        } => {
            let server_now = chrono::Utc::now().timestamp();
            let token = match begin_relay_read(
                relay_read_guard,
                peer,
                &requester_peer_id,
                timestamp,
                &signature,
                server_now,
            ) {
                Ok(token) => token,
                Err(response) => return response,
            };
            match service.process_get_older_board_posts(
                &requester_peer_id,
                &board_id,
                before.as_ref(),
                limit,
                timestamp,
                &signature,
            ) {
                Ok((posts, has_more)) => BoardSyncResponse::BoardPosts {
                    board_id,
                    posts: posts
                        .into_iter()
                        .map(|p| BoardPostInfoProto {
                            post_id: p.post_id,
                            board_id: p.board_id,
                            author_peer_id: p.author_peer_id,
                            author_display_name: p.author_display_name,
                            content_type: p.content_type,
                            content_text: p.content_text,
                            lamport_clock: p.lamport_clock,
                            created_at: p.created_at,
                            deleted_at: p.deleted_at,
                            signature: p.signature,
                        })
                        .collect(),
                    has_more,
                },
                Err(error) => deny_relay_read(relay_read_guard, token, error),
            }
        }
        BoardSyncRequest::SubmitPost {
            post_id,
            board_id,
            author_peer_id,
            content_type,
            content_text,
            lamport_clock,
            created_at,
            signature,
        } => {
            if author_peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "RELAY_POST_TRANSPORT_MISMATCH".to_string(),
                };
            }
            match service.process_submit_post(
                &post_id,
                &board_id,
                &author_peer_id,
                &content_type,
                content_text.as_deref(),
                lamport_clock,
                created_at,
                &signature,
            ) {
                Ok(()) => BoardSyncResponse::PostAccepted { post_id },
                Err(e) => BoardSyncResponse::Error { error: e },
            }
        }
        BoardSyncRequest::DeletePost {
            post_id,
            author_peer_id,
            timestamp,
            signature,
        } => {
            if author_peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "RELAY_POST_TRANSPORT_MISMATCH".to_string(),
                };
            }
            match service.process_delete_post(&post_id, &author_peer_id, timestamp, &signature) {
                Ok(()) => BoardSyncResponse::PostDeleted { post_id },
                Err(e) => BoardSyncResponse::Error { error: e },
            }
        }
        BoardSyncRequest::SubmitWallPost {
            author_peer_id,
            post_id,
            content_type,
            content_text,
            visibility,
            lamport_clock,
            created_at,
            signature,
            media_hashes,
            timestamp,
            request_signature,
            media_items,
        } => {
            if author_peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "RELAY_POST_TRANSPORT_MISMATCH".to_string(),
                };
            }
            match service.process_submit_wall_post(
                &author_peer_id,
                &post_id,
                &content_type,
                content_text.as_deref(),
                &visibility,
                lamport_clock,
                created_at,
                &signature,
                &media_hashes,
                timestamp,
                &request_signature,
                &media_items,
            ) {
                Ok(()) => BoardSyncResponse::WallPostStored { post_id },
                Err(e) => BoardSyncResponse::Error { error: e },
            }
        }
        BoardSyncRequest::GetWallPosts {
            requester_peer_id,
            author_peer_id,
            since_lamport_clock,
            limit,
            timestamp,
            signature,
            grant_proof,
        } => {
            let server_now = chrono::Utc::now().timestamp();
            let token = match begin_relay_read(
                relay_read_guard,
                peer,
                &requester_peer_id,
                timestamp,
                &signature,
                server_now,
            ) {
                Ok(token) => token,
                Err(response) => return response,
            };
            match service.process_get_wall_posts_at(
                &requester_peer_id,
                &author_peer_id,
                since_lamport_clock,
                limit,
                timestamp,
                &signature,
                grant_proof.as_ref(),
                server_now,
            ) {
                Ok((posts, has_more, media_map)) => {
                    // Build a lookup from post_id -> media items
                    let media_lookup: std::collections::HashMap<
                        String,
                        Vec<WallPostMediaItemProto>,
                    > = media_map
                        .into_iter()
                        .map(|(post_id, items)| {
                            let protos = items
                                .into_iter()
                                .map(|m| WallPostMediaItemProto {
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
                                .collect();
                            (post_id, protos)
                        })
                        .collect();

                    BoardSyncResponse::WallPosts {
                        posts: posts
                            .into_iter()
                            .map(|p| {
                                let media_items =
                                    media_lookup.get(&p.post_id).cloned().unwrap_or_default();
                                let media_hashes =
                                    media_items.iter().map(|m| m.media_hash.clone()).collect();
                                WallPostData {
                                    post_id: p.post_id,
                                    author_peer_id: p.author_peer_id,
                                    content_type: p.content_type,
                                    content_text: p.content_text,
                                    visibility: p.visibility,
                                    lamport_clock: p.lamport_clock,
                                    created_at: p.created_at,
                                    deleted_at: p.deleted_at,
                                    signature: p.signature,
                                    media_hashes,
                                    stored_at: p.stored_at,
                                    media_items,
                                }
                            })
                            .collect(),
                        has_more,
                    }
                }
                Err(error) => deny_relay_read(relay_read_guard, token, error),
            }
        }
        BoardSyncRequest::RegisterWallReadGrant { grant } => {
            if grant.issuer_peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "authenticated peer is not the grant issuer".to_string(),
                };
            }
            let grant_id = grant.grant_id.clone();
            match service.process_wall_read_grant(&grant) {
                Ok(()) => BoardSyncResponse::WallReadGrantStored { grant_id },
                Err(e) => BoardSyncResponse::Error { error: e },
            }
        }
        BoardSyncRequest::RevokeWallReadGrant {
            grant_id,
            issuer_peer_id,
            lamport_clock,
            revoked_at,
            signature,
        } => {
            if issuer_peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "authenticated peer is not the revocation issuer".to_string(),
                };
            }
            match service.process_wall_read_revoke(
                &grant_id,
                &issuer_peer_id,
                lamport_clock,
                revoked_at,
                &signature,
            ) {
                Ok(()) => BoardSyncResponse::WallReadGrantRevoked { grant_id },
                Err(e) => BoardSyncResponse::Error { error: e },
            }
        }
        BoardSyncRequest::DeleteWallPost {
            author_peer_id,
            post_id,
            lamport_clock,
            deleted_at,
            signature,
        } => {
            if author_peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "RELAY_POST_TRANSPORT_MISMATCH".to_string(),
                };
            }
            match service.process_delete_wall_post(
                &author_peer_id,
                &post_id,
                lamport_clock,
                deleted_at,
                &signature,
            ) {
                Ok(()) => BoardSyncResponse::WallPostDeleted { post_id },
                Err(e) => BoardSyncResponse::Error { error: e },
            }
        }
        BoardSyncRequest::SubmitWallSocialEvent {
            event,
            timestamp,
            request_signature,
        } => {
            if event.actor_peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "actor_peer_id mismatch".to_string(),
                };
            }
            let event_id = event.event_id.clone();
            match service.process_submit_wall_social_event(&event, timestamp, &request_signature) {
                Ok(()) => BoardSyncResponse::WallSocialEventStored { event_id },
                Err(e) => BoardSyncResponse::Error { error: e },
            }
        }
        BoardSyncRequest::GetWallSocialEvents {
            requester_peer_id,
            author_peer_id,
            post_ids,
            after_timestamp,
            limit,
            timestamp,
            signature,
        } => {
            let server_now = chrono::Utc::now().timestamp();
            let token = match begin_relay_read(
                relay_read_guard,
                peer,
                &requester_peer_id,
                timestamp,
                &signature,
                server_now,
            ) {
                Ok(token) => token,
                Err(response) => return response,
            };
            match service.process_get_wall_social_events_at(
                &requester_peer_id,
                &author_peer_id,
                &post_ids,
                after_timestamp,
                limit,
                timestamp,
                &signature,
                server_now,
            ) {
                Ok((rows, has_more, next_timestamp)) => BoardSyncResponse::WallSocialEvents {
                    events: rows
                        .into_iter()
                        .map(|e| WallSocialEventItemProto {
                            event_id: e.event_id,
                            event_type: e.event_type,
                            post_id: e.post_id,
                            actor_peer_id: e.actor_peer_id,
                            author_name: e.author_name,
                            comment_id: e.comment_id,
                            content: e.content,
                            reaction_type: e.reaction_type,
                            timestamp: e.timestamp,
                            payload_cbor: e.payload_cbor,
                            signature: e.signature,
                        })
                        .collect(),
                    has_more,
                    next_timestamp,
                },
                Err(error) => deny_relay_read(relay_read_guard, token, error),
            }
        }
    }
}
