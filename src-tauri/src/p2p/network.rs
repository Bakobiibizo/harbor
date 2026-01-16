use futures::StreamExt;
use libp2p::{
    identify, kad, mdns, ping,
    request_response::{self, ResponseChannel},
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm,
};
use std::collections::HashMap;
use base64::Engine;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

use super::behaviour::{ChatBehaviour, ChatBehaviourEvent, IdentityExchangeRequest, IdentityExchangeResponse, MessagingRequest, MessagingResponse};
use super::config::NetworkConfig;
use super::swarm::build_swarm;
use super::types::*;
use crate::error::{AppError, Result};
use crate::services::IdentityService;

/// Handle to interact with the network service
#[derive(Clone)]
pub struct NetworkHandle {
    command_tx: mpsc::Sender<(NetworkCommand, Option<oneshot::Sender<NetworkResponse>>)>,
}

impl NetworkHandle {
    /// Dial a peer at the given addresses
    pub async fn dial(&self, peer_id: PeerId, addresses: Vec<Multiaddr>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::Dial { peer_id, addresses }, Some(tx)))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Get list of connected peers
    pub async fn get_connected_peers(&self) -> Result<Vec<PeerInfo>> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::GetConnectedPeers, Some(tx)))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Peers(peers)) => Ok(peers),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Get network statistics
    pub async fn get_stats(&self) -> Result<NetworkStats> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::GetStats, Some(tx)))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Stats(stats)) => Ok(stats),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Bootstrap the DHT
    pub async fn bootstrap(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::Bootstrap, Some(tx)))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Shutdown the network service
    pub async fn shutdown(&self) -> Result<()> {
        self.command_tx
            .send((NetworkCommand::Shutdown, None))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;
        Ok(())
    }
}

/// The network service manages the libp2p swarm
pub struct NetworkService {
    swarm: Swarm<ChatBehaviour>,
    config: NetworkConfig,
    identity_service: IdentityService,
    command_rx: mpsc::Receiver<(NetworkCommand, Option<oneshot::Sender<NetworkResponse>>)>,
    event_tx: mpsc::Sender<NetworkEvent>,
    connected_peers: HashMap<PeerId, PeerInfo>,
    discovered_peers: HashMap<PeerId, Vec<Multiaddr>>,
    stats: NetworkStats,
    start_time: Instant,
    pending_identity_responses: HashMap<request_response::InboundRequestId, ResponseChannel<IdentityExchangeResponse>>,
    pending_messaging_responses: HashMap<request_response::InboundRequestId, ResponseChannel<MessagingResponse>>,
}

impl NetworkService {
    /// Create a new network service
    pub fn new(
        config: NetworkConfig,
        identity_service: IdentityService,
        keypair: libp2p::identity::Keypair,
    ) -> Result<(Self, NetworkHandle, mpsc::Receiver<NetworkEvent>)> {
        let swarm = build_swarm(keypair, &config)?;

        let (command_tx, command_rx) = mpsc::channel(256);
        let (event_tx, event_rx) = mpsc::channel(256);

        let handle = NetworkHandle { command_tx };

        let service = Self {
            swarm,
            config,
            identity_service,
            command_rx,
            event_tx,
            connected_peers: HashMap::new(),
            discovered_peers: HashMap::new(),
            stats: NetworkStats::default(),
            start_time: Instant::now(),
            pending_identity_responses: HashMap::new(),
            pending_messaging_responses: HashMap::new(),
        };

        Ok((service, handle, event_rx))
    }

    /// Get the local peer ID
    pub fn local_peer_id(&self) -> &PeerId {
        self.swarm.local_peer_id()
    }

    /// Start listening on configured addresses
    pub fn start_listening(&mut self) -> Result<()> {
        // Listen on TCP
        let tcp_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", self.config.tcp_port)
            .parse()
            .map_err(|e| AppError::Network(format!("Invalid TCP address: {}", e)))?;
        self.swarm.listen_on(tcp_addr.clone())?;
        info!("Listening on TCP: {}", tcp_addr);

        // Listen on QUIC
        let quic_addr: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", self.config.quic_port)
            .parse()
            .map_err(|e| AppError::Network(format!("Invalid QUIC address: {}", e)))?;
        self.swarm.listen_on(quic_addr.clone())?;
        info!("Listening on QUIC: {}", quic_addr);

        Ok(())
    }

    /// Run the network event loop
    pub async fn run(mut self) {
        info!("Network service starting...");

        if let Err(e) = self.start_listening() {
            error!("Failed to start listening: {}", e);
            return;
        }

        loop {
            tokio::select! {
                // Handle swarm events
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await;
                }

                // Handle commands from the application
                Some((command, response_tx)) = self.command_rx.recv() => {
                    let should_shutdown = matches!(command, NetworkCommand::Shutdown);
                    let response = self.handle_command(command).await;
                    if let Some(tx) = response_tx {
                        let _ = tx.send(response);
                    }
                    if should_shutdown {
                        info!("Network service shutting down...");
                        break;
                    }
                }
            }
        }
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<ChatBehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on: {}", address);
                let _ = self.event_tx.send(NetworkEvent::ListeningOn {
                    address: address.to_string(),
                }).await;
            }

            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                info!("Connected to peer: {} at {:?}", peer_id, endpoint);
                let peer_info = PeerInfo {
                    peer_id: peer_id.to_string(),
                    addresses: vec![endpoint.get_remote_address().to_string()],
                    protocol_version: None,
                    agent_version: None,
                    is_connected: true,
                    last_seen: Some(chrono::Utc::now().timestamp()),
                };
                self.connected_peers.insert(peer_id, peer_info);
                self.stats.connected_peers = self.connected_peers.len();

                let _ = self.event_tx.send(NetworkEvent::PeerConnected {
                    peer_id: peer_id.to_string(),
                }).await;
            }

            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                info!("Disconnected from peer: {} (cause: {:?})", peer_id, cause);
                self.connected_peers.remove(&peer_id);
                self.stats.connected_peers = self.connected_peers.len();

                let _ = self.event_tx.send(NetworkEvent::PeerDisconnected {
                    peer_id: peer_id.to_string(),
                }).await;
            }

            SwarmEvent::ExternalAddrConfirmed { address } => {
                info!("External address confirmed: {}", address);
                let _ = self.event_tx.send(NetworkEvent::ExternalAddressDiscovered {
                    address: address.to_string(),
                }).await;
            }

            SwarmEvent::Behaviour(behaviour_event) => {
                self.handle_behaviour_event(behaviour_event).await;
            }

            _ => {}
        }
    }

    async fn handle_behaviour_event(&mut self, event: ChatBehaviourEvent) {
        match event {
            ChatBehaviourEvent::Mdns(mdns::Event::Discovered(peers)) => {
                for (peer_id, addr) in peers {
                    info!("mDNS discovered peer: {} at {}", peer_id, addr);
                    self.discovered_peers
                        .entry(peer_id)
                        .or_default()
                        .push(addr.clone());

                    // Add to Kademlia routing table
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);

                    let _ = self.event_tx.send(NetworkEvent::PeerDiscovered {
                        peer_id: peer_id.to_string(),
                    }).await;
                }
            }

            ChatBehaviourEvent::Mdns(mdns::Event::Expired(peers)) => {
                for (peer_id, addr) in peers {
                    debug!("mDNS peer expired: {} at {}", peer_id, addr);
                    if let Some(addrs) = self.discovered_peers.get_mut(&peer_id) {
                        addrs.retain(|a| a != &addr);
                        if addrs.is_empty() {
                            self.discovered_peers.remove(&peer_id);
                            let _ = self.event_tx.send(NetworkEvent::PeerExpired {
                                peer_id: peer_id.to_string(),
                            }).await;
                        }
                    }
                }
            }

            ChatBehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. }) => {
                debug!("Identified peer: {} - {}", peer_id, info.agent_version);
                if let Some(peer_info) = self.connected_peers.get_mut(&peer_id) {
                    peer_info.protocol_version = Some(info.protocol_version);
                    peer_info.agent_version = Some(info.agent_version);
                }

                // Add addresses to Kademlia
                for addr in info.listen_addrs {
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                }
            }

            ChatBehaviourEvent::Kademlia(kad::Event::RoutingUpdated { peer, .. }) => {
                debug!("Kademlia routing updated for peer: {}", peer);
            }

            ChatBehaviourEvent::Ping(ping::Event { peer, result, .. }) => {
                if let Ok(rtt) = result {
                    debug!("Ping to {} succeeded: {:?}", peer, rtt);
                }
            }

            ChatBehaviourEvent::IdentityExchange(request_response::Event::Message { peer, message }) => {
                match message {
                    request_response::Message::Request { request_id, request, channel } => {
                        info!("Received identity request from {}", peer);
                        self.handle_identity_request(peer, request_id, request, channel).await;
                    }
                    request_response::Message::Response { request_id, response } => {
                        info!("Received identity response from {}", peer);
                        self.handle_identity_response(peer, request_id, response).await;
                    }
                }
            }

            ChatBehaviourEvent::Messaging(request_response::Event::Message { peer, message }) => {
                match message {
                    request_response::Message::Request { request_id, request, channel } => {
                        debug!("Received message request from {}", peer);
                        self.handle_messaging_request(peer, request_id, request, channel).await;
                    }
                    request_response::Message::Response { request_id: _, response: _ } => {
                        debug!("Received message response from {}", peer);
                        // Handle response (e.g., update message delivery status)
                    }
                }
            }

            _ => {}
        }
    }

    async fn handle_identity_request(
        &mut self,
        _peer: PeerId,
        _request_id: request_response::InboundRequestId,
        _request: IdentityExchangeRequest,
        channel: ResponseChannel<IdentityExchangeResponse>,
    ) {
        // Get our identity info to respond with
        match self.identity_service.get_identity_info() {
            Ok(Some(info)) => {
                // Sign the response
                let timestamp = chrono::Utc::now().timestamp();
                let signature = match self.identity_service.sign_raw(
                    format!("{}:{}:{}", info.peer_id, info.display_name, timestamp).as_bytes()
                ) {
                    Ok(sig) => sig,
                    Err(e) => {
                        warn!("Failed to sign identity response: {}", e);
                        return;
                    }
                };

                // Decode base64 public keys to bytes for the network protocol
                let engine = base64::engine::general_purpose::STANDARD;
                let public_key = match engine.decode(&info.public_key) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        warn!("Failed to decode public key: {}", e);
                        return;
                    }
                };
                let x25519_public = match engine.decode(&info.x25519_public) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        warn!("Failed to decode x25519 public key: {}", e);
                        return;
                    }
                };

                let response = IdentityExchangeResponse {
                    peer_id: info.peer_id,
                    public_key,
                    x25519_public,
                    display_name: info.display_name,
                    avatar_hash: info.avatar_hash,
                    bio: info.bio,
                    timestamp,
                    signature,
                };

                if let Err(e) = self.swarm.behaviour_mut().identity_exchange.send_response(channel, response) {
                    warn!("Failed to send identity response: {:?}", e);
                }
            }
            Ok(None) => {
                warn!("No identity configured, cannot respond to identity request");
            }
            Err(e) => {
                warn!("Failed to get identity info: {}", e);
            }
        }
    }

    async fn handle_identity_response(
        &mut self,
        peer: PeerId,
        _request_id: request_response::OutboundRequestId,
        response: IdentityExchangeResponse,
    ) {
        info!(
            "Got identity from {}: {} ({})",
            peer, response.display_name, response.peer_id
        );
        // TODO: Store in contacts database, verify signature
    }

    async fn handle_messaging_request(
        &mut self,
        peer: PeerId,
        _request_id: request_response::InboundRequestId,
        request: MessagingRequest,
        channel: ResponseChannel<MessagingResponse>,
    ) {
        // TODO: Process incoming message, store, emit event
        let response = MessagingResponse {
            success: true,
            message_id: None,
            error: None,
        };

        if let Err(e) = self.swarm.behaviour_mut().messaging.send_response(channel, response) {
            warn!("Failed to send messaging response: {:?}", e);
        }

        // Emit event for the application layer
        let _ = self.event_tx.send(NetworkEvent::MessageReceived {
            peer_id: peer.to_string(),
            protocol: "messaging".to_string(),
            payload: request.payload,
        }).await;
    }

    async fn handle_command(&mut self, command: NetworkCommand) -> NetworkResponse {
        match command {
            NetworkCommand::Dial { peer_id, addresses } => {
                for addr in addresses {
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                }
                match self.swarm.dial(peer_id) {
                    Ok(_) => NetworkResponse::Ok,
                    Err(e) => NetworkResponse::Error(format!("Failed to dial: {}", e)),
                }
            }

            NetworkCommand::Disconnect { peer_id } => {
                match self.swarm.disconnect_peer_id(peer_id) {
                    Ok(_) => NetworkResponse::Ok,
                    Err(e) => NetworkResponse::Error(format!("Failed to disconnect: {:?}", e)),
                }
            }

            NetworkCommand::SendMessage { peer_id, protocol, payload } => {
                let request = MessagingRequest {
                    message_type: protocol,
                    payload,
                };
                self.swarm.behaviour_mut().messaging.send_request(&peer_id, request);
                NetworkResponse::Ok
            }

            NetworkCommand::GetStats => {
                let mut stats = self.stats.clone();
                stats.uptime_seconds = self.start_time.elapsed().as_secs();
                NetworkResponse::Stats(stats)
            }

            NetworkCommand::GetConnectedPeers => {
                let peers: Vec<PeerInfo> = self.connected_peers.values().cloned().collect();
                NetworkResponse::Peers(peers)
            }

            NetworkCommand::Bootstrap => {
                if let Err(e) = self.swarm.behaviour_mut().kademlia.bootstrap() {
                    NetworkResponse::Error(format!("Bootstrap failed: {:?}", e))
                } else {
                    NetworkResponse::Ok
                }
            }

            NetworkCommand::Shutdown => {
                NetworkResponse::Ok
            }
        }
    }
}
