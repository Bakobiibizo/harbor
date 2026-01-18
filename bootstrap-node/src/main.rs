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
    identify, kad, noise, ping,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, StreamProtocol,
};
use std::time::Duration;
use tracing::{info, warn, debug};

/// Harbor bootstrap node for P2P peer discovery
#[derive(Parser, Debug)]
#[command(name = "harbor-bootstrap")]
#[command(about = "Bootstrap node for Harbor P2P network")]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "9000")]
    port: u16,

    /// External IP address (for NAT traversal)
    /// If not specified, peers will see your local IP
    #[arg(long)]
    external_ip: Option<String>,

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
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| filter.into()),
        )
        .init();

    info!("Starting Harbor Bootstrap Node");
    info!("Port: {}", args.port);
    if let Some(ref ip) = args.external_ip {
        info!("External IP: {}", ip);
    }

    // Generate a keypair for this bootstrap node
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    let peer_id = keypair.public().to_peer_id();

    info!("Bootstrap Node Peer ID: {}", peer_id);

    // Build the swarm
    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|key| {
            // Ping for keepalive
            let ping = ping::Behaviour::new(ping::Config::new());

            // Identify protocol
            let identify = identify::Behaviour::new(
                identify::Config::new(
                    "/harbor/bootstrap/1.0.0".to_string(),
                    key.public(),
                )
                .with_agent_version("harbor-bootstrap/0.1.0".to_string()),
            );

            // Kademlia DHT for peer routing
            let mut kad_config = kad::Config::new(StreamProtocol::new("/harbor/kad/1.0.0"));
            kad_config.set_query_timeout(Duration::from_secs(60));

            let store = kad::store::MemoryStore::new(key.public().to_peer_id());
            let kademlia = kad::Behaviour::with_config(
                key.public().to_peer_id(),
                store,
                kad_config,
            );

            BootstrapBehaviour {
                ping,
                identify,
                kademlia,
            }
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(300)))
        .build();

    // Listen on all interfaces
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", args.port).parse()?;
    swarm.listen_on(listen_addr.clone())?;
    info!("Listening on {}", listen_addr);

    // If external IP provided, also listen on IPv6
    let listen_addr_v6: Multiaddr = format!("/ip6/::/tcp/{}", args.port).parse()?;
    if let Err(e) = swarm.listen_on(listen_addr_v6.clone()) {
        warn!("Could not listen on IPv6: {}", e);
    }

    // Print connection info
    println!("\n========================================");
    println!("Harbor Bootstrap Node Running");
    println!("========================================");
    println!("Peer ID: {}", peer_id);
    println!("Port: {}", args.port);

    if let Some(ref external_ip) = args.external_ip {
        let multiaddr = format!("/ip4/{}/tcp/{}/p2p/{}", external_ip, args.port, peer_id);
        println!("\nShare this address with peers:");
        println!("  {}", multiaddr);
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
                if address.to_string().contains("127.0.0.1") || address.to_string().contains("::1") {
                    debug!("(localhost address - for local testing only)");
                }
            }

            SwarmEvent::ConnectionEstablished { peer_id: remote_peer, endpoint, .. } => {
                connected_peers.insert(remote_peer);
                info!(
                    "Peer connected: {} via {:?} (total: {})",
                    remote_peer,
                    endpoint,
                    connected_peers.len()
                );

                // Add to Kademlia routing table
                if let Some(addr) = endpoint.get_remote_address().clone().into() {
                    swarm.behaviour_mut().kademlia.add_address(&remote_peer, addr);
                }
            }

            SwarmEvent::ConnectionClosed { peer_id: remote_peer, cause, .. } => {
                connected_peers.remove(&remote_peer);
                info!(
                    "Peer disconnected: {} (cause: {:?}, remaining: {})",
                    remote_peer,
                    cause,
                    connected_peers.len()
                );
            }

            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Identify(identify::Event::Received {
                peer_id: remote_peer,
                info,
                ..
            })) => {
                info!(
                    "Identified peer {}: {} ({})",
                    remote_peer,
                    info.agent_version,
                    info.protocol_version
                );

                // Add all of the peer's listen addresses to Kademlia
                for addr in info.listen_addrs {
                    debug!("Adding address for {}: {}", remote_peer, addr);
                    swarm.behaviour_mut().kademlia.add_address(&remote_peer, addr);
                }
            }

            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Kademlia(kad::Event::RoutingUpdated {
                peer,
                is_new_peer,
                addresses,
                ..
            })) => {
                if is_new_peer {
                    info!("New peer added to routing table: {} ({} addresses)", peer, addresses.len());
                } else {
                    debug!("Routing table updated for peer: {}", peer);
                }
            }

            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Kademlia(kad::Event::InboundRequest { request })) => {
                debug!("Kademlia inbound request: {:?}", request);
            }

            SwarmEvent::Behaviour(BootstrapBehaviourEvent::Ping(ping::Event { peer, result, .. })) => {
                match result {
                    Ok(rtt) => debug!("Ping to {}: {:?}", peer, rtt),
                    Err(e) => debug!("Ping to {} failed: {:?}", peer, e),
                }
            }

            SwarmEvent::IncomingConnection { local_addr, send_back_addr, .. } => {
                debug!("Incoming connection from {} to {}", send_back_addr, local_addr);
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer) = peer_id {
                    warn!("Failed to connect to {}: {}", peer, error);
                }
            }

            SwarmEvent::IncomingConnectionError { local_addr, send_back_addr, error, .. } => {
                warn!(
                    "Incoming connection error from {} to {}: {}",
                    send_back_addr, local_addr, error
                );
            }

            _ => {}
        }
    }
}
