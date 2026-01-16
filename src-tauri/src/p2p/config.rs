use libp2p::Multiaddr;
use std::time::Duration;

/// Configuration for the P2P network
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Port to listen on for TCP connections (0 = random)
    pub tcp_port: u16,
    /// Port to listen on for QUIC connections (0 = random)
    pub quic_port: u16,
    /// Enable mDNS for local peer discovery
    pub enable_mdns: bool,
    /// Enable the Kademlia DHT
    pub enable_dht: bool,
    /// Bootstrap nodes for the DHT
    pub bootstrap_nodes: Vec<Multiaddr>,
    /// Idle connection timeout
    pub idle_connection_timeout: Duration,
    /// Enable relay client for NAT traversal
    pub enable_relay_client: bool,
    /// Enable DCUtR (Direct Connection Upgrade through Relay) for hole punching
    pub enable_dcutr: bool,
    /// Enable AutoNAT for external address discovery
    pub enable_autonat: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            tcp_port: 0,  // Random port
            quic_port: 0, // Random port
            enable_mdns: true,
            enable_dht: true,
            bootstrap_nodes: Vec::new(),
            idle_connection_timeout: Duration::from_secs(60),
            enable_relay_client: true,
            enable_dcutr: true,
            enable_autonat: true,
        }
    }
}

impl NetworkConfig {
    /// Create a config optimized for LAN discovery
    pub fn lan_only() -> Self {
        Self {
            enable_mdns: true,
            enable_dht: false,
            enable_relay_client: false,
            enable_dcutr: false,
            enable_autonat: false,
            ..Default::default()
        }
    }

    /// Create a config with custom ports
    pub fn with_ports(tcp_port: u16, quic_port: u16) -> Self {
        Self {
            tcp_port,
            quic_port,
            ..Default::default()
        }
    }
}
