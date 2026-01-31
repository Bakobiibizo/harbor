//! Harbor Relay Server
//!
//! A simple libp2p relay server that enables NAT traversal for Harbor chat app users.
//! This server accepts relay reservations and forwards traffic between peers who
//! cannot connect directly due to NAT/firewall restrictions.

use clap::Parser;
use futures::StreamExt;
use libp2p::{
    identify, noise, ping, relay,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, SwarmBuilder,
};
use std::net::Ipv4Addr;
use std::time::Duration;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Harbor Relay Server - Enables NAT traversal for Harbor chat app
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
}

/// Combined behaviour for the relay server
#[derive(NetworkBehaviour)]
struct RelayServerBehaviour {
    relay: relay::Behaviour,
    ping: ping::Behaviour,
    identify: identify::Behaviour,
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

    info!("Starting Harbor Relay Server...");
    info!("Port: {}", args.port);
    info!("Max reservations: {}", args.max_reservations);
    info!("Max circuits per peer: {}", args.max_circuits_per_peer);

    // Build the swarm
    let mut swarm = SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(|keypair| {
            let local_peer_id = PeerId::from(keypair.public());
            let local_public_key = keypair.public();

            // Configure relay server with limits from CLI args
            let relay_config = relay::Config {
                max_reservations: args.max_reservations,
                max_circuits: args.max_circuits,
                max_circuits_per_peer: args.max_circuits_per_peer,
                ..Default::default()
            };

            let relay = relay::Behaviour::new(local_peer_id, relay_config);

            let ping = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(30)));

            let identify = identify::Behaviour::new(identify::Config::new(
                "/harbor-relay/1.0.0".to_string(),
                local_public_key,
            ));

            RelayServerBehaviour {
                relay,
                ping,
                identify,
            }
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(3600)))
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

        swarm.add_external_address(external_tcp.clone());
        swarm.add_external_address(external_quic.clone());

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

    // Run the event loop
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on: {}/p2p/{}", address, local_peer_id);
            }
            SwarmEvent::Behaviour(RelayServerBehaviourEvent::Relay(event)) => {
                // Log all relay events for debugging
                info!("Relay event: {:?}", event);
            }
            SwarmEvent::Behaviour(RelayServerBehaviourEvent::Identify(identify::Event::Received {
                peer_id,
                info,
                ..
            })) => {
                info!("Identified peer {}: {}", peer_id, info.agent_version);
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                info!("Connection established with: {}", peer_id);
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                info!("Connection closed with: {}", peer_id);
            }
            _ => {}
        }
    }
}
