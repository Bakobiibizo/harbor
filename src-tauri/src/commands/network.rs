use crate::error::AppError;
use crate::p2p::{NetworkConfig, NetworkHandle, NetworkService, NetworkStats, PeerInfo};
use crate::services::{
    ContactsService, ContentSyncService, IdentityService, MessagingService, PermissionsService,
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
            .ok_or_else(|| AppError::Network("Network not initialized".to_string()))
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

/// Start the P2P network (called after identity is unlocked)
#[tauri::command]
pub async fn start_network(
    app: AppHandle,
    network: State<'_, NetworkState>,
    identity_service: State<'_, Arc<IdentityService>>,
    messaging_service: State<'_, Arc<MessagingService>>,
    contacts_service: State<'_, Arc<ContactsService>>,
    permissions_service: State<'_, Arc<PermissionsService>>,
    content_sync_service: State<'_, Arc<ContentSyncService>>,
) -> Result<(), AppError> {
    // Check if identity is unlocked
    if !identity_service.is_unlocked() {
        return Err(AppError::PermissionDenied(
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
    let identity_arc: Arc<IdentityService> = (*identity_service).clone();
    let (mut service, handle, mut event_rx) = NetworkService::new(config, identity_arc, keypair)?;

    // Inject services for message processing, contact storage, permissions, and content sync
    service.set_messaging_service((*messaging_service).clone());
    service.set_contacts_service((*contacts_service).clone());
    service.set_permissions_service((*permissions_service).clone());
    service.set_content_sync_service((*content_sync_service).clone());

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
