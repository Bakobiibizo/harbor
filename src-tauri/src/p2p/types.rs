use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::protocols::board_sync::WallPostMediaItem;

/// Network connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
}

/// NAT status detected by AutoNAT
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NatStatus {
    /// NAT status not yet determined
    #[default]
    Unknown,
    /// We appear to have a public IP address
    Public,
    /// We are behind NAT but have relay connectivity
    Private,
    /// We are behind strict NAT, relay may not work
    BehindNat,
}

/// Information about a discovered or connected peer
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub peer_id: String,
    pub addresses: Vec<String>,
    pub protocol_version: Option<String>,
    pub agent_version: Option<String>,
    pub is_connected: bool,
    pub last_seen: Option<i64>,
}

/// Network statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStats {
    pub connected_peers: usize,
    pub total_bytes_in: u64,
    pub total_bytes_out: u64,
    pub uptime_seconds: u64,
    /// Current NAT status
    pub nat_status: NatStatus,
    /// Relay addresses we can be reached at (via relay)
    pub relay_addresses: Vec<String>,
    /// External addresses discovered via AutoNAT
    pub external_addresses: Vec<String>,
}

/// Events emitted by the network layer to the application
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NetworkEvent {
    /// A new peer was discovered (e.g., via mDNS)
    PeerDiscovered { peer_id: String },
    /// A peer went offline/expired
    PeerExpired { peer_id: String },
    /// Successfully connected to a peer
    PeerConnected { peer_id: String },
    /// Disconnected from a peer
    PeerDisconnected { peer_id: String },
    /// Our external address was discovered
    ExternalAddressDiscovered { address: String },
    /// Listening on a new address
    ListeningOn { address: String },
    /// An incoming message was received
    MessageReceived {
        peer_id: String,
        protocol: String,
        payload: Vec<u8>,
    },
    /// Network status changed
    StatusChanged { status: ConnectionStatus },
    /// A contact was added via identity exchange
    ContactAdded {
        peer_id: String,
        display_name: String,
    },
    /// NAT status changed
    NatStatusChanged { status: NatStatus },
    /// Successfully connected to a relay and have a relay address
    RelayConnected { relay_address: String },
    /// Direct connection established via hole-punching
    HolePunchSucceeded { peer_id: String },
    /// Content manifest received from a peer
    ContentManifestReceived {
        peer_id: String,
        post_count: usize,
        has_more: bool,
    },
    /// Content fetched from a peer
    ContentFetched { peer_id: String, post_id: String },
    /// Content sync error
    ContentSyncError { peer_id: String, error: String },
    /// Board list received from a relay
    BoardListReceived {
        relay_peer_id: String,
        board_count: usize,
    },
    /// Board posts received from a relay
    BoardPostsReceived {
        relay_peer_id: String,
        board_id: String,
        post_count: usize,
    },
    /// Board post submitted successfully
    BoardPostSubmitted {
        relay_peer_id: String,
        post_id: String,
    },
    /// Board sync error
    BoardSyncError {
        relay_peer_id: String,
        error: String,
    },
    /// A community relay was auto-detected and joined
    CommunityAutoJoined {
        relay_peer_id: String,
        relay_address: String,
        community_name: Option<String>,
        board_count: usize,
    },
    /// A message acknowledgment was received (delivery or read receipt)
    MessageAckReceived {
        message_id: String,
        conversation_id: String,
        status: String,
        timestamp: i64,
    },
    /// A wall post was successfully stored on a relay
    WallPostSynced {
        relay_peer_id: String,
        post_id: String,
    },
    /// Wall posts were received from a relay for a specific author
    WallPostsReceived {
        relay_peer_id: String,
        author_peer_id: String,
        post_count: usize,
    },
    /// A wall post was deleted on the relay
    WallPostDeletedOnRelay {
        relay_peer_id: String,
        post_id: String,
    },
    /// Media was fetched from a peer and stored locally
    MediaFetched {
        peer_id: String,
        media_hash: String,
    },
}

/// Commands that can be sent to the network service
#[derive(Debug)]
pub enum NetworkCommand {
    /// Connect to a specific peer
    Dial {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    /// Disconnect from a peer
    Disconnect { peer_id: PeerId },
    /// Send a message to a peer
    SendMessage {
        peer_id: PeerId,
        protocol: String,
        payload: Vec<u8>,
    },
    /// Request identity from a peer
    RequestIdentity { peer_id: PeerId },
    /// Get current network stats
    GetStats,
    /// Get list of connected peers
    GetConnectedPeers,
    /// Get listening addresses
    GetListeningAddresses,
    /// Add a bootstrap node address
    AddBootstrapNode { address: Multiaddr },
    /// Bootstrap the DHT
    Bootstrap,
    /// Add a custom relay server address
    AddRelayServer { address: Multiaddr },
    /// Connect to public relay servers
    ConnectToPublicRelays,
    /// Request content manifest from a peer
    RequestContentManifest {
        peer_id: PeerId,
        cursor: HashMap<String, u64>,
        limit: u32,
    },
    /// Request content fetch from a peer
    RequestContentFetch {
        peer_id: PeerId,
        post_id: String,
        include_media: bool,
    },
    /// Sync feed content from connected peers
    SyncFeed { limit: u32 },
    /// Join a community (register peer + list boards)
    JoinCommunity {
        relay_peer_id: PeerId,
        relay_address: String,
    },
    /// List boards on a relay
    ListBoards { relay_peer_id: PeerId },
    /// Get board posts from a relay
    GetBoardPosts {
        relay_peer_id: PeerId,
        board_id: String,
        after_timestamp: Option<i64>,
        limit: u32,
    },
    /// Submit a board post to a relay
    SubmitBoardPost {
        relay_peer_id: PeerId,
        board_id: String,
        content_text: String,
    },
    /// Delete a board post on a relay
    DeleteBoardPost {
        relay_peer_id: PeerId,
        post_id: String,
    },
    /// Sync a board (get latest posts)
    SyncBoard {
        relay_peer_id: PeerId,
        board_id: String,
    },
    /// Submit a wall post to a relay for offline availability
    SubmitWallPostToRelay {
        relay_peer_id: PeerId,
        post_id: String,
        content_type: String,
        content_text: Option<String>,
        visibility: String,
        lamport_clock: i64,
        created_at: i64,
        signature: Vec<u8>,
        media_items: Vec<WallPostMediaItem>,
    },
    /// Fetch media by hash from a peer
    FetchMedia {
        peer_id: PeerId,
        media_hash: String,
    },
    /// Get wall posts for a specific author from a relay
    GetWallPostsFromRelay {
        relay_peer_id: PeerId,
        author_peer_id: String,
        since_lamport_clock: i64,
        limit: u32,
    },
    /// Delete a wall post on a relay
    DeleteWallPostOnRelay {
        relay_peer_id: PeerId,
        post_id: String,
    },
    /// Shutdown the network
    Shutdown,
}

/// Response to network commands
#[derive(Debug)]
pub enum NetworkResponse {
    Ok,
    Stats(NetworkStats),
    Peers(Vec<PeerInfo>),
    Addresses(Vec<String>),
    Error(String),
}
