use clap::Parser;
use futures::StreamExt;
use libp2p::{
    identify, noise, ping, relay,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, SwarmBuilder,
    identity::Keypair,
};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
struct Args {
    /// Relay multiaddr to dial (must include /p2p/<relay_peer_id>)
    #[arg(long)]
    relay_addr: String,

    /// Optional peer circuit address to dial (e.g. /p2p/<relay>/p2p-circuit/p2p/<peer>)
    #[arg(long)]
    dial: Option<String>,

    /// Local TCP listen port (0 = random)
    #[arg(long, default_value_t = 0)]
    listen_port: u16,

    /// Path to the persistent identity key (generated if missing)
    #[arg(long, default_value_t = default_identity_path())]
    identity_key_path: String,
}

#[derive(NetworkBehaviour)]
struct SmokeBehaviour {
    relay_client: relay::client::Behaviour,
    ping: ping::Behaviour,
    identify: identify::Behaviour,
}

fn default_identity_path() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/harbor-relay-smoke/id.key")
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
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let args = Args::parse();
    let keypair = load_or_generate_identity(&args.identity_key_path)?;

    info!("Using identity key at {}", args.identity_key_path);

    let local_peer_id = PeerId::from(keypair.public());
    info!("Local Peer ID: {}", local_peer_id);

    let relay_addr: Multiaddr = args.relay_addr.parse()?;

    let mut swarm = SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
        .with_quic()
        .with_relay_client(noise::Config::new, yamux::Config::default)?
        .with_behaviour(|key, relay_client| {
            let ping = ping::Behaviour::new(
                ping::Config::new()
                    .with_interval(Duration::from_secs(10))
                    .with_timeout(Duration::from_secs(30)),
            );

            let identify = identify::Behaviour::new(identify::Config::new(
                "/harbor-relay-smoke/1.0.0".to_string(),
                key.public(),
            ));

            Ok(SmokeBehaviour {
                relay_client,
                ping,
                identify,
            })
        })?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(365 * 24 * 60 * 60)))
        .build();

    // Listen locally (random port by default) for any inbound via relay
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", args.listen_port).parse()?;
    swarm.listen_on(listen_addr)?;

    // Dial the relay
    info!("Dialing relay: {}", relay_addr);
    swarm.dial(relay_addr.clone())?;

    let mut dial_target: Option<Multiaddr> = match args.dial {
        Some(addr) => Some(addr.parse::<Multiaddr>()?),
        None => None,
    };

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on: {}", address);
            }
            SwarmEvent::Behaviour(SmokeBehaviourEvent::RelayClient(event)) => {
                match event {
                    relay::client::Event::ReservationReqAccepted { relay_peer_id, renewal, limit: _ } => {
                        info!("Reservation accepted by relay {} (renewal: {})", relay_peer_id, renewal);
                        let circuit_addr: Multiaddr = format!("/p2p/{}/p2p-circuit/p2p/{}", relay_peer_id, local_peer_id).parse()?;
                        println!("CIRCUIT_ADDRESS {}", circuit_addr);
                    }
                    relay::client::Event::OutboundCircuitEstablished { relay_peer_id, .. } => {
                        info!("Outbound circuit established via relay {}", relay_peer_id);
                    }
                    relay::client::Event::InboundCircuitEstablished { src_peer_id, .. } => {
                        info!("Inbound circuit established from {}", src_peer_id);
                    }
                }
            }
            SwarmEvent::Behaviour(SmokeBehaviourEvent::Ping(event)) => {
                info!("Ping event: {:?}", event);
            }
            SwarmEvent::Behaviour(SmokeBehaviourEvent::Identify(event)) => {
                info!("Identify event: {:?}", event);
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                info!("Connection established with {} at {:?}", peer_id, endpoint);
                
                // If this is the relay connection, request a relay reservation by listening on circuit
                if let Some(relay_peer_id) = relay_peer_id_from_addr(&relay_addr) {
                    if peer_id == relay_peer_id {
                        info!("Connected to relay, requesting relay reservation...");
                        let circuit_listen_addr: Multiaddr = format!("/p2p/{}/p2p-circuit", relay_peer_id).parse()?;
                        if let Err(e) = swarm.listen_on(circuit_listen_addr.clone()) {
                            error!("Failed to listen on relay circuit {}: {}", circuit_listen_addr, e);
                        }
                    }
                }
                
                // Once connected to relay, if a dial target was provided, try once.
                if let Some(target) = dial_target.take() {
                    info!("Dialing target via relay: {}", target);
                    if let Err(e) = swarm.dial(target.clone()) {
                        error!("Failed to dial target {}: {}", target, e);
                    }
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, endpoint, cause, .. } => {
                info!("Connection closed with {} ({:?}), cause: {:?}", peer_id, endpoint, cause);
            }
            SwarmEvent::OutgoingConnectionError { connection_id, peer_id, error } => {
                info!("Outgoing connection error to {:?} via {:?}: {:?}", peer_id, connection_id, error);
            }
            other => {
                debug_event(other);
            }
        }
    }
}

fn debug_event<T: std::fmt::Debug>(event: SwarmEvent<T>) {
    // Reduce noise; adjust as needed for debugging.
    if let SwarmEvent::IncomingConnection { .. } = event {
        info!("Incoming connection event: {:?}", event);
    }
}

/// Extract the peer ID from a multiaddr that ends with /p2p/<peer_id>
fn relay_peer_id_from_addr(addr: &Multiaddr) -> Option<PeerId> {
    use libp2p::multiaddr::Protocol;
    addr.iter().find_map(|p| {
        if let Protocol::P2p(peer_id_bytes) = p {
            PeerId::try_from(peer_id_bytes).ok()
        } else {
            None
        }
    })
}
