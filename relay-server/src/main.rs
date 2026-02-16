//! Harbor Relay Server
//!
//! A libp2p relay server that enables NAT traversal for Harbor chat app users.
//! Run with `--community` to enable community boards with SQLite storage.

mod board_service;
mod db;

use board_service::BoardService;
use clap::Parser;
use db::RelayDatabase;
use futures::StreamExt;
use libp2p::{
    identify, noise, ping, relay,
    request_response::{self, ProtocolSupport},
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, StreamProtocol, SwarmBuilder,
    identity::Keypair,
};
use std::collections::HashMap;
use std::fs;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

/// Board sync protocol version
const BOARD_SYNC_PROTOCOL: &str = "/harbor/board/1.0.0";

/// Default maximum requests per peer within the rate limit window
const DEFAULT_RATE_LIMIT_MAX_REQUESTS: u64 = 60;

/// Default rate limit window duration in seconds
const DEFAULT_RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// How often to purge stale entries from the rate limiter (in seconds)
const RATE_LIMITER_CLEANUP_INTERVAL_SECS: u64 = 300;

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
}

impl PeerRateLimiter {
    fn new(max_requests: u64, window_duration: Duration) -> Self {
        Self {
            peers: HashMap::new(),
            max_requests,
            window_duration,
        }
    }

    /// Check whether a peer is allowed to make a request.
    ///
    /// Returns `Ok(())` if the request is permitted, or `Err(message)` if the
    /// peer has exceeded their rate limit for the current window.
    fn check_rate_limit(&mut self, peer_id: &PeerId) -> Result<(), String> {
        let now = Instant::now();

        let (request_count, window_start) = self
            .peers
            .entry(*peer_id)
            .or_insert((0, now));

        // If the current window has expired, reset the counter
        if now.duration_since(*window_start) >= self.window_duration {
            *request_count = 0;
            *window_start = now;
        }

        // Check if the peer has exceeded the limit
        if *request_count >= self.max_requests {
            warn!(
                "Rate limit exceeded for peer {}: {} requests in {}s window",
                peer_id, request_count, self.window_duration.as_secs()
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
        let now = Instant::now();
        let stale_threshold = self.window_duration * 2;
        let initial_count = self.peers.len();

        self.peers
            .retain(|_peer_id, (_count, window_start)| {
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
        timestamp: i64,
        request_signature: Vec<u8>,
    },
    GetWallPosts {
        requester_peer_id: String,
        author_peer_id: String,
        since_lamport_clock: i64,
        limit: u32,
        timestamp: i64,
        signature: Vec<u8>,
    },
    DeleteWallPost {
        author_peer_id: String,
        post_id: String,
        timestamp: i64,
        signature: Vec<u8>,
    },
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
    pub signature: Vec<u8>,
    pub stored_at: i64,
}

/// Board sync response (wire protocol)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BoardSyncResponse {
    BoardList {
        boards: Vec<BoardInfoProto>,
        relay_peer_id: String,
    },
    BoardPosts {
        board_id: String,
        posts: Vec<BoardPostInfoProto>,
        has_more: bool,
    },
    PostAccepted { post_id: String },
    PeerRegistered { peer_id: String },
    PostDeleted { post_id: String },
    WallPosts {
        posts: Vec<WallPostData>,
        has_more: bool,
    },
    WallPostStored { post_id: String },
    WallPostDeleted { post_id: String },
    Error { error: String },
}

/// Harbor Relay Server - Enables NAT traversal and optionally hosts community boards
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value_t = 4001)]
    port: u16,

    /// Public IP address to announce (optional, for NAT scenarios)
    #[arg(long)]
    announce_ip: Option<Ipv4Addr>,

    /// Maximum number of relay reservations
    #[arg(long, default_value_t = 128)]
    max_reservations: usize,

    /// Maximum circuits per peer
    #[arg(long, default_value_t = 16)]
    max_circuits_per_peer: usize,

    /// Maximum total circuits
    #[arg(long, default_value_t = 512)]
    max_circuits: usize,

    /// Path to the persistent identity key (generated if missing)
    #[arg(long, default_value_t = default_identity_path())]
    identity_key_path: String,

    /// Enable community mode (boards, posts, SQLite storage)
    #[arg(long, default_value_t = false)]
    community: bool,

    /// Directory for SQLite database storage (only used with --community)
    #[arg(long)]
    data_dir: Option<String>,

    /// Community name for this relay (only used with --community)
    #[arg(long, default_value = "Harbor Community")]
    community_name: String,

    /// Maximum board sync requests per peer within the rate limit window (only used with --community)
    #[arg(long, default_value_t = DEFAULT_RATE_LIMIT_MAX_REQUESTS)]
    rate_limit_max_requests: u64,

    /// Rate limit window duration in seconds (only used with --community)
    #[arg(long, default_value_t = DEFAULT_RATE_LIMIT_WINDOW_SECS)]
    rate_limit_window_secs: u64,
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

fn load_or_generate_identity(path: &str) -> Result<Keypair, Box<dyn std::error::Error>> {
    let path = PathBuf::from(path);

    if path.exists() {
        let bytes = fs::read(&path)?;
        let key = Keypair::from_protobuf_encoding(&bytes)?;
        return Ok(key);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let key = Keypair::generate_ed25519();
    let encoded = key.to_protobuf_encoding()?;
    fs::write(&path, encoded)?;
    Ok(key)
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

    // Warn if community-only options are used without --community
    if !args.community {
        if args.data_dir.is_some() {
            warn!("--data-dir has no effect without --community");
        }
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
    info!("Max reservations: {}", args.max_reservations);
    info!("Max circuits per peer: {}", args.max_circuits_per_peer);

    let keypair = load_or_generate_identity(&args.identity_key_path)?;
    info!("Using identity key at {}", args.identity_key_path);

    // Initialize database and board service only in community mode
    let board_service: Option<BoardService> = if args.community {
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

        let relay_db = RelayDatabase::open(&db_path)?;
        let service = BoardService::new(relay_db, args.community_name.clone());
        info!("Database initialized at {}", db_path);
        Some(service)
    } else {
        None
    };

    // Initialize rate limiter for board sync requests (community mode only)
    let mut rate_limiter: Option<PeerRateLimiter> = if args.community {
        let limiter = PeerRateLimiter::new(
            args.rate_limit_max_requests,
            Duration::from_secs(args.rate_limit_window_secs),
        );
        info!(
            "Rate limiter enabled: {} requests per {}s window",
            args.rate_limit_max_requests, args.rate_limit_window_secs
        );
        Some(limiter)
    } else {
        None
    };

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

            // Configure relay server with limits from CLI args.
            // Durations are set very high — Harbor is a social app where users
            // stay connected for days. The libp2p defaults (1h reservation,
            // 2min circuit, 128KiB circuit bytes) are far too aggressive.
            let relay_config = relay::Config {
                max_reservations: args.max_reservations,
                max_circuits: args.max_circuits,
                max_circuits_per_peer: args.max_circuits_per_peer,
                reservation_duration: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
                max_circuit_duration: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
                max_circuit_bytes: 0, // unlimited
                ..Default::default()
            };

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

            // Board sync protocol (only in community mode)
            let board_sync = if community_mode {
                Toggle::from(Some(request_response::cbor::Behaviour::new(
                    [(
                        StreamProtocol::new(BOARD_SYNC_PROTOCOL),
                        ProtocolSupport::Full,
                    )],
                    request_response::Config::default(),
                )))
            } else {
                Toggle::from(None)
            };

            RelayServerBehaviour {
                relay,
                ping,
                identify,
                board_sync,
            }
        })?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(365 * 24 * 60 * 60)))
        .build();

    let local_peer_id = *swarm.local_peer_id();
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
        let external_tcp: Multiaddr =
            format!("/ip4/{}/tcp/{}/p2p/{}", announce_ip, args.port, local_peer_id).parse()?;
        let external_quic: Multiaddr =
            format!("/ip4/{}/udp/{}/quic-v1/p2p/{}", announce_ip, args.port, local_peer_id)
                .parse()?;
        let local_0_0_0_0_tcp: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}/p2p/{}", args.port, local_peer_id).parse()?;
        let local_0_0_0_0_quic: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1/p2p/{}", args.port, local_peer_id).parse()?;

        swarm.add_external_address(external_tcp.clone());
        swarm.add_external_address(external_quic.clone());
        swarm.add_external_address(local_0_0_0_0_tcp.clone());
        swarm.add_external_address(local_0_0_0_0_quic.clone());

        info!("========================================");
        info!("YOUR RELAY ADDRESSES:");
        info!("  TCP:  {}", external_tcp);
        info!("  QUIC: {}", external_quic);
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
        RATE_LIMITER_CLEANUP_INTERVAL_SECS,
    ));
    // The first tick completes immediately; consume it so we don't
    // run cleanup at startup.
    cleanup_interval.tick().await;

    // Run the event loop
    loop {
        tokio::select! {
            _ = cleanup_interval.tick() => {
                if let Some(ref mut limiter) = rate_limiter {
                    limiter.cleanup_stale_entries();
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
                            // Check per-peer rate limit before processing the request
                            let response = if let Some(ref mut limiter) = rate_limiter {
                                match limiter.check_rate_limit(&peer) {
                                    Ok(()) => handle_board_request(service, &local_peer_id, &peer, request),
                                    Err(rate_limit_error) => BoardSyncResponse::Error {
                                        error: rate_limit_error,
                                    },
                                }
                            } else {
                                handle_board_request(service, &local_peer_id, &peer, request)
                            };

                            if let Err(send_error) = swarm
                                .behaviour_mut()
                                .board_sync
                                .as_mut()
                                .expect("board_sync enabled in community mode")
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
                    info!("Connection established with: {} via {:?} ({:?})", peer_id, connection_id, endpoint);
                }
                SwarmEvent::ConnectionClosed { peer_id, connection_id, cause, endpoint, .. } => {
                    info!("Connection closed with: {} via {:?} ({:?}), cause: {:?}", peer_id, connection_id, endpoint, cause);
                }
                _ => {}
            }
        }
    }
}

fn handle_board_request(
    service: &BoardService,
    local_peer_id: &PeerId,
    peer: &PeerId,
    request: BoardSyncRequest,
) -> BoardSyncResponse {
    match request {
        BoardSyncRequest::RegisterPeer {
            peer_id,
            public_key,
            display_name,
            timestamp,
            signature,
        } => {
            if peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "peer_id mismatch".to_string(),
                };
            }
            match service.process_register_peer(&peer_id, &public_key, &display_name, timestamp, &signature) {
                Ok(()) => BoardSyncResponse::PeerRegistered { peer_id },
                Err(e) => BoardSyncResponse::Error { error: e },
            }
        }
        BoardSyncRequest::ListBoards {
            requester_peer_id,
            timestamp,
            signature,
        } => match service.process_list_boards(&requester_peer_id, timestamp, &signature) {
            Ok(boards) => {
                info!("Serving board list for community: {}", service.community_name());
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
            },
            Err(e) => BoardSyncResponse::Error { error: e },
        },
        BoardSyncRequest::GetBoardPosts {
            requester_peer_id,
            board_id,
            after_timestamp,
            limit,
            timestamp,
            signature,
        } => match service.process_get_board_posts(&requester_peer_id, &board_id, after_timestamp, limit, timestamp, &signature) {
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
            Err(e) => BoardSyncResponse::Error { error: e },
        },
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
                    error: "author_peer_id mismatch".to_string(),
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
                    error: "author_peer_id mismatch".to_string(),
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
            timestamp,
            request_signature,
        } => {
            if author_peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "author_peer_id mismatch".to_string(),
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
                timestamp,
                &request_signature,
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
        } => {
            match service.process_get_wall_posts(
                &requester_peer_id,
                &author_peer_id,
                since_lamport_clock,
                limit,
                timestamp,
                &signature,
            ) {
                Ok((posts, has_more)) => BoardSyncResponse::WallPosts {
                    posts: posts
                        .into_iter()
                        .map(|p| WallPostData {
                            post_id: p.post_id,
                            author_peer_id: p.author_peer_id,
                            content_type: p.content_type,
                            content_text: p.content_text,
                            visibility: p.visibility,
                            lamport_clock: p.lamport_clock,
                            created_at: p.created_at,
                            signature: p.signature,
                            stored_at: p.stored_at,
                        })
                        .collect(),
                    has_more,
                },
                Err(e) => BoardSyncResponse::Error { error: e },
            }
        }
        BoardSyncRequest::DeleteWallPost {
            author_peer_id,
            post_id,
            timestamp,
            signature,
        } => {
            if author_peer_id != peer.to_string() {
                return BoardSyncResponse::Error {
                    error: "author_peer_id mismatch".to_string(),
                };
            }
            match service.process_delete_wall_post(&author_peer_id, &post_id, timestamp, &signature) {
                Ok(()) => BoardSyncResponse::WallPostDeleted { post_id },
                Err(e) => BoardSyncResponse::Error { error: e },
            }
        }
    }
}
