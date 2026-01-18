//! Harbor Mock Peer Server
//!
//! A standalone mock peer that uses the same libp2p protocols as Harbor
//! for testing P2P connectivity. It will:
//! - Announce itself via mDNS on the local network
//! - Respond to identity exchange requests
//! - Auto-reply to messages with contextual responses

mod protocols;

use clap::Parser;
use futures::StreamExt;
use libp2p::{
    identify, mdns, noise, ping,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, StreamProtocol, Swarm,
};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{info, warn, error, debug};

use protocols::{
    IdentityRequest, IdentityResponse,
    MessagingRequest, MessagingResponse,
    DirectMessage, MessagingMessage, derive_conversation_id,
};

/// Protocol strings (must match Harbor)
const IDENTITY_PROTOCOL: &str = "/harbor/identity/1.0.0";
const MESSAGING_PROTOCOL: &str = "/harbor/messaging/1.0.0";

/// Mock peer command line arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Display name for the mock peer
    #[arg(short, long, default_value = "Mock Peer")]
    name: String,

    /// Bio/description
    #[arg(short, long, default_value = "A mock peer for testing Harbor P2P")]
    bio: String,

    /// TCP port to listen on (0 = random)
    #[arg(short, long, default_value_t = 0)]
    port: u16,
}

/// Combined network behaviour for the mock peer
#[derive(NetworkBehaviour)]
struct MockPeerBehaviour {
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    mdns: mdns::tokio::Behaviour,
    identity_exchange: request_response::cbor::Behaviour<IdentityRequest, IdentityResponse>,
    messaging: request_response::cbor::Behaviour<MessagingRequest, MessagingResponse>,
}

/// Stored peer information (from identity exchange)
struct PeerInfo {
    display_name: String,
    x25519_public: [u8; 32],
}

/// Mock peer state
struct MockPeer {
    /// Display name
    name: String,
    /// Bio
    bio: String,
    /// Ed25519 signing keypair
    signing_key: ed25519_dalek::SigningKey,
    /// X25519 secret key (for encryption)
    x25519_secret: x25519_dalek::StaticSecret,
    /// X25519 public key
    x25519_public: [u8; 32],
    /// libp2p peer ID
    peer_id: PeerId,
    /// Message counter for auto-replies
    message_counter: u64,
    /// Lamport clock for message ordering
    lamport_clock: u64,
    /// Known peers and their X25519 public keys
    known_peers: HashMap<String, PeerInfo>,
}

impl MockPeer {
    fn new(name: String, bio: String, keypair: &libp2p::identity::Keypair) -> Self {
        // Extract Ed25519 key for signing
        let ed25519_keypair = keypair.clone().try_into_ed25519().unwrap();
        let signing_key = ed25519_dalek::SigningKey::from_bytes(
            &ed25519_keypair.secret().as_ref()[..32].try_into().unwrap()
        );

        // Generate X25519 key from Ed25519 (deterministic)
        let x25519_secret = x25519_dalek::StaticSecret::from(
            *signing_key.as_bytes()
        );
        let x25519_public = x25519_dalek::PublicKey::from(&x25519_secret);

        Self {
            name,
            bio,
            signing_key,
            x25519_secret,
            x25519_public: x25519_public.to_bytes(),
            peer_id: keypair.public().to_peer_id(),
            message_counter: 0,
            lamport_clock: 0,
            known_peers: HashMap::new(),
        }
    }

    /// Store a peer's identity info
    fn store_peer(&mut self, peer_id: &str, display_name: String, x25519_public: [u8; 32]) {
        info!("Storing peer info for {}: {}", peer_id, display_name);
        self.known_peers.insert(peer_id.to_string(), PeerInfo {
            display_name,
            x25519_public,
        });
    }

    /// Create identity response for this peer
    fn create_identity_response(&self) -> IdentityResponse {
        use ed25519_dalek::Signer;

        let timestamp = chrono::Utc::now().timestamp();
        let public_key = self.signing_key.verifying_key().to_bytes().to_vec();

        // Create response without signature first
        let mut response = IdentityResponse {
            peer_id: self.peer_id.to_string(),
            public_key: public_key.clone(),
            x25519_public: self.x25519_public.to_vec(),
            display_name: self.name.clone(),
            avatar_hash: None,
            bio: Some(self.bio.clone()),
            timestamp,
            signature: vec![],
        };

        // Sign the response
        let sign_data = format!(
            "{}:{}:{}:{}:{}:{:?}:{}",
            response.peer_id,
            hex::encode(&response.public_key),
            hex::encode(&response.x25519_public),
            response.display_name,
            response.avatar_hash.as_deref().unwrap_or(""),
            response.bio,
            response.timestamp,
        );

        let signature = self.signing_key.sign(sign_data.as_bytes());
        response.signature = signature.to_bytes().to_vec();

        response
    }

    /// Generate an auto-reply message based on content
    fn generate_reply(&mut self, sender_name: &str, _content: &str) -> String {
        self.message_counter += 1;

        // Cycle through different responses
        let responses = [
            format!("Hey! This is {} - a mock peer for testing. Your message was received successfully!", self.name),
            format!("Thanks for testing Harbor's P2P messaging! Connection verified. - {}", self.name),
            format!("Message received loud and clear! The decentralized future is here. - {}", self.name),
            format!("Hello from the mock peer server! Everything is working as expected."),
            format!("Great to connect with you, {}! Harbor's P2P is functioning properly.", sender_name),
        ];

        responses[self.message_counter as usize % responses.len()].clone()
    }

    /// Create a DirectMessage to send as a reply
    fn create_reply_message(&mut self, recipient_peer_id: &str, content: &str) -> Option<DirectMessage> {
        use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
        use aes_gcm::aead::generic_array::GenericArray;
        use ed25519_dalek::Signer;

        // Look up recipient's X25519 public key
        let peer_info = self.known_peers.get(recipient_peer_id)?;

        // Compute shared secret via X25519 key exchange
        let recipient_x25519_public = x25519_dalek::PublicKey::from(peer_info.x25519_public);
        let shared_secret = self.x25519_secret.diffie_hellman(&recipient_x25519_public);

        // Use shared secret as AES-256-GCM key
        let cipher = Aes256Gcm::new(GenericArray::from_slice(shared_secret.as_bytes()));

        // Generate nonce from counter (12 bytes: 4 zero bytes + 8 byte counter)
        self.message_counter += 1;
        let nonce_counter = self.message_counter;
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..12].copy_from_slice(&nonce_counter.to_be_bytes());
        let nonce = GenericArray::from_slice(&nonce_bytes);

        // Encrypt the message content
        let content_encrypted = cipher.encrypt(nonce, content.as_bytes()).ok()?;

        // Update lamport clock
        self.lamport_clock += 1;

        // Build message fields
        let message_id = uuid::Uuid::new_v4().to_string();
        let conversation_id = derive_conversation_id(&self.peer_id.to_string(), recipient_peer_id);
        let timestamp = chrono::Utc::now().timestamp();

        // Create message without signature first
        let mut msg = DirectMessage {
            message_id: message_id.clone(),
            conversation_id: conversation_id.clone(),
            sender_peer_id: self.peer_id.to_string(),
            recipient_peer_id: recipient_peer_id.to_string(),
            content_encrypted,
            content_type: "text".to_string(),
            reply_to: None,
            nonce_counter,
            lamport_clock: self.lamport_clock,
            timestamp,
            signature: vec![],
        };

        // Sign the message
        let sign_data = format!(
            "{}:{}:{}:{}:{}:{}:{:?}:{}:{}:{}",
            msg.message_id,
            msg.conversation_id,
            msg.sender_peer_id,
            msg.recipient_peer_id,
            hex::encode(&msg.content_encrypted),
            msg.content_type,
            msg.reply_to,
            msg.nonce_counter,
            msg.lamport_clock,
            msg.timestamp,
        );
        let signature = self.signing_key.sign(sign_data.as_bytes());
        msg.signature = signature.to_bytes().to_vec();

        Some(msg)
    }

    /// Encode a DirectMessage into the MessagingRequest format
    fn encode_message_request(msg: DirectMessage) -> Option<MessagingRequest> {
        // Wrap in MessagingMessage enum
        let wrapped = MessagingMessage::Message(msg);

        // Encode to CBOR
        let mut payload = Vec::new();
        ciborium::into_writer(&wrapped, &mut payload).ok()?;

        Some(MessagingRequest {
            message_type: "direct_message".to_string(),
            payload,
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("harbor_mock_peer=info".parse()?)
                .add_directive("libp2p_mdns=info".parse()?)
                .add_directive("libp2p_identify=debug".parse()?)
        )
        .init();

    let args = Args::parse();

    info!("Starting Harbor Mock Peer Server");
    info!("Name: {}", args.name);
    info!("Bio: {}", args.bio);

    // Generate a new keypair for this peer
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    let peer_id = keypair.public().to_peer_id();

    info!("Peer ID: {}", peer_id);

    // Create mock peer state
    let mock_peer = MockPeer::new(args.name.clone(), args.bio.clone(), &keypair);

    // Build the swarm
    let mut swarm = build_swarm(keypair.clone())?;

    // Listen on TCP
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", args.port).parse()?;
    swarm.listen_on(listen_addr)?;

    info!("Mock peer is running. Press Ctrl+C to stop.");
    info!("Other Harbor instances on the local network will discover this peer via mDNS.");

    // Store mock peer in a cell for mutation
    let mut mock_peer = mock_peer;

    // Main event loop
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {}/p2p/{}", address, peer_id);
            }

            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                info!("Connected to peer: {} via {:?}", peer_id, endpoint);
            }

            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                info!("Disconnected from peer: {} (cause: {:?})", peer_id, cause);
            }

            SwarmEvent::Behaviour(MockPeerBehaviourEvent::Mdns(event)) => {
                match event {
                    mdns::Event::Discovered(peers) => {
                        for (peer_id, addr) in peers {
                            info!("mDNS discovered peer: {} at {}", peer_id, addr);
                            // Add to Kademlia-like tracking (we could dial them)
                        }
                    }
                    mdns::Event::Expired(peers) => {
                        for (peer_id, _) in peers {
                            debug!("mDNS peer expired: {}", peer_id);
                        }
                    }
                }
            }

            SwarmEvent::Behaviour(MockPeerBehaviourEvent::Ping(event)) => {
                debug!("Ping: {:?}", event);
            }

            SwarmEvent::Behaviour(MockPeerBehaviourEvent::Identify(event)) => {
                match event {
                    identify::Event::Received { peer_id, info, .. } => {
                        info!(
                            "Identified peer {}: {} ({})",
                            peer_id,
                            info.agent_version,
                            info.protocol_version
                        );
                    }
                    identify::Event::Sent { peer_id, .. } => {
                        debug!("Sent identify to {}", peer_id);
                    }
                    _ => {}
                }
            }

            SwarmEvent::Behaviour(MockPeerBehaviourEvent::IdentityExchange(event)) => {
                match event {
                    request_response::Event::Message { peer, message, .. } => {
                        match message {
                            request_response::Message::Request { request, channel, .. } => {
                                info!("Identity request from {}: {:?}", peer, request.requester_peer_id);

                                // Create and send response
                                let response = mock_peer.create_identity_response();
                                info!("Sending identity response: name={}", response.display_name);

                                if let Err(e) = swarm.behaviour_mut().identity_exchange.send_response(channel, response) {
                                    error!("Failed to send identity response: {:?}", e);
                                }
                            }
                            request_response::Message::Response { response, .. } => {
                                info!("Received identity response from {}: {}", peer, response.display_name);

                                // Store the peer's identity info for future encryption
                                if response.x25519_public.len() == 32 {
                                    let mut x25519_arr = [0u8; 32];
                                    x25519_arr.copy_from_slice(&response.x25519_public);
                                    mock_peer.store_peer(
                                        &response.peer_id,
                                        response.display_name.clone(),
                                        x25519_arr,
                                    );
                                } else {
                                    warn!("Invalid X25519 public key length from {}", peer);
                                }
                            }
                        }
                    }
                    request_response::Event::OutboundFailure { peer, error, .. } => {
                        warn!("Identity exchange outbound failure to {}: {:?}", peer, error);
                    }
                    request_response::Event::InboundFailure { peer, error, .. } => {
                        warn!("Identity exchange inbound failure from {}: {:?}", peer, error);
                    }
                    request_response::Event::ResponseSent { peer, .. } => {
                        debug!("Identity response sent to {}", peer);
                    }
                }
            }

            SwarmEvent::Behaviour(MockPeerBehaviourEvent::Messaging(event)) => {
                match event {
                    request_response::Event::Message { peer, message, .. } => {
                        match message {
                            request_response::Message::Request { request, channel, .. } => {
                                info!(
                                    "Message from {}: type={}, payload_len={}",
                                    peer, request.message_type, request.payload.len()
                                );

                                // Get the sender's peer ID string for the reply
                                let sender_peer_id = peer.to_string();

                                // Get peer name for contextual reply
                                let sender_name = mock_peer.known_peers
                                    .get(&sender_peer_id)
                                    .map(|p| p.display_name.clone())
                                    .unwrap_or_else(|| sender_peer_id.clone());

                                // Generate auto-reply content
                                let reply_content = mock_peer.generate_reply(&sender_name, "(encrypted content)");
                                info!("Auto-reply to {}: {}", sender_name, reply_content);

                                // Send success response first (acknowledge receipt)
                                let response = MessagingResponse {
                                    success: true,
                                    message_id: Some(format!("mock-{}", mock_peer.message_counter)),
                                    error: None,
                                };

                                if let Err(e) = swarm.behaviour_mut().messaging.send_response(channel, response) {
                                    error!("Failed to send messaging response: {:?}", e);
                                }

                                // Now send the actual reply message (after a short delay)
                                // Check if we know this peer's X25519 key
                                if mock_peer.known_peers.contains_key(&sender_peer_id) {
                                    // Create the reply DirectMessage
                                    if let Some(reply_msg) = mock_peer.create_reply_message(&sender_peer_id, &reply_content) {
                                        // Encode into MessagingRequest
                                        if let Some(reply_request) = MockPeer::encode_message_request(reply_msg) {
                                            info!("Sending reply message to {}", sender_peer_id);
                                            swarm.behaviour_mut().messaging.send_request(&peer, reply_request);
                                        } else {
                                            warn!("Failed to encode reply message");
                                        }
                                    } else {
                                        warn!("Failed to create reply message for {}", sender_peer_id);
                                    }
                                } else {
                                    info!("No X25519 key known for {} - requesting identity", sender_peer_id);
                                    // We could request their identity here, but for now just log
                                    // In a real implementation, we'd queue the reply and send after identity exchange
                                }
                            }
                            request_response::Message::Response { response, .. } => {
                                if response.success {
                                    info!("Reply delivered to peer: message_id={:?}", response.message_id);
                                } else {
                                    warn!("Reply failed: {:?}", response.error);
                                }
                            }
                        }
                    }
                    request_response::Event::OutboundFailure { peer, error, .. } => {
                        warn!("Messaging outbound failure to {}: {:?}", peer, error);
                    }
                    request_response::Event::InboundFailure { peer, error, .. } => {
                        warn!("Messaging inbound failure from {}: {:?}", peer, error);
                    }
                    request_response::Event::ResponseSent { peer, .. } => {
                        debug!("Messaging response sent to {}", peer);
                    }
                }
            }

            _ => {}
        }
    }
}

/// Build the libp2p swarm with all required behaviours
fn build_swarm(
    keypair: libp2p::identity::Keypair,
) -> Result<Swarm<MockPeerBehaviour>, Box<dyn std::error::Error>> {
    let peer_id = keypair.public().to_peer_id();

    let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|keypair| {
            // Ping for connection liveness
            let ping = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(15)));

            // Identify for peer info exchange
            let identify = identify::Behaviour::new(identify::Config::new(
                "/harbor/1.0.0".to_string(),
                keypair.public(),
            ));

            // mDNS for local discovery
            let mdns = mdns::tokio::Behaviour::new(
                mdns::Config::default(),
                peer_id,
            )?;

            // Identity exchange protocol (using built-in CBOR codec)
            let identity_exchange = request_response::cbor::Behaviour::new(
                [(StreamProtocol::new(IDENTITY_PROTOCOL), ProtocolSupport::Full)],
                request_response::Config::default(),
            );

            // Messaging protocol (using built-in CBOR codec)
            let messaging = request_response::cbor::Behaviour::new(
                [(StreamProtocol::new(MESSAGING_PROTOCOL), ProtocolSupport::Full)],
                request_response::Config::default(),
            );

            Ok(MockPeerBehaviour {
                ping,
                identify,
                mdns,
                identity_exchange,
                messaging,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    Ok(swarm)
}
