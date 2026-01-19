use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};

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
