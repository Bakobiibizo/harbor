use libp2p::{
    autonat, dcutr, identify, kad, mdns, ping, relay,
    request_response::{self, ProtocolSupport},
    swarm::NetworkBehaviour,
    StreamProtocol,
};
use std::collections::HashMap;
use std::time::Duration;

use super::protocols::{CONTENT_SYNC_PROTOCOL, IDENTITY_PROTOCOL, MESSAGING_PROTOCOL};

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
    /// DCUtR for direct connection upgrade through relay
    pub dcutr: dcutr::Behaviour,
    /// AutoNAT for external address discovery
    pub autonat: autonat::Behaviour,
    /// Request-response for identity exchange
    pub identity_exchange:
        request_response::cbor::Behaviour<IdentityExchangeRequest, IdentityExchangeResponse>,
    /// Request-response for messaging
    pub messaging: request_response::cbor::Behaviour<MessagingRequest, MessagingResponse>,
    /// Request-response for content sync (feed/wall)
    pub content_sync: request_response::cbor::Behaviour<ContentSyncRequest, ContentSyncResponse>,
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

/// Content sync request (manifest request)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub struct ContentSyncRequest {
    pub request_type: String, // "manifest" or "fetch"
    pub requester_peer_id: String,
    pub cursor: HashMap<String, u64>,
    pub limit: u32,
    pub post_id: Option<String>, // For fetch requests
    pub include_media: bool,
    pub timestamp: i64,
    pub signature: Vec<u8>,
    /// Request a manifest of posts newer than the provided cursor
    Manifest {
        requester_peer_id: String,
        cursor: std::collections::HashMap<String, u64>,
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

/// Content sync response
#[serde(tag = "type", rename_all = "snake_case")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ContentSyncResponse {

    pub response_type: String, // "manifest" or "fetch"
    pub responder_peer_id: String,
    // Manifest response fields
    pub posts: Vec<PostSummaryProto>,
    pub has_more: bool,
    pub next_cursor: HashMap<String, u64>,
    // Fetch response fields
    pub post_id: Option<String>,
    pub author_peer_id: Option<String>,
    pub content_type: Option<String>,
    pub content_text: Option<String>,
    pub visibility: Option<String>,
    pub lamport_clock: Option<u64>,
    pub created_at: Option<i64>,
    pub post_signature: Vec<u8>,
    // Common fields
    pub timestamp: i64,
    pub signature: Vec<u8>,
    pub success: bool,
    pub error: Option<String>,
    Manifest {
        responder_peer_id: String,
        posts: Vec<crate::services::PostSummary>,
        has_more: bool,
        next_cursor: std::collections::HashMap<String, u64>,
        timestamp: i64,
        signature: Vec<u8>,
    },
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
    Error {
        error: String,
    },
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

        // Kademlia DHT
        let kademlia =
            kad::Behaviour::new(local_peer_id, kad::store::MemoryStore::new(local_peer_id));

        // mDNS
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)
            .expect("Failed to create mDNS behaviour");

        // DCUtR for hole punching
        let dcutr = dcutr::Behaviour::new(local_peer_id);

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
        }
    }
}
