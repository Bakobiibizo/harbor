//! Harbor Bootstrap Node
//!
//! A lightweight bootstrap/rendezvous server for Harbor P2P network.
//! This node helps peers discover each other across different networks.
//!
//! Usage:
//!   harbor-bootstrap --port 9000
//!   harbor-bootstrap --port 9000 --external-ip 1.2.3.4

use clap::Parser;
use futures::StreamExt;
use libp2p::{
    autonat, identify, kad, noise, ping, relay,
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, StreamProtocol,
};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Harbor bootstrap node for P2P peer discovery
#[derive(Parser, Debug)]
#[command(name = "harbor-bootstrap")]
#[command(about = "Bootstrap node for Harbor P2P network")]
struct Args {
    /// Port to listen on for TCP
    #[arg(short, long, default_value = "9000")]
    port: u16,

    /// Port to listen on for QUIC (default: TCP port + 1)
    #[arg(long)]
    quic_port: Option<u16>,

    /// External IP address (for NAT traversal)
    /// If not specified, peers will see your local IP
    #[arg(long)]
    external_ip: Option<String>,

    /// Enable relay server functionality
    /// This allows this node to relay traffic for peers behind NAT
    #[arg(long, default_value = "true")]
    enable_relay: bool,

    /// Maximum circuits for relay (connections per peer)
    #[arg(long, default_value = "128")]
    relay_max_circuits: usize,

    /// Enable AutoNAT server
    /// This helps peers detect their NAT status
    #[arg(long, default_value = "true")]
    enable_autonat: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

/// Combined network behaviour for bootstrap node
#[derive(NetworkBehaviour)]
struct BootstrapBehaviour {
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    relay: Toggle<relay::Behaviour>,
    autonat: Toggle<autonat::Behaviour>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize logging
    let filter = if args.verbose {
        "harbor_bootstrap=debug,libp2p=debug"
    } else {
        "harbor_bootstrap=info,libp2p=warn"
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| filter.into()),
        )
        .init();

    info!("Starting Harbor Bootstrap Node");
    info!("TCP Port: {}", args.port);
    let quic_port = args.quic_port.unwrap_or(args.port + 1);
    info!("QUIC Port: {}", quic_port);
    if let Some(ref ip) = args.external_ip {
        info!("External IP: {}", ip);
    }
    info!("Relay enabled: {}", args.enable_relay);
    info!("AutoNAT enabled: {}", args.enable_autonat);

    // Generate a keypair for this bootstrap node
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    let peer_id = keypair.public().to_peer_id();

    info!("Bootstrap Node Peer ID: {}", peer_id);

    // Capture flags for use in closure
    let enable_relay = args.enable_relay;
    let enable_autonat = args.enable_autonat;

    // Build the swarm with TCP and QUIC transports
    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(|key| {
            let local_peer_id = key.public().to_peer_id();

            // Ping for keepalive
            let ping = ping::Behaviour::new(ping::Config::new());

            // Identify protocol
            let identify = identify::Behaviour::new(
                identify::Config::new("/harbor/bootstrap/1.0.0".to_string(), key.public())
                    .with_agent_version("harbor-bootstrap/0.1.0".to_string()),
            );

            // Kademlia DHT for peer routing
            let mut kad_config = kad::Config::new(StreamProtocol::new("/harbor/kad/1.0.0"));
            kad_config.set_query_timeout(Duration::from_secs(60));

            let store = kad::store::MemoryStore::new(local_peer_id);
            let kademlia = kad::Behaviour::with_config(local_peer_id, store, kad_config);

            // Relay server for NAT traversal (conditionally enabled)
            let relay = if enable_relay {
                let relay_config = relay::Config {
                    max_reservations: 128,
                    max_reservations_per_peer: 4,
                    reservation_duration: Duration::from_secs(3600), // 1 hour
                    max_circuits: 128,
                    max_circuits_per_peer: 4,
                    ..Default::default()
                };
                Toggle::from(Some(relay::Behaviour::new(local_peer_id, relay_config)))
            } else {
                Toggle::from(None)
            };

            // AutoNAT server to help peers detect their NAT status (conditionally enabled)
            let autonat = if enable_autonat {
                Toggle::from(Some(autonat::Behaviour::new(
                    local_peer_id,
                    autonat::Config::default(),
                )))
            } else {
                Toggle::from(None)
            };

            BootstrapBehaviour {
                ping,
                identify,
                kademlia,
                relay,
                autonat,
            }
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(300)))
        .build();

    // Listen on all interfaces - TCP
    let listen_addr_tcp: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", args.port).parse()?;
    swarm.listen_on(listen_addr_tcp.clone())?;
    info!("Listening on TCP: {}", listen_addr_tcp);

    // Listen on QUIC
    let listen_addr_quic: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", quic_port).parse()?;
    swarm.listen_on(listen_addr_quic.clone())?;
    info!("Listening on QUIC: {}", listen_addr_quic);

    // If external IP provided, also listen on IPv6
    let listen_addr_v6_tcp: Multiaddr = format!("/ip6/::/tcp/{}", args.port).parse()?;
    if let Err(e) = swarm.listen_on(listen_addr_v6_tcp.clone()) {
        warn!("Could not listen on IPv6 TCP: {}", e);
    }

    let listen_addr_v6_quic: Multiaddr = format!("/ip6/::/udp/{}/quic-v1", quic_port).parse()?;
    if let Err(e) = swarm.listen_on(listen_addr_v6_quic.clone()) {
        warn!("Could not listen on IPv6 QUIC: {}", e);
    }

    // Print connection info
    println!("\n========================================");
    println!("Harbor Bootstrap Node Running");
    println!("========================================");
    println!("Peer ID: {}", peer_id);
    println!("TCP Port: {}", args.port);
    println!("QUIC Port: {}", quic_port);
    println!(
        "Relay: {}",
        if args.enable_relay {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!(
        "AutoNAT: {}",
        if args.enable_autonat {
            "enabled"
        } else {
            "disabled"
        }
    );

    if let Some(ref external_ip) = args.external_ip {
        println!("\nShare these addresses with peers:");
        println!(
            "  TCP:  /ip4/{}/tcp/{}/p2p/{}",
            external_ip, args.port, peer_id
        );
        println!(
            "  QUIC: /ip4/{}/udp/{}/quic-v1/p2p/{}",
            external_ip, quic_port, peer_id
        );
    } else {
        println!("\nLocal addresses will be printed when available.");
        println!("For remote connections, use --external-ip YOUR_PUBLIC_IP");
    }
    println!("========================================\n");

    // Track connected peers
    let mut connected_peers: std::collections::HashSet<PeerId> = std::collections::HashSet::new();

    // Event loop
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                let full_addr = format!("{}/p2p/{}", address, peer_id);
                info!("Listening on: {}", full_addr);

                // Print user-friendly message for local addresses
                if address.to_string().contains("127.0.0.1") || address.to_string().contains("::1")
                {
                    debug!("(localhost address - for local testing only)");
                }
            }

            SwarmEvent::ConnectionEstablished {
                peer_id: remote_peer,
                endpoint,
                ..
            } => {
                connected_peers.insert(remote_peer);
                info!(
                    "Peer connected: {} via {:?} (total: {})",
                    remote_peer,
                    endpoint,
                    connected_peers.len()
                );

                // Add to Kademlia routing table
                if let Some(addr) = endpoint.get_remote_address().clone().into() {
                    swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&remote_peer, addr);
                }
            }

            SwarmEvent::ConnectionClosed {
                peer_id: remote_peer,
                cause,
                ..
            } => {
                connected_peers.remove(&remote_peer);
                info!(
                    "Peer disconnected: {} (cause: {:?}, remaining: {})",
                    remote_peer,
                    cause,
                    connected_peers.len()
                );
            }

            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Identify(
                identify::Event::Received {
                    peer_id: remote_peer,
                    info,
                    ..
                },
            )) => {
                info!(
                    "Identified peer {}: {} ({})",
                    remote_peer, info.agent_version, info.protocol_version
                );

                // Add all of the peer's listen addresses to Kademlia
                for addr in info.listen_addrs {
                    debug!("Adding address for {}: {}", remote_peer, addr);
                    swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&remote_peer, addr);
                }
            }

            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Kademlia(
                kad::Event::RoutingUpdated {
                    peer,
                    is_new_peer,
                    addresses,
                    ..
                },
            )) => {
                if is_new_peer {
                    info!(
                        "New peer added to routing table: {} ({} addresses)",
                        peer,
                        addresses.len()
                    );
                } else {
                    debug!("Routing table updated for peer: {}", peer);
                }
            }

            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Kademlia(
                kad::Event::InboundRequest { request },
            )) => {
                debug!("Kademlia inbound request: {:?}", request);
            }

            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Ping(ping::Event {
                peer,
                result,
                ..
            })) => match result {
                Ok(rtt) => debug!("Ping to {}: {:?}", peer, rtt),
                Err(e) => debug!("Ping to {} failed: {:?}", peer, e),
            },

            SwarmEvent::IncomingConnection {
                local_addr,
                send_back_addr,
                ..
            } => {
                debug!(
                    "Incoming connection from {} to {}",
                    send_back_addr, local_addr
                );
            }

            SwarmEvent::OutgoingConnectionError {
                peer_id: Some(peer),
                error,
                ..
            } => {
                warn!("Failed to connect to {}: {}", peer, error);
            }

            SwarmEvent::IncomingConnectionError {
                local_addr,
                send_back_addr,
                error,
                ..
            } => {
                warn!(
                    "Incoming connection error from {} to {}: {}",
                    send_back_addr, local_addr, error
                );
            }

            // Relay server events
            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Relay(
                relay::Event::ReservationReqAccepted {
                    src_peer_id,
                    renewed,
                },
            )) => {
                info!(
                    "Relay reservation accepted for {} (renewed: {})",
                    src_peer_id, renewed
                );
            }

            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Relay(
                relay::Event::ReservationReqDenied { src_peer_id, .. },
            )) => {
                warn!("Relay reservation denied for {}", src_peer_id);
            }

            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Relay(
                relay::Event::CircuitReqAccepted {
                    src_peer_id,
                    dst_peer_id,
                },
            )) => {
                debug!("Relay circuit accepted: {} -> {}", src_peer_id, dst_peer_id);
            }

            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Relay(
                relay::Event::CircuitClosed {
                    src_peer_id,
                    dst_peer_id,
                    ..
                },
            )) => {
                debug!("Relay circuit closed: {} -> {}", src_peer_id, dst_peer_id);
            }

            // AutoNAT events
            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Autonat(
                autonat::Event::InboundProbe(autonat::InboundProbeEvent::Request { peer, .. }),
            )) => {
                debug!("AutoNAT probe request from {}", peer);
            }

            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Autonat(
                autonat::Event::InboundProbe(autonat::InboundProbeEvent::Response { peer, .. }),
            )) => {
                debug!("AutoNAT probe response sent to {}", peer);
            }

            _ => {}
        }
    }
}
