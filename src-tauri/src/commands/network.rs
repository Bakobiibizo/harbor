use crate::error::AppError;
use crate::p2p::{NetworkConfig, NetworkHandle, NetworkService, NetworkStats, PeerInfo};
use crate::services::{
    BoardService, ContactsService, ContentSyncService, IdentityService, MessagingService,
    PermissionsService, PostsService,
};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;
use tracing::info;

/// Wrapper for NetworkHandle to make it Tauri state compatible
pub struct NetworkState {
    pub handle: RwLock<Option<NetworkHandle>>,
}

impl NetworkState {
    pub fn new() -> Self {
        Self {
            handle: RwLock::new(None),
        }
    }

    pub async fn set_handle(&self, handle: NetworkHandle) {
        let mut guard = self.handle.write().await;
        *guard = Some(handle);
    }

    pub async fn get_handle(&self) -> Result<NetworkHandle, AppError> {
        let guard: tokio::sync::RwLockReadGuard<'_, Option<NetworkHandle>> =
            self.handle.read().await;
        guard
            .clone()
            .ok_or_else(|| AppError::NetworkNotInitialized("Network not initialized".to_string()))
    }
}

impl Default for NetworkState {
    fn default() -> Self {
        Self::new()
    }
}

/// Get list of connected peers
#[tauri::command]
pub async fn get_connected_peers(
    network: State<'_, NetworkState>,
) -> Result<Vec<PeerInfo>, AppError> {
    let handle: NetworkHandle = network.get_handle().await?;
    handle.get_connected_peers().await
}

/// Get network statistics
#[tauri::command]
pub async fn get_network_stats(network: State<'_, NetworkState>) -> Result<NetworkStats, AppError> {
    let handle: NetworkHandle = network.get_handle().await?;
    handle.get_stats().await
}

/// Check if the network is running
#[tauri::command]
pub async fn is_network_running(network: State<'_, NetworkState>) -> Result<bool, AppError> {
    let guard: tokio::sync::RwLockReadGuard<'_, Option<NetworkHandle>> =
        network.handle.read().await;
    Ok(guard.is_some())
}

/// Bootstrap the DHT (connect to bootstrap nodes)
#[tauri::command]
pub async fn bootstrap_network(network: State<'_, NetworkState>) -> Result<(), AppError> {
    let handle: NetworkHandle = network.get_handle().await?;
    handle.bootstrap().await
}

/// Services needed to start the P2P network
pub struct StartNetworkServices {
    pub identity_service: Arc<IdentityService>,
    pub messaging_service: Arc<MessagingService>,
    pub contacts_service: Arc<ContactsService>,
    pub permissions_service: Arc<PermissionsService>,
    pub posts_service: Arc<PostsService>,
    pub content_sync_service: Arc<ContentSyncService>,
    pub board_service: Arc<BoardService>,
}

/// Start the P2P network (called after identity is unlocked)
///
/// Note: Tauri State<> parameters are auto-injected by the framework and cannot be
/// grouped into a struct. The actual logic is delegated to start_network_with_services
/// which uses a StartNetworkServices parameter struct.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn start_network(
    app: AppHandle,
    network: State<'_, NetworkState>,
    identity_service: State<'_, Arc<IdentityService>>,
    messaging_service: State<'_, Arc<MessagingService>>,
    contacts_service: State<'_, Arc<ContactsService>>,
    permissions_service: State<'_, Arc<PermissionsService>>,
    posts_service: State<'_, Arc<PostsService>>,
    content_sync_service: State<'_, Arc<ContentSyncService>>,
    board_service: State<'_, Arc<BoardService>>,
) -> Result<(), AppError> {
    let services = StartNetworkServices {
        identity_service: (*identity_service).clone(),
        messaging_service: (*messaging_service).clone(),
        contacts_service: (*contacts_service).clone(),
        permissions_service: (*permissions_service).clone(),
        posts_service: (*posts_service).clone(),
        content_sync_service: (*content_sync_service).clone(),
        board_service: (*board_service).clone(),
    };
    start_network_with_services(app, network, services).await
}

/// Internal implementation for starting the P2P network
async fn start_network_with_services(
    app: AppHandle,
    network: State<'_, NetworkState>,
    services: StartNetworkServices,
) -> Result<(), AppError> {
    let identity_service = &services.identity_service;
    // Check if identity is unlocked
    if !identity_service.is_unlocked() {
        return Err(AppError::IdentityLocked(
            "Identity must be unlocked to start network".to_string(),
        ));
    }

    // Check if network is already running
    {
        let guard: tokio::sync::RwLockReadGuard<'_, Option<NetworkHandle>> =
            network.handle.read().await;
        if guard.is_some() {
            return Ok(()); // Already running
        }
    }

    // Get the unlocked keys to create a libp2p keypair
    let unlocked_keys = identity_service.get_unlocked_keys()?;
    let ed25519_bytes = unlocked_keys.ed25519_signing.to_bytes();

    // Convert to libp2p keypair
    let keypair = crate::p2p::swarm::ed25519_to_libp2p_keypair(&ed25519_bytes)?;
    let network_peer_id = libp2p::PeerId::from(keypair.public());

    // Compare with stored identity peer ID to verify they match
    if let Ok(Some(identity_info)) = identity_service.get_identity_info() {
        info!(
            "PEER ID CHECK - Stored: {} (len={}) vs Network: {} (len={})",
            identity_info.peer_id,
            identity_info.peer_id.len(),
            network_peer_id,
            network_peer_id.to_string().len()
        );
        if identity_info.peer_id != network_peer_id.to_string() {
            tracing::error!(
                "PEER ID MISMATCH! Stored peer ID does not match network peer ID. This will cause messaging to fail."
            );
        }
    }

    // Create network config
    let config = NetworkConfig::default();

    // Create network service - clone the Arc to pass to the service
    let identity_arc: Arc<IdentityService> = services.identity_service.clone();
    let (mut service, handle, mut event_rx) = NetworkService::new(config, identity_arc, keypair)?;

    // Inject services for message processing, contact storage, permissions, content sync, and boards
    service.set_messaging_service(services.messaging_service.clone());
    service.set_contacts_service(services.contacts_service.clone());
    service.set_permissions_service(services.permissions_service.clone());
    service.set_posts_service(services.posts_service.clone());
    service.set_content_sync_service(services.content_sync_service.clone());
    service.set_board_service(services.board_service.clone());

    // Store the handle
    network.set_handle(handle).await;

    // Spawn the network service in a background task
    tokio::spawn(async move {
        info!("Network service starting in background task");
        service.run().await;
        info!("Network service stopped");
    });

    // Spawn a task to process network events and forward to frontend
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            info!("Network event: {:?}", event);
            // Emit event to frontend
            if let Err(e) = app_clone.emit("harbor:network", &event) {
                tracing::warn!("Failed to emit network event: {}", e);
            }
        }
    });

    info!("Network started successfully");
    Ok(())
}

/// Stop the P2P network
#[tauri::command]
pub async fn stop_network(network: State<'_, NetworkState>) -> Result<(), AppError> {
    let maybe_handle: Option<NetworkHandle> = {
        let mut guard = network.handle.write().await;
        guard.take()
    };

    if let Some(handle) = maybe_handle {
        handle.shutdown().await?;
        info!("Network stopped");
    }

    Ok(())
}

/// Get listening addresses (for sharing with remote peers)
#[tauri::command]
pub async fn get_listening_addresses(
    network: State<'_, NetworkState>,
) -> Result<Vec<String>, AppError> {
    let handle: NetworkHandle = network.get_handle().await?;
    handle.get_listening_addresses().await
}

/// Connect to a peer by multiaddress
/// Format: /ip4/1.2.3.4/tcp/9000/p2p/12D3KooW...
#[tauri::command]
pub async fn connect_to_peer(
    network: State<'_, NetworkState>,
    multiaddr: String,
) -> Result<(), AppError> {
    let handle: NetworkHandle = network.get_handle().await?;

    // Parse the multiaddress
    let addr: libp2p::Multiaddr = multiaddr
        .parse()
        .map_err(|e| AppError::Validation(format!("Invalid multiaddress: {}", e)))?;

    // Use add_bootstrap_node which handles both adding to Kademlia and dialing
    handle.add_bootstrap_node(addr).await
}

/// Add a bootstrap node address
#[tauri::command]
pub async fn add_bootstrap_node(
    network: State<'_, NetworkState>,
    multiaddr: String,
) -> Result<(), AppError> {
    let handle: NetworkHandle = network.get_handle().await?;

    let addr: libp2p::Multiaddr = multiaddr
        .parse()
        .map_err(|e| AppError::Validation(format!("Invalid multiaddress: {}", e)))?;

    handle.add_bootstrap_node(addr).await
}

/// Get shareable addresses for remote peers to connect to us
/// Returns external addresses discovered via AutoNAT or relay addresses if behind NAT
#[tauri::command]
pub async fn get_shareable_addresses(
    network: State<'_, NetworkState>,
    identity_service: State<'_, Arc<IdentityService>>,
) -> Result<Vec<String>, AppError> {
    let handle: NetworkHandle = network.get_handle().await?;
    let stats = handle.get_stats().await?;

    // Get our peer ID
    let peer_id = if let Ok(Some(identity)) = identity_service.get_identity_info() {
        identity.peer_id
    } else {
        return Err(AppError::IdentityNotFound("Identity not found".to_string()));
    };

    let mut addresses = Vec::new();

    // First, prefer external addresses (direct connectivity)
    for addr in &stats.external_addresses {
        if !addr.contains("127.0.0.1") && !addr.contains("::1") {
            // Ensure address includes peer ID
            if addr.contains("/p2p/") {
                addresses.push(addr.clone());
            } else {
                addresses.push(format!("{}/p2p/{}", addr, peer_id));
            }
        }
    }

    // If no external addresses, use relay addresses
    if addresses.is_empty() {
        for addr in &stats.relay_addresses {
            addresses.push(addr.clone());
        }
    }

    Ok(addresses)
}

/// Add a custom relay server address
#[tauri::command]
pub async fn add_relay_server(
    network: State<'_, NetworkState>,
    multiaddr: String,
) -> Result<(), AppError> {
    let handle: NetworkHandle = network.get_handle().await?;

    let addr: libp2p::Multiaddr = multiaddr
        .parse()
        .map_err(|e| AppError::Validation(format!("Invalid multiaddress: {}", e)))?;

    handle.add_relay_server(addr).await
}

/// Connect to public relay servers for NAT traversal
#[tauri::command]
pub async fn connect_to_public_relays(network: State<'_, NetworkState>) -> Result<(), AppError> {
    let handle: NetworkHandle = network.get_handle().await?;
    handle.connect_to_public_relays().await
}

/// Get detailed NAT status from network stats
#[tauri::command]
pub async fn get_nat_status(
    network: State<'_, NetworkState>,
) -> Result<crate::p2p::NatStatus, AppError> {
    let handle: NetworkHandle = network.get_handle().await?;
    let stats = handle.get_stats().await?;
    Ok(stats.nat_status)
}

/// Trigger feed sync from connected peers
#[tauri::command]
pub async fn sync_feed(
    network: State<'_, NetworkState>,
    limit: Option<u32>,
) -> Result<(), AppError> {
    let handle: NetworkHandle = network.get_handle().await?;
    handle.sync_feed(limit.unwrap_or(50)).await
}

/// Contact bundle for sharing - contains everything needed to add someone as a contact
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactBundle {
    /// Multiaddress for connection
    pub multiaddr: String,
    /// Display name
    pub display_name: String,
    /// Ed25519 public key (base64)
    pub public_key: String,
    /// X25519 public key for encryption (base64)
    pub x25519_public: String,
    /// Optional bio
    pub bio: Option<String>,
    /// Optional avatar hash
    pub avatar_hash: Option<String>,
}

/// Generate a shareable contact string that includes all info needed to add as contact
/// Format: harbor://<base64_encoded_json>
#[tauri::command]
pub async fn get_shareable_contact_string(
    network: State<'_, NetworkState>,
    identity_service: State<'_, Arc<IdentityService>>,
) -> Result<String, AppError> {
    use base64::Engine;

    let handle: NetworkHandle = network.get_handle().await?;
    let stats = handle.get_stats().await?;

    // Get our identity with keys
    let identity = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::IdentityNotFound("Identity not found".to_string()))?;

    let keys = identity_service
        .get_identity_info()?
        .ok_or_else(|| AppError::IdentityNotFound("Identity keys not found".to_string()))?;

    // Get the best address to share
    let multiaddr = if !stats.relay_addresses.is_empty() {
        // Prefer relay addresses as they work through NAT
        stats.relay_addresses[0].clone()
    } else if !stats.external_addresses.is_empty() {
        // Use external address if available
        let addr = &stats.external_addresses[0];
        if addr.contains("/p2p/") {
            addr.clone()
        } else {
            format!("{}/p2p/{}", addr, identity.peer_id)
        }
    } else {
        return Err(AppError::Network(
            "No shareable address available. Please connect to a relay first.".to_string(),
        ));
    };

    let bundle = ContactBundle {
        multiaddr,
        display_name: identity.display_name,
        public_key: keys.public_key,
        x25519_public: keys.x25519_public,
        bio: identity.bio,
        avatar_hash: identity.avatar_hash,
    };

    let json = serde_json::to_string(&bundle)
        .map_err(|e| AppError::Serialization(format!("Failed to serialize contact: {}", e)))?;

    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes());

    Ok(format!("harbor://{}", encoded))
}

/// Add a contact from a shareable contact string and connect to them
/// This is the simplified flow - no handshake needed
#[tauri::command]
pub async fn add_contact_from_string(
    network: State<'_, NetworkState>,
    contacts_service: State<'_, Arc<ContactsService>>,
    permissions_service: State<'_, Arc<PermissionsService>>,
    contact_string: String,
) -> Result<String, AppError> {
    use crate::db::Capability;
    use base64::Engine;

    // Parse the contact string
    let encoded = contact_string
        .strip_prefix("harbor://")
        .ok_or_else(|| AppError::Validation("Invalid contact string format".to_string()))?;

    let json_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|e| AppError::Validation(format!("Invalid contact encoding: {}", e)))?;

    let bundle: ContactBundle = serde_json::from_slice(&json_bytes)
        .map_err(|e| AppError::Validation(format!("Invalid contact data: {}", e)))?;

    // Decode the keys — contact strings have double-base64-encoded keys:
    // base64(base64(32 raw bytes)). Decode both layers, fall back to single decode
    // for correctly-encoded keys.
    let pk_once = base64::engine::general_purpose::STANDARD
        .decode(&bundle.public_key)
        .map_err(|e| AppError::Validation(format!("Invalid public key: {}", e)))?;
    let public_key = base64::engine::general_purpose::STANDARD
        .decode(&pk_once)
        .unwrap_or(pk_once);

    let x25519_once = base64::engine::general_purpose::STANDARD
        .decode(&bundle.x25519_public)
        .map_err(|e| AppError::Validation(format!("Invalid x25519 key: {}", e)))?;
    let x25519_public = base64::engine::general_purpose::STANDARD
        .decode(&x25519_once)
        .unwrap_or(x25519_once);

    // Extract peer ID from multiaddr
    let peer_id = bundle
        .multiaddr
        .split("/p2p/")
        .last()
        .ok_or_else(|| AppError::Validation("No peer ID in multiaddr".to_string()))?
        .to_string();

    // Add as contact
    contacts_service.add_contact(
        &peer_id,
        &public_key,
        &x25519_public,
        &bundle.display_name,
        bundle.avatar_hash.as_deref(),
        bundle.bio.as_deref(),
    )?;

    // Grant them permissions (WallRead and Chat by default)
    let _ = permissions_service.create_permission_grant(&peer_id, Capability::WallRead, None);
    let _ = permissions_service.create_permission_grant(&peer_id, Capability::Chat, None);

    // Connect to them
    let handle: NetworkHandle = network.get_handle().await?;
    let addr: libp2p::Multiaddr = bundle
        .multiaddr
        .parse()
        .map_err(|e| AppError::Validation(format!("Invalid multiaddress: {}", e)))?;

    // Don't fail if connection fails - they might be offline
    let _ = handle.add_bootstrap_node(addr).await;

    info!(
        "Added contact {} ({}) from shareable string",
        bundle.display_name, peer_id
    );

    Ok(peer_id)
}
