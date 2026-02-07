use base64::Engine;
use futures::StreamExt;
use libp2p::{
    autonat, dcutr, identify, kad, mdns, ping, relay,
    request_response::{self, ResponseChannel},
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm,
};
use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

/// Public relay servers that support libp2p relay v2
/// Only Harbor relay servers are listed here. IPFS bootstrap nodes use relay v1
/// and RSA-based peer IDs that are incompatible with relay v2.
const PUBLIC_RELAYS: &[&str] = &[
    // Harbor community relay (primary) - IPv6
    "/ip4/52.200.206.197/tcp/4001/p2p/12D3KooWHi81G15poZuH4BL5WnifQ3b2S2uSkvsa1wAhBZNA8PW9",
];

/// Public relay servers specifically for relay circuits (if available)
/// These can be used as fallback when the bootstrap nodes don't provide relay service
#[allow(dead_code)]
const FALLBACK_RELAYS: &[&str] = &[
    // Placeholder for future Harbor-specific relay servers
    // Users can deploy their own using the AWS CloudFormation template
];

use super::behaviour::{
    ChatBehaviour, ChatBehaviourEvent, ContentSyncRequest, ContentSyncResponse,
    IdentityExchangeRequest, IdentityExchangeResponse, MessagingRequest, MessagingResponse,
    PostSummaryProto,
};
use super::config::NetworkConfig;
use super::protocols::board_sync::{
    BoardSyncRequest as WireBoardSyncRequest, BoardSyncResponse as WireBoardSyncResponse,
};
use super::protocols::messaging::{MessagingCodec, MessagingMessage};
use super::swarm::build_swarm;
use super::types::*;
use crate::db::Capability;
use crate::error::{AppError, Result};
use crate::services::board_service::StorableBoardPost;
use crate::services::{
    BoardService, ContactsService, ContentSyncService, IdentityService, MessagingService,
    PermissionsService, PostsService,
};
use std::sync::Arc;

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

    /// Send a message to a peer
    pub async fn send_message(
        &self,
        peer_id: PeerId,
        protocol: String,
        payload: Vec<u8>,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::SendMessage {
                    peer_id,
                    protocol,
                    payload,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Request identity from a peer
    pub async fn request_identity(&self, peer_id: PeerId) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::RequestIdentity { peer_id }, Some(tx)))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Get listening addresses (with peer ID appended)
    pub async fn get_listening_addresses(&self) -> Result<Vec<String>> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::GetListeningAddresses, Some(tx)))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Addresses(addrs)) => Ok(addrs),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Add a bootstrap node and dial it
    pub async fn add_bootstrap_node(&self, address: Multiaddr) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::AddBootstrapNode { address }, Some(tx)))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Add a custom relay server and attempt to get a relay reservation
    pub async fn add_relay_server(&self, address: Multiaddr) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::AddRelayServer { address }, Some(tx)))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Request content manifest from a peer
    pub async fn request_content_manifest(
        &self,
        peer_id: PeerId,
        cursor: std::collections::HashMap<String, u64>,
        limit: u32,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::RequestContentManifest {
                    peer_id,
                    cursor,
                    limit,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Join a community (register peer and list boards)
    pub async fn join_community(&self, relay_peer_id: PeerId, relay_address: String) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::JoinCommunity {
                    relay_peer_id,
                    relay_address,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// List boards on a relay
    pub async fn list_boards(&self, relay_peer_id: PeerId) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::ListBoards { relay_peer_id }, Some(tx)))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Get board posts from a relay
    pub async fn get_board_posts(
        &self,
        relay_peer_id: PeerId,
        board_id: String,
        after_timestamp: Option<i64>,
        limit: u32,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::GetBoardPosts {
                    relay_peer_id,
                    board_id,
                    after_timestamp,
                    limit,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Submit a board post to a relay
    pub async fn submit_board_post(
        &self,
        relay_peer_id: PeerId,
        board_id: String,
        content_text: String,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::SubmitBoardPost {
                    relay_peer_id,
                    board_id,
                    content_text,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Delete a board post on a relay
    pub async fn delete_board_post(&self, relay_peer_id: PeerId, post_id: String) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::DeleteBoardPost {
                    relay_peer_id,
                    post_id,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Connect to public relay servers for NAT traversal
    pub async fn connect_to_public_relays(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::ConnectToPublicRelays, Some(tx)))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Request content fetch from a peer
    pub async fn request_content_fetch(
        &self,
        peer_id: PeerId,
        post_id: String,
        include_media: bool,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((
                NetworkCommand::RequestContentFetch {
                    peer_id,
                    post_id,
                    include_media,
                },
                Some(tx),
            ))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }

    /// Trigger feed content sync from connected peers
    pub async fn sync_feed(&self, limit: u32) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send((NetworkCommand::SyncFeed { limit }, Some(tx)))
            .await
            .map_err(|_| AppError::Internal("Network service unavailable".into()))?;

        match rx.await {
            Ok(NetworkResponse::Ok) => Ok(()),
            Ok(NetworkResponse::Error(e)) => Err(AppError::Network(e)),
            _ => Err(AppError::Internal("Unexpected response".into())),
        }
    }
}

use super::types::NatStatus;

/// The network service manages the libp2p swarm
pub struct NetworkService {
    swarm: Swarm<ChatBehaviour>,
    config: NetworkConfig,
    identity_service: Arc<IdentityService>,
    messaging_service: Option<Arc<MessagingService>>,
    contacts_service: Option<Arc<ContactsService>>,
    permissions_service: Option<Arc<PermissionsService>>,
    posts_service: Option<Arc<PostsService>>,
    content_sync_service: Option<Arc<ContentSyncService>>,
    board_service: Option<Arc<BoardService>>,
    command_rx: mpsc::Receiver<(NetworkCommand, Option<oneshot::Sender<NetworkResponse>>)>,
    event_tx: mpsc::Sender<NetworkEvent>,
    connected_peers: HashMap<PeerId, PeerInfo>,
    discovered_peers: HashMap<PeerId, Vec<Multiaddr>>,
    listening_addresses: Vec<Multiaddr>,
    stats: NetworkStats,
    start_time: Instant,
    /// Current NAT status
    nat_status: NatStatus,
    /// Relay addresses we're reachable at
    relay_addresses: Vec<Multiaddr>,
    /// External addresses discovered via AutoNAT
    external_addresses: Vec<Multiaddr>,
    /// Whether we've attempted to connect to relays
    relay_connection_attempted: bool,
    /// Relay peers we've dialed but haven't yet requested a reservation for.
    /// Key: relay peer ID, Value: full relay multiaddr (transport + /p2p/<id>).
    /// Reservation is requested in Identify::Received after the connection is fully negotiated.
    pending_relay_reservations: HashMap<PeerId, Multiaddr>,
}

impl NetworkService {
    /// Create a new network service
    pub fn new(
        config: NetworkConfig,
        identity_service: Arc<IdentityService>,
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
            messaging_service: None,
            contacts_service: None,
            permissions_service: None,
            posts_service: None,
            content_sync_service: None,
            board_service: None,
            command_rx,
            event_tx,
            connected_peers: HashMap::new(),
            discovered_peers: HashMap::new(),
            listening_addresses: Vec::new(),
            stats: NetworkStats::default(),
            start_time: Instant::now(),
            nat_status: NatStatus::Unknown,
            relay_addresses: Vec::new(),
            external_addresses: Vec::new(),
            relay_connection_attempted: false,
            pending_relay_reservations: HashMap::new(),
        };

        Ok((service, handle, event_rx))
    }

    /// Set the messaging service for processing incoming messages
    pub fn set_messaging_service(&mut self, service: Arc<MessagingService>) {
        self.messaging_service = Some(service);
    }

    /// Set the contacts service for storing contacts from identity exchange
    pub fn set_contacts_service(&mut self, service: Arc<ContactsService>) {
        self.contacts_service = Some(service);
    }

    /// Set the permissions service for granting permissions to contacts
    pub fn set_permissions_service(&mut self, service: Arc<PermissionsService>) {
        self.permissions_service = Some(service);
    }

    /// Set the posts service for serving post fetch requests
    pub fn set_posts_service(&mut self, service: Arc<PostsService>) {
        self.posts_service = Some(service);
    }

    /// Set content sync service for handling manifest processing + storage
    pub fn set_content_sync_service(&mut self, service: Arc<ContentSyncService>) {
        self.content_sync_service = Some(service);
    }

    /// Set board service for community board operations
    pub fn set_board_service(&mut self, service: Arc<BoardService>) {
        self.board_service = Some(service);
    }

    /// Get the local peer ID
    pub fn local_peer_id(&self) -> &PeerId {
        self.swarm.local_peer_id()
    }

    /// Create an identity exchange request
    fn create_identity_request(&self) -> Result<IdentityExchangeRequest> {
        let info = self
            .identity_service
            .get_identity_info()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let timestamp = chrono::Utc::now().timestamp();
        let signature = self
            .identity_service
            .sign_raw(format!("{}:{}", info.peer_id, timestamp).as_bytes())?;

        Ok(IdentityExchangeRequest {
            requester_peer_id: info.peer_id,
            timestamp,
            signature,
        })
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

        // Auto-connect to relay on start (don't wait for AutoNAT)
        info!("Auto-connecting to Harbor relay...");
        self.connect_to_relays().await;

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
                self.listening_addresses.push(address.clone());
                let _ = self
                    .event_tx
                    .send(NetworkEvent::ListeningOn {
                        address: address.to_string(),
                    })
                    .await;
            }

            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
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

                let _ = self
                    .event_tx
                    .send(NetworkEvent::PeerConnected {
                        peer_id: peer_id.to_string(),
                    })
                    .await;
            }

            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                info!("Disconnected from peer: {} (cause: {:?})", peer_id, cause);
                self.connected_peers.remove(&peer_id);
                self.stats.connected_peers = self.connected_peers.len();

                let _ = self
                    .event_tx
                    .send(NetworkEvent::PeerDisconnected {
                        peer_id: peer_id.to_string(),
                    })
                    .await;
            }

            SwarmEvent::ExternalAddrConfirmed { address } => {
                info!("External address confirmed: {}", address);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::ExternalAddressDiscovered {
                        address: address.to_string(),
                    })
                    .await;
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    warn!("Failed to connect to peer {}: {}", peer_id, error);
                } else {
                    warn!("Outgoing connection error: {}", error);
                }
            }

            SwarmEvent::Behaviour(behaviour_event) => {
                self.handle_behaviour_event(behaviour_event).await;
            }

            _ => {}
        }
    }

    async fn handle_content_sync_request(
        &mut self,
        peer: PeerId,
        _request_id: request_response::InboundRequestId,
        request: ContentSyncRequest,
        channel: ResponseChannel<ContentSyncResponse>,
    ) {
        let Some(ref content_sync_service) = self.content_sync_service else {
            let _ = self.swarm.behaviour_mut().content_sync.send_response(
                channel,
                ContentSyncResponse::Error {
                    error: "Content sync service unavailable".to_string(),
                },
            );
            return;
        };

        match request {
            ContentSyncRequest::Manifest {
                requester_peer_id,
                cursor,
                limit,
                timestamp,
                signature,
            } => {
                // Ensure peer id matches claimed requester
                if requester_peer_id != peer.to_string() {
                    let _ = self.swarm.behaviour_mut().content_sync.send_response(
                        channel,
                        ContentSyncResponse::Error {
                            error: "requester_peer_id mismatch".to_string(),
                        },
                    );
                    return;
                }

                match content_sync_service.process_manifest_request(
                    &requester_peer_id,
                    &cursor,
                    limit,
                    timestamp,
                    &signature,
                ) {
                    Ok(resp) => {
                        let response = ContentSyncResponse::Manifest {
                            responder_peer_id: resp.responder_peer_id,
                            posts: resp
                                .posts
                                .into_iter()
                                .map(|p| PostSummaryProto {
                                    post_id: p.post_id,
                                    author_peer_id: p.author_peer_id,
                                    lamport_clock: p.lamport_clock,
                                    content_type: p.content_type,
                                    has_media: p.has_media,
                                    media_hashes: p.media_hashes,
                                    created_at: p.created_at,
                                })
                                .collect(),
                            has_more: resp.has_more,
                            next_cursor: resp.next_cursor,
                            timestamp: resp.timestamp,
                            signature: resp.signature,
                        };

                        if let Err(e) = self
                            .swarm
                            .behaviour_mut()
                            .content_sync
                            .send_response(channel, response)
                        {
                            warn!("Failed to send content manifest response: {:?}", e);
                        }
                    }
                    Err(e) => {
                        let _ = self.swarm.behaviour_mut().content_sync.send_response(
                            channel,
                            ContentSyncResponse::Error {
                                error: e.to_string(),
                            },
                        );
                    }
                }
            }
            ContentSyncRequest::FetchPost {
                post_id,
                include_media,
                requester_peer_id,
                timestamp,
                signature,
            } => {
                // Ensure peer id matches claimed requester
                if requester_peer_id != peer.to_string() {
                    let _ = self.swarm.behaviour_mut().content_sync.send_response(
                        channel,
                        ContentSyncResponse::Error {
                            error: "requester_peer_id mismatch".to_string(),
                        },
                    );
                    return;
                }

                match content_sync_service.process_fetch_request(
                    &requester_peer_id,
                    &post_id,
                    include_media,
                    timestamp,
                    &signature,
                ) {
                    Ok(resp) => {
                        let response = ContentSyncResponse::Post {
                            post_id: resp.post_id,
                            author_peer_id: resp.author_peer_id,
                            content_type: resp.content_type,
                            content_text: resp.content_text,
                            visibility: resp.visibility,
                            lamport_clock: resp.lamport_clock,
                            created_at: resp.created_at,
                            signature: resp.signature,
                        };

                        if let Err(e) = self
                            .swarm
                            .behaviour_mut()
                            .content_sync
                            .send_response(channel, response)
                        {
                            warn!("Failed to send fetch post response: {:?}", e);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to process fetch request from {}: {}", peer, e);
                        let _ = self.swarm.behaviour_mut().content_sync.send_response(
                            channel,
                            ContentSyncResponse::Error {
                                error: e.to_string(),
                            },
                        );
                    }
                }
            }
        }
    }

    async fn handle_content_sync_response(
        &mut self,
        peer: PeerId,
        _request_id: request_response::OutboundRequestId,
        response: ContentSyncResponse,
    ) {
        let Some(ref content_sync_service) = self.content_sync_service else {
            return;
        };

        match response {
            ContentSyncResponse::Manifest {
                responder_peer_id,
                posts,
                has_more,
                next_cursor,
                timestamp,
                signature,
            } => {
                if responder_peer_id != peer.to_string() {
                    warn!(
                        "Content manifest responder mismatch: expected {}, got {}",
                        peer, responder_peer_id
                    );
                    return;
                }

                // Convert wire format to service format
                let service_posts: Vec<crate::services::PostSummary> = posts
                    .into_iter()
                    .map(|p| crate::services::PostSummary {
                        post_id: p.post_id,
                        author_peer_id: p.author_peer_id,
                        lamport_clock: p.lamport_clock,
                        content_type: p.content_type,
                        has_media: p.has_media,
                        media_hashes: p.media_hashes,
                        created_at: p.created_at,
                    })
                    .collect();

                match content_sync_service.process_manifest_response(
                    &responder_peer_id,
                    &service_posts,
                    has_more,
                    &next_cursor,
                    timestamp,
                    &signature,
                ) {
                    Ok(posts_to_fetch) => {
                        // Emit manifest received event
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::ContentManifestReceived {
                                peer_id: peer.to_string(),
                                post_count: posts_to_fetch.len(),
                                has_more,
                            })
                            .await;

                        // Issue fetch requests for posts we need
                        for post_id in posts_to_fetch {
                            match content_sync_service.create_fetch_request(post_id.clone(), false)
                            {
                                Ok(fetch_req) => {
                                    let request = ContentSyncRequest::FetchPost {
                                        post_id: fetch_req.post_id,
                                        include_media: fetch_req.include_media,
                                        requester_peer_id: fetch_req.requester_peer_id,
                                        timestamp: fetch_req.timestamp,
                                        signature: fetch_req.signature,
                                    };
                                    self.swarm
                                        .behaviour_mut()
                                        .content_sync
                                        .send_request(&peer, request);
                                    debug!("Sent fetch request for post {} to {}", post_id, peer);
                                }
                                Err(e) => {
                                    warn!("Failed to create fetch request for {}: {}", post_id, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to process manifest response: {}", e);
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::ContentSyncError {
                                peer_id: peer.to_string(),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            }
            ContentSyncResponse::Post {
                post_id,
                author_peer_id,
                content_type,
                content_text,
                visibility,
                lamport_clock,
                created_at,
                signature,
            } => {
                info!("Received post {} from {}", post_id, peer);

                // Verify the author matches the peer we requested from
                if author_peer_id != peer.to_string() {
                    warn!(
                        "Post author mismatch: expected {}, got {}",
                        peer, author_peer_id
                    );
                    return;
                }

                // Store the remote post
                match content_sync_service.store_remote_post(
                    &post_id,
                    &author_peer_id,
                    &content_type,
                    content_text.as_deref(),
                    &visibility,
                    lamport_clock,
                    created_at,
                    &signature,
                ) {
                    Ok(_) => {
                        info!("Stored remote post {} from {}", post_id, peer);
                        // Emit event for UI to refresh feed
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::ContentFetched {
                                peer_id: peer.to_string(),
                                post_id,
                            })
                            .await;
                    }
                    Err(e) => {
                        warn!("Failed to store remote post {}: {}", post_id, e);
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::ContentSyncError {
                                peer_id: peer.to_string(),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            }
            ContentSyncResponse::Error { error } => {
                warn!("Content sync error from {}: {}", peer, error);
            }
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
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr);

                    let _ = self
                        .event_tx
                        .send(NetworkEvent::PeerDiscovered {
                            peer_id: peer_id.to_string(),
                        })
                        .await;
                }
            }

            ChatBehaviourEvent::Mdns(mdns::Event::Expired(peers)) => {
                for (peer_id, addr) in peers {
                    debug!("mDNS peer expired: {} at {}", peer_id, addr);
                    if let Some(addrs) = self.discovered_peers.get_mut(&peer_id) {
                        addrs.retain(|a| a != &addr);
                        if addrs.is_empty() {
                            self.discovered_peers.remove(&peer_id);
                            let _ = self
                                .event_tx
                                .send(NetworkEvent::PeerExpired {
                                    peer_id: peer_id.to_string(),
                                })
                                .await;
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
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr);
                }

                // If this peer is a relay we're waiting on, request the reservation NOW.
                // This is the correct timing — the connection is fully negotiated and
                // the relay client transport knows about it.
                if let Some(relay_addr) = self.pending_relay_reservations.remove(&peer_id) {
                    let circuit_listen_addr: Multiaddr = relay_addr
                        .clone()
                        .with(libp2p::multiaddr::Protocol::P2pCircuit);
                    info!(
                        "Requesting relay reservation on {} (post-identify)",
                        circuit_listen_addr
                    );
                    match self.swarm.listen_on(circuit_listen_addr.clone()) {
                        Ok(id) => {
                            info!(
                                "Relay listener registered: {:?} on {}",
                                id, circuit_listen_addr
                            );
                        }
                        Err(e) => {
                            warn!(
                                "Failed to request relay reservation {}: {}",
                                circuit_listen_addr, e
                            );
                        }
                    }
                }
            }

            ChatBehaviourEvent::Kademlia(kad::Event::RoutingUpdated { peer, .. }) => {
                debug!("Kademlia routing updated for peer: {}", peer);
            }

            ChatBehaviourEvent::Ping(ping::Event {
                peer,
                result: Ok(rtt),
                ..
            }) => {
                debug!("Ping to {} succeeded: {:?}", peer, rtt);
            }
            ChatBehaviourEvent::Ping(_) => {}

            ChatBehaviourEvent::IdentityExchange(request_response::Event::Message {
                peer,
                message,
                ..
            }) => match message {
                request_response::Message::Request {
                    request_id,
                    request,
                    channel,
                } => {
                    info!("Received identity request from {}", peer);
                    self.handle_identity_request(peer, request_id, request, channel)
                        .await;
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    info!("Received identity response from {}", peer);
                    self.handle_identity_response(peer, request_id, response)
                        .await;
                }
            },

            ChatBehaviourEvent::Messaging(request_response::Event::Message {
                peer,
                message,
                ..
            }) => {
                match message {
                    request_response::Message::Request {
                        request_id,
                        request,
                        channel,
                    } => {
                        debug!("Received message request from {}", peer);
                        self.handle_messaging_request(peer, request_id, request, channel)
                            .await;
                    }
                    request_response::Message::Response {
                        request_id: _,
                        response: _,
                    } => {
                        debug!("Received message response from {}", peer);
                        // Handle response (e.g., update message delivery status)
                    }
                }
            }
            ChatBehaviourEvent::ContentSync(request_response::Event::Message {
                peer,
                message,
                ..
            }) => match message {
                request_response::Message::Request {
                    request_id,
                    request,
                    channel,
                } => {
                    debug!("Received content sync request from {}", peer);
                    self.handle_content_sync_request(peer, request_id, request, channel)
                        .await;
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    debug!("Received content sync response from {}", peer);
                    self.handle_content_sync_response(peer, request_id, response)
                        .await;
                }
            },

            // Board sync events
            ChatBehaviourEvent::BoardSync(request_response::Event::Message {
                peer,
                message,
                ..
            }) => match message {
                request_response::Message::Request { channel, .. } => {
                    // Client doesn't serve board requests; send error
                    let _ = self.swarm.behaviour_mut().board_sync.send_response(
                        channel,
                        WireBoardSyncResponse::Error {
                            error: "Not a relay server".to_string(),
                        },
                    );
                }
                request_response::Message::Response { response, .. } => {
                    self.handle_board_sync_response(peer, response).await;
                }
            },

            // Relay client events for NAT traversal
            ChatBehaviourEvent::RelayClient(event) => {
                self.handle_relay_client_event(event).await;
            }

            // DCUtR events for hole punching
            ChatBehaviourEvent::Dcutr(event) => {
                self.handle_dcutr_event(event).await;
            }

            // AutoNAT events for NAT detection
            ChatBehaviourEvent::Autonat(event) => {
                self.handle_autonat_event(event).await;
            }

            _ => {}
        }
    }

    /// Handle relay client events
    async fn handle_relay_client_event(&mut self, event: relay::client::Event) {
        match event {
            relay::client::Event::ReservationReqAccepted {
                relay_peer_id,
                renewal,
                limit: _,
            } => {
                let local_peer_id = *self.swarm.local_peer_id();
                info!(
                    "Relay reservation accepted by {} (renewal: {})",
                    relay_peer_id, renewal
                );

                // Build full relay circuit address WITH transport prefix.
                // Look up the relay peer's transport address from connected peers
                // so other peers can actually reach us through this relay.
                let mut relay_circuit_addr: Option<Multiaddr> = None;

                if let Some(peer_info) = self.connected_peers.get(&relay_peer_id) {
                    for addr_str in &peer_info.addresses {
                        if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                            // Strip /p2p/ from the address to get transport-only
                            let transport_addr: Multiaddr = addr
                                .iter()
                                .filter(|p| !matches!(p, libp2p::multiaddr::Protocol::P2p(_)))
                                .collect();

                            if transport_addr.to_string().is_empty() {
                                continue;
                            }

                            // Build: TRANSPORT/p2p/RELAY_ID/p2p-circuit/p2p/LOCAL_ID
                            let full_circuit_addr: Multiaddr = format!(
                                "{}/p2p/{}/p2p-circuit/p2p/{}",
                                transport_addr, relay_peer_id, local_peer_id
                            )
                            .parse()
                            .expect("valid relay circuit multiaddr");

                            relay_circuit_addr = Some(full_circuit_addr);
                            break;
                        }
                    }
                }

                // Fallback: if we couldn't find a transport address, use bare p2p form
                let relay_circuit_addr = relay_circuit_addr.unwrap_or_else(|| {
                    format!("/p2p/{}/p2p-circuit/p2p/{}", relay_peer_id, local_peer_id)
                        .parse()
                        .expect("valid relay circuit multiaddr")
                });

                // Register as external address so Identify advertises it to other peers
                self.swarm.add_external_address(relay_circuit_addr.clone());
                info!(
                    "Added relay circuit as external address: {}",
                    relay_circuit_addr
                );

                // Store the relay address if not already present
                if !self.relay_addresses.contains(&relay_circuit_addr) {
                    self.relay_addresses.push(relay_circuit_addr.clone());
                    info!("Added relay address: {}", relay_circuit_addr);

                    // Emit event to frontend
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::RelayConnected {
                            relay_address: relay_circuit_addr.to_string(),
                        })
                        .await;
                }

                // Update NAT status to Private (we're behind NAT but reachable via relay)
                if self.nat_status != NatStatus::Public {
                    self.nat_status = NatStatus::Private;
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::NatStatusChanged {
                            status: self.nat_status,
                        })
                        .await;
                }
            }

            relay::client::Event::OutboundCircuitEstablished {
                relay_peer_id,
                limit: _,
            } => {
                debug!("Outbound circuit established via relay {}", relay_peer_id);
            }

            relay::client::Event::InboundCircuitEstablished {
                src_peer_id,
                limit: _,
            } => {
                debug!("Inbound circuit established from {}", src_peer_id);
            }
        }
    }

    /// Handle DCUtR (hole punching) events
    /// Note: dcutr::Event is a struct with remote_peer_id and result fields
    async fn handle_dcutr_event(&mut self, event: dcutr::Event) {
        let remote_peer_id = event.remote_peer_id;
        match event.result {
            Ok(_connection_id) => {
                info!(
                    "Direct connection upgrade succeeded with {}",
                    remote_peer_id
                );
                // Emit event to frontend
                let _ = self
                    .event_tx
                    .send(NetworkEvent::HolePunchSucceeded {
                        peer_id: remote_peer_id.to_string(),
                    })
                    .await;
            }
            Err(error) => {
                debug!(
                    "Direct connection upgrade failed with {}: {:?}",
                    remote_peer_id, error
                );
                // Connection stays relayed - this is fine
            }
        }
    }

    /// Handle AutoNAT events
    async fn handle_autonat_event(&mut self, event: autonat::Event) {
        match event {
            autonat::Event::StatusChanged { old, new } => {
                info!("AutoNAT status changed from {:?} to {:?}", old, new);

                let new_nat_status = match new {
                    autonat::NatStatus::Public(addr) => {
                        info!("AutoNAT: We have a public address: {}", addr);
                        // Store the external address
                        if !self.external_addresses.contains(&addr) {
                            self.external_addresses.push(addr.clone());
                            let _ = self
                                .event_tx
                                .send(NetworkEvent::ExternalAddressDiscovered {
                                    address: addr.to_string(),
                                })
                                .await;
                        }
                        NatStatus::Public
                    }
                    autonat::NatStatus::Private => {
                        info!("AutoNAT: We are behind NAT, attempting relay connection...");
                        // Try to connect to relays if we haven't already
                        if !self.relay_connection_attempted {
                            self.connect_to_relays().await;
                        }
                        NatStatus::Private
                    }
                    autonat::NatStatus::Unknown => NatStatus::Unknown,
                };

                if self.nat_status != new_nat_status {
                    self.nat_status = new_nat_status;
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::NatStatusChanged {
                            status: self.nat_status,
                        })
                        .await;
                }
            }

            autonat::Event::InboundProbe(_) | autonat::Event::OutboundProbe(_) => {
                // These are just probe events, no action needed
            }
        }
    }

    /// Connect to public relay servers for NAT traversal
    async fn connect_to_relays(&mut self) {
        self.relay_connection_attempted = true;
        info!("Attempting to connect to public relay servers...");

        for relay_addr_str in PUBLIC_RELAYS {
            match relay_addr_str.parse::<Multiaddr>() {
                Ok(relay_addr) => {
                    // Extract peer ID from the multiaddress
                    let peer_id = relay_addr.iter().find_map(|proto| {
                        if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                            Some(peer_id)
                        } else {
                            None
                        }
                    });

                    if let Some(relay_peer_id) = peer_id {
                        info!("Dialing relay server: {}", relay_addr);

                        // Extract transport-only address (without /p2p/...)
                        let addr_without_peer: Multiaddr = relay_addr
                            .iter()
                            .filter(|p| !matches!(p, libp2p::multiaddr::Protocol::P2p(_)))
                            .collect();

                        // Add to Kademlia for routing
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&relay_peer_id, addr_without_peer.clone());

                        // Dial the relay
                        if let Err(e) = self.swarm.dial(relay_addr.clone()) {
                            warn!("Failed to dial relay {}: {}", relay_addr, e);
                        } else {
                            info!(
                                "Dial initiated to relay: {} (waiting for connection...)",
                                relay_peer_id
                            );
                        }

                        // Queue relay reservation for after Identify completes.
                        // listen_on must be called AFTER the connection is fully negotiated
                        // (Identify::Received), not immediately after dial — otherwise the
                        // relay client transport doesn't know about the connection yet.
                        self.pending_relay_reservations
                            .insert(relay_peer_id, relay_addr.clone());
                        info!(
                            "Relay reservation queued for {} (will request after identify)",
                            relay_peer_id
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to parse relay address '{}': {}", relay_addr_str, e);
                }
            }
        }
    }

    async fn handle_board_sync_response(&mut self, peer: PeerId, response: WireBoardSyncResponse) {
        let Some(ref board_service) = self.board_service else {
            return;
        };
        let relay_peer_id = peer.to_string();

        match response {
            WireBoardSyncResponse::BoardList { boards, .. } => {
                let board_data: Vec<(String, String, Option<String>, bool)> = boards
                    .iter()
                    .map(|b| {
                        (
                            b.board_id.clone(),
                            b.name.clone(),
                            b.description.clone(),
                            b.is_default,
                        )
                    })
                    .collect();
                match board_service.store_boards(&relay_peer_id, &board_data) {
                    Ok(()) => {
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::BoardListReceived {
                                relay_peer_id,
                                board_count: boards.len(),
                            })
                            .await;
                    }
                    Err(e) => {
                        warn!("Failed to store boards from {}: {}", peer, e);
                    }
                }
            }
            WireBoardSyncResponse::BoardPosts {
                board_id, posts, ..
            } => {
                let storable: Vec<StorableBoardPost> = posts
                    .iter()
                    .map(|p| StorableBoardPost {
                        post_id: p.post_id.clone(),
                        board_id: p.board_id.clone(),
                        author_peer_id: p.author_peer_id.clone(),
                        author_display_name: p.author_display_name.clone(),
                        content_type: p.content_type.clone(),
                        content_text: p.content_text.clone(),
                        lamport_clock: p.lamport_clock as i64,
                        created_at: p.created_at,
                        deleted_at: p.deleted_at,
                        signature: p.signature.clone(),
                    })
                    .collect();
                let post_count = storable.len();
                match board_service.store_board_posts(&relay_peer_id, &storable) {
                    Ok(()) => {
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::BoardPostsReceived {
                                relay_peer_id,
                                board_id,
                                post_count,
                            })
                            .await;
                    }
                    Err(e) => {
                        warn!("Failed to store board posts from {}: {}", peer, e);
                    }
                }
            }
            WireBoardSyncResponse::PostAccepted { post_id } => {
                info!("Board post {} accepted by relay {}", post_id, peer);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::BoardPostSubmitted {
                        relay_peer_id,
                        post_id,
                    })
                    .await;
            }
            WireBoardSyncResponse::PeerRegistered { peer_id } => {
                info!("Registered with relay {} as {}", peer, peer_id);
            }
            WireBoardSyncResponse::PostDeleted { post_id } => {
                info!("Board post {} deleted on relay {}", post_id, peer);
            }
            WireBoardSyncResponse::Error { error } => {
                warn!("Board sync error from {}: {}", peer, error);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::BoardSyncError {
                        relay_peer_id,
                        error,
                    })
                    .await;
            }
        }
    }

    async fn handle_identity_request(
        &mut self,
        _peer: PeerId,
        _request_id: request_response::InboundRequestId,
        _request: IdentityExchangeRequest,
        channel: ResponseChannel<IdentityExchangeResponse>,
    ) {
        // Get our libp2p peer ID (this is what other peers see us as)
        let local_peer_id = *self.swarm.local_peer_id();

        // Get our identity info to respond with
        match self.identity_service.get_identity_info() {
            Ok(Some(info)) => {
                // Sign the response using the libp2p peer ID
                let timestamp = chrono::Utc::now().timestamp();
                let signature = match self.identity_service.sign_raw(
                    format!("{}:{}:{}", local_peer_id, info.display_name, timestamp).as_bytes(),
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
                    // Use the libp2p peer ID, not the stored Harbor peer_id
                    peer_id: local_peer_id.to_string(),
                    public_key,
                    x25519_public,
                    display_name: info.display_name,
                    avatar_hash: info.avatar_hash,
                    bio: info.bio,
                    timestamp,
                    signature,
                };

                if let Err(e) = self
                    .swarm
                    .behaviour_mut()
                    .identity_exchange
                    .send_response(channel, response)
                {
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

        // Store in contacts database if we have the contacts service
        if let Some(ref contacts_service) = self.contacts_service {
            // Verify the response peer ID matches the peer we received from
            if response.peer_id != peer.to_string() {
                warn!(
                    "Identity response peer ID mismatch: expected {}, got {}",
                    peer, response.peer_id
                );
                return;
            }

            // TODO: Verify signature on the response
            // For now, we trust the identity since we're getting it from a direct connection

            match contacts_service.add_contact(
                &response.peer_id,
                &response.public_key,
                &response.x25519_public,
                &response.display_name,
                response.avatar_hash.as_deref(),
                response.bio.as_deref(),
            ) {
                Ok(contact_id) => {
                    info!(
                        "Added contact {} with ID {}",
                        response.display_name, contact_id
                    );

                    // Grant chat permission to the new contact
                    if let Some(ref permissions_service) = self.permissions_service {
                        match permissions_service.create_permission_grant(
                            &response.peer_id,
                            Capability::Chat,
                            None, // No expiration
                        ) {
                            Ok(_) => {
                                info!("Granted chat permission to {}", response.peer_id);
                            }
                            Err(e) => {
                                warn!("Failed to grant chat permission: {}", e);
                            }
                        }
                    }

                    // Emit event to notify frontend
                    drop(self.event_tx.send(NetworkEvent::ContactAdded {
                        peer_id: response.peer_id.clone(),
                        display_name: response.display_name.clone(),
                    }));
                }
                Err(e) => {
                    warn!("Failed to add contact: {}", e);
                }
            }
        } else {
            warn!("No contacts service configured, cannot store identity");
        }
    }

    async fn handle_messaging_request(
        &mut self,
        peer: PeerId,
        _request_id: request_response::InboundRequestId,
        request: MessagingRequest,
        channel: ResponseChannel<MessagingResponse>,
    ) {
        // Decode the message payload
        let msg_result = MessagingCodec::decode(&request.payload);

        let (success, message_id, error) = match msg_result {
            Ok(MessagingMessage::Message(direct_msg)) => {
                info!(
                    "Received direct message {} from {}",
                    direct_msg.message_id, peer
                );

                // Process the message if we have a messaging service
                if let Some(ref messaging_service) = self.messaging_service {
                    match messaging_service.process_incoming_message(
                        &direct_msg.message_id,
                        &direct_msg.conversation_id,
                        &direct_msg.sender_peer_id,
                        &direct_msg.recipient_peer_id,
                        &direct_msg.content_encrypted,
                        &direct_msg.content_type,
                        direct_msg.reply_to.as_deref(),
                        direct_msg.nonce_counter,
                        direct_msg.lamport_clock,
                        direct_msg.timestamp,
                        &direct_msg.signature,
                    ) {
                        Ok(_) => {
                            info!("Message {} processed successfully", direct_msg.message_id);
                            (true, Some(direct_msg.message_id.clone()), None)
                        }
                        Err(e) => {
                            warn!("Failed to process message {}: {}", direct_msg.message_id, e);
                            (
                                false,
                                Some(direct_msg.message_id.clone()),
                                Some(e.to_string()),
                            )
                        }
                    }
                } else {
                    warn!("No messaging service configured, cannot process message");
                    (
                        false,
                        Some(direct_msg.message_id),
                        Some("Messaging service not available".to_string()),
                    )
                }
            }
            Ok(MessagingMessage::Ack(ack)) => {
                info!("Received message ack for {} from {}", ack.message_id, peer);
                // TODO: Process acknowledgment (update message status)
                (true, Some(ack.message_id), None)
            }
            Err(e) => {
                warn!("Failed to decode messaging payload: {}", e);
                (false, None, Some(format!("Failed to decode: {}", e)))
            }
        };

        // Send response
        let response = MessagingResponse {
            success,
            message_id,
            error,
        };

        if let Err(e) = self
            .swarm
            .behaviour_mut()
            .messaging
            .send_response(channel, response)
        {
            warn!("Failed to send messaging response: {:?}", e);
        }

        // Emit event for the application layer (for UI updates)
        let _ = self
            .event_tx
            .send(NetworkEvent::MessageReceived {
                peer_id: peer.to_string(),
                protocol: "messaging".to_string(),
                payload: request.payload,
            })
            .await;
    }

    async fn handle_command(&mut self, command: NetworkCommand) -> NetworkResponse {
        match command {
            NetworkCommand::Dial { peer_id, addresses } => {
                for addr in addresses {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr.clone());
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

            NetworkCommand::SendMessage {
                peer_id,
                protocol,
                payload,
            } => {
                let request = MessagingRequest {
                    message_type: protocol,
                    payload,
                };
                self.swarm
                    .behaviour_mut()
                    .messaging
                    .send_request(&peer_id, request);
                NetworkResponse::Ok
            }

            NetworkCommand::RequestIdentity { peer_id } => {
                // Create identity request
                match self.create_identity_request() {
                    Ok(request) => {
                        self.swarm
                            .behaviour_mut()
                            .identity_exchange
                            .send_request(&peer_id, request);
                        NetworkResponse::Ok
                    }
                    Err(e) => {
                        NetworkResponse::Error(format!("Failed to create identity request: {}", e))
                    }
                }
            }

            NetworkCommand::GetStats => {
                let mut stats = self.stats.clone();
                stats.uptime_seconds = self.start_time.elapsed().as_secs();
                stats.nat_status = self.nat_status;
                stats.relay_addresses =
                    self.relay_addresses.iter().map(|a| a.to_string()).collect();
                stats.external_addresses = self
                    .external_addresses
                    .iter()
                    .map(|a| a.to_string())
                    .collect();
                NetworkResponse::Stats(stats)
            }

            NetworkCommand::GetConnectedPeers => {
                let peers: Vec<PeerInfo> = self.connected_peers.values().cloned().collect();
                NetworkResponse::Peers(peers)
            }

            NetworkCommand::GetListeningAddresses => {
                let local_peer_id = self.swarm.local_peer_id();
                let mut addresses: Vec<String> = Vec::new();

                // Add relay addresses first (most important for remote connections)
                for addr in &self.relay_addresses {
                    addresses.push(addr.to_string());
                }

                // Add external addresses discovered via AutoNAT
                for addr in &self.external_addresses {
                    addresses.push(format!("{}/p2p/{}", addr, local_peer_id));
                }

                // Add local listening addresses
                for addr in &self.listening_addresses {
                    addresses.push(format!("{}/p2p/{}", addr, local_peer_id));
                }

                NetworkResponse::Addresses(addresses)
            }

            NetworkCommand::AddBootstrapNode { address } => {
                // Parse the multiaddress to extract peer ID if present
                if let Some(peer_id) = address.iter().find_map(|proto| {
                    if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                        Some(peer_id)
                    } else {
                        None
                    }
                }) {
                    // Add to Kademlia routing table
                    let addr_without_peer: Multiaddr = address
                        .iter()
                        .filter(|p| !matches!(p, libp2p::multiaddr::Protocol::P2p(_)))
                        .collect();
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr_without_peer);
                    info!("Added bootstrap node: {} at {}", peer_id, address);

                    // Try to dial the bootstrap node
                    match self.swarm.dial(address.clone()) {
                        Ok(_) => {
                            info!("Dialing bootstrap node: {}", address);
                            NetworkResponse::Ok
                        }
                        Err(e) => {
                            NetworkResponse::Error(format!("Failed to dial bootstrap node: {}", e))
                        }
                    }
                } else {
                    NetworkResponse::Error(
                        "Multiaddress must contain peer ID (/p2p/...)".to_string(),
                    )
                }
            }

            NetworkCommand::Bootstrap => {
                if let Err(e) = self.swarm.behaviour_mut().kademlia.bootstrap() {
                    NetworkResponse::Error(format!("Bootstrap failed: {:?}", e))
                } else {
                    NetworkResponse::Ok
                }
            }

            NetworkCommand::AddRelayServer { address } => {
                // Parse the multiaddress to extract peer ID if present
                if let Some(relay_peer_id) = address.iter().find_map(|proto| {
                    if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                        Some(peer_id)
                    } else {
                        None
                    }
                }) {
                    // Extract transport-only address (without /p2p/...)
                    let addr_without_peer: Multiaddr = address
                        .iter()
                        .filter(|p| !matches!(p, libp2p::multiaddr::Protocol::P2p(_)))
                        .collect();

                    if addr_without_peer.is_empty() {
                        // No transport components (e.g. address is just /p2p/<peer_id>), so skip Kademlia
                        info!(
                            "Relay server {} has no non-P2p components in address {}; skipping Kademlia add_address",
                            relay_peer_id, address
                        );
                    } else {
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&relay_peer_id, addr_without_peer.clone());
                        info!("Added relay server: {} at {}", relay_peer_id, address);
                    }

                    // Try to dial the relay server using the full multiaddr (including /p2p)
                    match self.swarm.dial(address.clone()) {
                        Ok(_) => {
                            info!("Dialing relay server: {}", address);
                        }
                        Err(e) => {
                            return NetworkResponse::Error(format!(
                                "Failed to dial relay server: {}",
                                e
                            ));
                        }
                    }

                    // Queue relay reservation for after Identify completes.
                    // listen_on must be called AFTER the connection is fully negotiated
                    // (Identify::Received), not immediately after dial.
                    self.pending_relay_reservations
                        .insert(relay_peer_id, address.clone());
                    info!(
                        "Relay reservation queued for {} (will request after identify)",
                        relay_peer_id
                    );

                    NetworkResponse::Ok
                } else {
                    NetworkResponse::Error(
                        "Relay address must contain peer ID (/p2p/...)".to_string(),
                    )
                }
            }

            NetworkCommand::ConnectToPublicRelays => {
                // Reset the flag to allow reconnection and actually connect
                self.relay_connection_attempted = false;
                info!("Manually triggering connection to public relay servers...");
                self.connect_to_relays().await;
                NetworkResponse::Ok
            }

            NetworkCommand::SyncFeed { limit } => {
                // Clamp the limit to avoid pathological or abusive requests.
                const MAX_MANIFEST_LIMIT: u32 = 1000;
                let clamped_limit = limit.min(MAX_MANIFEST_LIMIT);

                let Some(ref content_sync_service) = self.content_sync_service else {
                    return NetworkResponse::Error("Content sync service unavailable".to_string());
                };

                // Avoid borrow issues: collect peer ids first.
                let connected_peer_ids: Vec<PeerId> =
                    self.connected_peers.keys().cloned().collect();

                // Request manifest from each connected peer (excluding ourselves, if present).
                for peer_id in connected_peer_ids {
                    let peer_id_string = peer_id.to_string();
                    let cursor = match content_sync_service.get_sync_cursor(&peer_id_string) {
                        Ok(cursor_value) => cursor_value,
                        Err(error) => {
                            warn!("Failed to load sync cursor for {}: {}", peer_id, error);
                            HashMap::new()
                        }
                    };

                    let manifest_request =
                        match content_sync_service.create_manifest_request(cursor, clamped_limit) {
                            Ok(request_value) => request_value,
                            Err(error) => {
                                warn!(
                                    "Failed to create manifest request for {}: {}",
                                    peer_id, error
                                );
                                continue;
                            }
                        };

                    let wire_message = ContentSyncRequest::Manifest {
                        requester_peer_id: manifest_request.requester_peer_id,
                        cursor: manifest_request.cursor,
                        limit: manifest_request.limit,
                        timestamp: manifest_request.timestamp,
                        signature: manifest_request.signature,
                    };

                    self.swarm
                        .behaviour_mut()
                        .content_sync
                        .send_request(&peer_id, wire_message);
                }

                NetworkResponse::Ok
            }

            NetworkCommand::RequestContentManifest {
                peer_id,
                cursor,
                limit,
            } => {
                const MAX_MANIFEST_LIMIT: u32 = 1000;
                let clamped_limit = limit.min(MAX_MANIFEST_LIMIT);

                let Some(ref content_sync_service) = self.content_sync_service else {
                    return NetworkResponse::Error("Content sync service unavailable".to_string());
                };

                let manifest_request =
                    match content_sync_service.create_manifest_request(cursor, clamped_limit) {
                        Ok(request_value) => request_value,
                        Err(error) => {
                            return NetworkResponse::Error(format!(
                                "Failed to create manifest request: {}",
                                error
                            ));
                        }
                    };

                let wire_message = ContentSyncRequest::Manifest {
                    requester_peer_id: manifest_request.requester_peer_id,
                    cursor: manifest_request.cursor,
                    limit: manifest_request.limit,
                    timestamp: manifest_request.timestamp,
                    signature: manifest_request.signature,
                };

                self.swarm
                    .behaviour_mut()
                    .content_sync
                    .send_request(&peer_id, wire_message);

                NetworkResponse::Ok
            }

            NetworkCommand::RequestContentFetch {
                peer_id,
                post_id,
                include_media,
            } => {
                let Some(ref content_sync_service) = self.content_sync_service else {
                    return NetworkResponse::Error("Content sync service unavailable".to_string());
                };

                let fetch_request = match content_sync_service
                    .create_fetch_request(post_id.clone(), include_media)
                {
                    Ok(request_value) => request_value,
                    Err(error) => {
                        return NetworkResponse::Error(format!(
                            "Failed to create content fetch request: {}",
                            error
                        ));
                    }
                };

                let wire_message = ContentSyncRequest::FetchPost {
                    requester_peer_id: fetch_request.requester_peer_id,
                    post_id: fetch_request.post_id,
                    include_media: fetch_request.include_media,
                    timestamp: fetch_request.timestamp,
                    signature: fetch_request.signature,
                };

                self.swarm
                    .behaviour_mut()
                    .content_sync
                    .send_request(&peer_id, wire_message);

                NetworkResponse::Ok
            }

            NetworkCommand::JoinCommunity {
                relay_peer_id,
                relay_address,
            } => {
                let Some(ref board_service) = self.board_service else {
                    return NetworkResponse::Error("Board service unavailable".to_string());
                };

                // Store community locally
                if let Err(e) =
                    board_service.join_community(&relay_peer_id.to_string(), &relay_address, None)
                {
                    return NetworkResponse::Error(format!("Failed to join community: {}", e));
                }

                // Register peer with relay
                match board_service.create_peer_registration() {
                    Ok(reg) => {
                        let request = WireBoardSyncRequest::RegisterPeer {
                            peer_id: reg.peer_id,
                            public_key: reg.public_key,
                            display_name: reg.display_name,
                            timestamp: reg.timestamp,
                            signature: reg.signature,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);

                        // Also list boards
                        if let Ok(list_req) = board_service.create_list_boards_request() {
                            let request = WireBoardSyncRequest::ListBoards {
                                requester_peer_id: list_req.requester_peer_id,
                                timestamp: list_req.timestamp,
                                signature: list_req.signature,
                            };
                            self.swarm
                                .behaviour_mut()
                                .board_sync
                                .send_request(&relay_peer_id, request);
                        }
                        NetworkResponse::Ok
                    }
                    Err(e) => {
                        NetworkResponse::Error(format!("Failed to create registration: {}", e))
                    }
                }
            }

            NetworkCommand::ListBoards { relay_peer_id } => {
                let Some(ref board_service) = self.board_service else {
                    return NetworkResponse::Error("Board service unavailable".to_string());
                };

                match board_service.create_list_boards_request() {
                    Ok(req) => {
                        let request = WireBoardSyncRequest::ListBoards {
                            requester_peer_id: req.requester_peer_id,
                            timestamp: req.timestamp,
                            signature: req.signature,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);
                        NetworkResponse::Ok
                    }
                    Err(e) => NetworkResponse::Error(format!(
                        "Failed to create list boards request: {}",
                        e
                    )),
                }
            }

            NetworkCommand::GetBoardPosts {
                relay_peer_id,
                board_id,
                after_timestamp,
                limit,
            } => {
                let Some(ref board_service) = self.board_service else {
                    return NetworkResponse::Error("Board service unavailable".to_string());
                };

                match board_service.create_get_board_posts_request(
                    &board_id,
                    after_timestamp,
                    limit,
                ) {
                    Ok(req) => {
                        let request = WireBoardSyncRequest::GetBoardPosts {
                            requester_peer_id: req.requester_peer_id,
                            board_id: req.board_id,
                            after_timestamp: req.after_timestamp,
                            limit: req.limit,
                            timestamp: req.timestamp,
                            signature: req.signature,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);
                        NetworkResponse::Ok
                    }
                    Err(e) => NetworkResponse::Error(format!(
                        "Failed to create get board posts request: {}",
                        e
                    )),
                }
            }

            NetworkCommand::SubmitBoardPost {
                relay_peer_id,
                board_id,
                content_text,
            } => {
                let Some(ref board_service) = self.board_service else {
                    return NetworkResponse::Error("Board service unavailable".to_string());
                };

                match board_service.create_board_post(&board_id, &content_text) {
                    Ok(post) => {
                        let request = WireBoardSyncRequest::SubmitPost {
                            post_id: post.post_id,
                            board_id: post.board_id,
                            author_peer_id: post.author_peer_id,
                            content_type: post.content_type,
                            content_text: post.content_text,
                            lamport_clock: post.lamport_clock,
                            created_at: post.created_at,
                            signature: post.signature,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);
                        NetworkResponse::Ok
                    }
                    Err(e) => NetworkResponse::Error(format!("Failed to create board post: {}", e)),
                }
            }

            NetworkCommand::DeleteBoardPost {
                relay_peer_id,
                post_id,
            } => {
                let Some(ref board_service) = self.board_service else {
                    return NetworkResponse::Error("Board service unavailable".to_string());
                };

                match board_service.create_delete_post_request(&post_id) {
                    Ok(req) => {
                        let request = WireBoardSyncRequest::DeletePost {
                            post_id: req.post_id,
                            author_peer_id: req.author_peer_id,
                            timestamp: req.timestamp,
                            signature: req.signature,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);
                        NetworkResponse::Ok
                    }
                    Err(e) => {
                        NetworkResponse::Error(format!("Failed to create delete request: {}", e))
                    }
                }
            }

            NetworkCommand::SyncBoard {
                relay_peer_id,
                board_id,
            } => {
                let Some(ref board_service) = self.board_service else {
                    return NetworkResponse::Error("Board service unavailable".to_string());
                };

                let after_timestamp = board_service
                    .get_sync_cursor(&relay_peer_id.to_string(), &board_id)
                    .unwrap_or(None);

                match board_service.create_get_board_posts_request(&board_id, after_timestamp, 50) {
                    Ok(req) => {
                        let request = WireBoardSyncRequest::GetBoardPosts {
                            requester_peer_id: req.requester_peer_id,
                            board_id: req.board_id,
                            after_timestamp: req.after_timestamp,
                            limit: req.limit,
                            timestamp: req.timestamp,
                            signature: req.signature,
                        };
                        self.swarm
                            .behaviour_mut()
                            .board_sync
                            .send_request(&relay_peer_id, request);
                        NetworkResponse::Ok
                    }
                    Err(e) => {
                        NetworkResponse::Error(format!("Failed to create sync request: {}", e))
                    }
                }
            }

            NetworkCommand::Shutdown => NetworkResponse::Ok,
        }
    }

    /// Attempt to connect to public relay servers
    /// This is called when we detect we're behind NAT or when manually requested
    pub async fn try_connect_to_relays(&mut self) {
        if self.relay_connection_attempted {
            info!("Already attempted to connect to relays, skipping");
            return;
        }
        self.connect_to_relays().await;
    }
}
