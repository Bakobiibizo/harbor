use libp2p::{
    autonat, dcutr, identify, kad, mdns, ping, relay,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, behaviour::toggle::Toggle},
    StreamProtocol,
};
use std::collections::HashMap;
use std::time::Duration;

use super::protocols::board_sync::{BoardSyncRequest, BoardSyncResponse};
use super::protocols::{
    BOARD_SYNC_PROTOCOL, CONTENT_SYNC_PROTOCOL, IDENTITY_PROTOCOL, MESSAGING_PROTOCOL,
};

// Duration is used in ping configuration

/// Combined network behaviour for the chat application
#[derive(NetworkBehaviour)]
pub struct ChatBehaviour {
    /// Ping protocol for connection liveness
    pub ping: ping::Behaviour,
    /// Identify protocol for peer identification
    pub identify: identify::Behaviour,
    /// Kademlia DHT for peer discovery and routing
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    /// mDNS for local network peer discovery
    pub mdns: mdns::tokio::Behaviour,
    /// Relay client for NAT traversal
    pub relay_client: relay::client::Behaviour,
    /// DCUtR for direct connection upgrade through relay (disabled by default —
    /// hole punching fails in most agent topologies and destabilises relay circuits)
    pub dcutr: Toggle<dcutr::Behaviour>,
    /// AutoNAT for external address discovery
    pub autonat: autonat::Behaviour,
    /// Request-response for identity exchange
    pub identity_exchange:
        request_response::cbor::Behaviour<IdentityExchangeRequest, IdentityExchangeResponse>,
    /// Request-response for messaging
    pub messaging: request_response::cbor::Behaviour<MessagingRequest, MessagingResponse>,
    /// Request-response for content sync (feed/wall)
    pub content_sync: request_response::cbor::Behaviour<ContentSyncRequest, ContentSyncResponse>,
    /// Request-response for board sync (community boards)
    pub board_sync: request_response::cbor::Behaviour<BoardSyncRequest, BoardSyncResponse>,
}

/// Identity exchange request (simplified for request-response)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IdentityExchangeRequest {
    pub requester_peer_id: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Identity exchange response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IdentityExchangeResponse {
    pub peer_id: String,
    pub public_key: Vec<u8>,
    pub x25519_public: Vec<u8>,
    pub display_name: String,
    pub avatar_hash: Option<String>,
    pub bio: Option<String>,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Messaging request
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MessagingRequest {
    pub message_type: String,
    pub payload: Vec<u8>,
}

/// Messaging response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MessagingResponse {
    pub success: bool,
    pub message_id: Option<String>,
    pub error: Option<String>,
}

/// Post summary for content sync manifest
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PostSummaryProto {
    pub post_id: String,
    pub author_peer_id: String,
    pub lamport_clock: u64,
    pub content_type: String,
    pub has_media: bool,
    pub media_hashes: Vec<String>,
    pub created_at: i64,
}

/// Content sync request (wire protocol)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentSyncRequest {
    /// Request a manifest of posts newer than the provided cursor
    Manifest {
        requester_peer_id: String,
        cursor: HashMap<String, u64>,
        limit: u32,
        timestamp: i64,
        signature: Vec<u8>,
    },
    /// Fetch a full post by ID
    FetchPost {
        post_id: String,
        include_media: bool,
        requester_peer_id: String,
        timestamp: i64,
        signature: Vec<u8>,
    },
}

/// Content sync response (wire protocol)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentSyncResponse {
    /// Response with manifest of posts
    Manifest {
        responder_peer_id: String,
        posts: Vec<PostSummaryProto>,
        has_more: bool,
        next_cursor: HashMap<String, u64>,
        timestamp: i64,
        signature: Vec<u8>,
    },
    /// Response with full post content
    Post {
        post_id: String,
        author_peer_id: String,
        content_type: String,
        content_text: Option<String>,
        visibility: String,
        lamport_clock: u64,
        created_at: i64,
        signature: Vec<u8>,
    },
    /// Error response
    Error { error: String },
}

impl ChatBehaviour {
    /// Create a new chat behaviour with the given local peer ID and keypair
    pub fn new(
        local_peer_id: libp2p::PeerId,
        local_public_key: libp2p::identity::PublicKey,
        relay_client: relay::client::Behaviour,
    ) -> Self {
        // Ping
        let ping = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(15)));

        // Identify
        let identify = identify::Behaviour::new(identify::Config::new(
            "/harbor/1.0.0".to_string(),
            local_public_key.clone(),
        ));

        // Kademlia DHT — use custom protocol name matching bootstrap node
        // to avoid pollution from the public IPFS DHT
        let mut kad_config = kad::Config::new(StreamProtocol::new("/harbor/kad/1.0.0"));
        kad_config.set_query_timeout(Duration::from_secs(60));
        let store = kad::store::MemoryStore::new(local_peer_id);
        let kademlia = kad::Behaviour::with_config(local_peer_id, store, kad_config);

        // mDNS
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)
            .expect("Failed to create mDNS behaviour");

        // DCUtR for hole punching — disabled by default.
        // When enabled, failed hole-punch attempts destabilise relay circuits
        // causing disconnections every ~2 minutes.
        let dcutr = Toggle::from(None::<dcutr::Behaviour>);

        // AutoNAT
        let autonat = autonat::Behaviour::new(local_peer_id, autonat::Config::default());

        // Identity exchange protocol
        let identity_exchange = request_response::cbor::Behaviour::new(
            [(
                StreamProtocol::new(IDENTITY_PROTOCOL),
                ProtocolSupport::Full,
            )],
            request_response::Config::default(),
        );

        // Messaging protocol
        let messaging = request_response::cbor::Behaviour::new(
            [(
                StreamProtocol::new(MESSAGING_PROTOCOL),
                ProtocolSupport::Full,
            )],
            request_response::Config::default(),
        );

        // Content sync protocol
        let content_sync = request_response::cbor::Behaviour::new(
            [(
                StreamProtocol::new(CONTENT_SYNC_PROTOCOL),
                ProtocolSupport::Full,
            )],
            request_response::Config::default(),
        );

        // Board sync protocol
        let board_sync = request_response::cbor::Behaviour::new(
            [(
                StreamProtocol::new(BOARD_SYNC_PROTOCOL),
                ProtocolSupport::Full,
            )],
            request_response::Config::default(),
        );

        Self {
            ping,
            identify,
            kademlia,
            mdns,
            relay_client,
            dcutr,
            autonat,
            identity_exchange,
            messaging,
            content_sync,
            board_sync,
        }
    }
}
