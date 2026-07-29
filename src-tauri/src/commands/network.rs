use crate::commands::relay_names::NameClaimDto;
use crate::db::repositories::{bootstrap_repo::BootstrapNodesRepo, RelayNamesRepository};
use crate::db::Database;
use crate::error::AppError;
use crate::p2p::{
    NetworkConfig, NetworkEvent, NetworkHandle, NetworkService, NetworkStats, PeerInfo,
};
use crate::services::{
    BoardService, CallingService, ContactsService, ContentSyncService, IdentityService,
    MediaStorageService, MentionsService, MessagingService, PermissionsService, PostsService,
    WallSocialService,
};
use std::collections::HashSet;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{oneshot, watch, Mutex};
use tokio::task::JoinHandle;
use tracing::info;

fn log_network_event(event: &NetworkEvent) {
    if let NetworkEvent::CallSignalingReceived { message, .. } = event {
        let summary = message.log_summary("inbound", "verified");
        info!(
            signaling_kind = summary.kind,
            correlation_id = %summary.correlation_id,
            direction = summary.direction,
            result = summary.result,
            "Network call signaling event"
        );
    } else {
        info!("Network event: {:?}", event);
    }
}

struct NetworkRuntime {
    id: u64,
    handle: NetworkHandle,
    cancel: watch::Sender<bool>,
    workers: Vec<JoinHandle<()>>,
    runtime_cache_cleanup: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// Owns the complete runtime for the active profile. The mutex serializes start,
/// stop, and profile-lock transitions so two swarms cannot be published at once.
pub struct NetworkState {
    runtime: Arc<Mutex<Option<NetworkRuntime>>>,
    next_runtime_id: AtomicU64,
}

impl NetworkState {
    pub fn new() -> Self {
        Self {
            runtime: Arc::new(Mutex::new(None)),
            next_runtime_id: AtomicU64::new(1),
        }
    }

    pub async fn get_handle(&self) -> Result<NetworkHandle, AppError> {
        let guard = self.runtime.lock().await;
        guard
            .as_ref()
            .map(|runtime| runtime.handle.clone())
            .ok_or_else(|| AppError::NetworkNotInitialized("Network not initialized".to_string()))
    }

    pub async fn is_running(&self) -> bool {
        self.runtime.lock().await.is_some()
    }

    /// Attach profile work to the runtime so lock, switch, and stop await it.
    pub async fn spawn_scoped<F>(&self, future: F) -> Result<(), AppError>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut guard = self.runtime.lock().await;
        let runtime = guard.as_mut().ok_or_else(|| {
            AppError::NetworkNotInitialized("Network not initialized".to_string())
        })?;
        runtime.workers.push(tokio::spawn(future));
        Ok(())
    }

    /// Stop all profile workers before clearing private key material, while the
    /// lifecycle lock prevents a concurrent start from reopening the profile.
    pub async fn stop_and_lock_identity(
        &self,
        identity_service: &IdentityService,
    ) -> Result<(), AppError> {
        let mut guard = self.runtime.lock().await;
        stop_runtime(&mut guard).await?;
        identity_service.lock();
        Ok(())
    }
}

fn build_network_config(
    db: &Database,
    enable_mdns: Option<bool>,
    requested_bootstraps: Option<Vec<String>>,
) -> Result<NetworkConfig, AppError> {
    let mut addresses = BootstrapNodesRepo::get_enabled_addresses(db)?;
    addresses.extend(requested_bootstraps.unwrap_or_default());

    let mut seen = HashSet::new();
    let mut bootstrap_nodes = Vec::new();
    for address in addresses {
        if !seen.insert(address.clone()) {
            continue;
        }
        let multiaddr: libp2p::Multiaddr = address.parse().map_err(|error| {
            AppError::Validation(format!(
                "Invalid persisted bootstrap address {address}: {error}"
            ))
        })?;
        if !multiaddr
            .iter()
            .any(|protocol| matches!(protocol, libp2p::multiaddr::Protocol::P2p(_)))
        {
            return Err(AppError::Validation(format!(
                "Persisted bootstrap address must contain a peer ID: {address}"
            )));
        }
        bootstrap_nodes.push(multiaddr);
    }

    Ok(NetworkConfig {
        enable_mdns: enable_mdns.unwrap_or(true),
        bootstrap_nodes,
        ..NetworkConfig::default()
    })
}

impl Default for NetworkState {
    fn default() -> Self {
        Self::new()
    }
}

/// Revalidate cached identity keys against freshly loaded persisted metadata
/// before any network service or externally usable handle can be created.
fn validated_network_keypair(
    identity_service: &IdentityService,
) -> Result<libp2p::identity::Keypair, AppError> {
    let unlocked_keys = identity_service.get_validated_unlocked_keys()?;
    let keypair =
        crate::p2p::swarm::ed25519_to_libp2p_keypair(&unlocked_keys.ed25519_signing.to_bytes())?;
    let network_peer_id = libp2p::PeerId::from(keypair.public()).to_string();
    let identity_info = identity_service
        .get_identity_info()?
        .ok_or_else(|| AppError::IdentityNotFound("Identity not found".into()))?;

    if identity_info.peer_id != network_peer_id {
        return Err(AppError::Crypto(
            "Stored PeerId does not match the network signing key".into(),
        ));
    }

    Ok(keypair)
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
    Ok(network.is_running().await)
}

/// Bootstrap the DHT (connect to bootstrap nodes)
#[tauri::command]
pub async fn bootstrap_network(network: State<'_, NetworkState>) -> Result<(), AppError> {
    let handle: NetworkHandle = network.get_handle().await?;
    handle.bootstrap().await
}

/// Services needed to start the P2P network
pub struct StartNetworkServices {
    pub db: Arc<Database>,
    pub identity_service: Arc<IdentityService>,
    pub messaging_service: Arc<MessagingService>,
    pub calling_service: Arc<CallingService>,
    pub contacts_service: Arc<ContactsService>,
    pub permissions_service: Arc<PermissionsService>,
    pub posts_service: Arc<PostsService>,
    pub content_sync_service: Arc<ContentSyncService>,
    pub wall_social_service: Arc<WallSocialService>,
    pub board_service: Arc<BoardService>,
    pub media_service: Arc<MediaStorageService>,
    pub mentions_service: Arc<MentionsService>,
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
    db: State<'_, Arc<Database>>,
    enable_mdns: Option<bool>,
    bootstrap_nodes: Option<Vec<String>>,
    identity_service: State<'_, Arc<IdentityService>>,
    messaging_service: State<'_, Arc<MessagingService>>,
    calling_service: State<'_, Arc<CallingService>>,
    contacts_service: State<'_, Arc<ContactsService>>,
    permissions_service: State<'_, Arc<PermissionsService>>,
    posts_service: State<'_, Arc<PostsService>>,
    content_sync_service: State<'_, Arc<ContentSyncService>>,
    wall_social_service: State<'_, Arc<WallSocialService>>,
    board_service: State<'_, Arc<BoardService>>,
    media_service: State<'_, Arc<MediaStorageService>>,
    mentions_service: State<'_, Arc<MentionsService>>,
) -> Result<(), AppError> {
    let services = StartNetworkServices {
        db: (*db).clone(),
        identity_service: (*identity_service).clone(),
        messaging_service: (*messaging_service).clone(),
        calling_service: (*calling_service).clone(),
        contacts_service: (*contacts_service).clone(),
        permissions_service: (*permissions_service).clone(),
        posts_service: (*posts_service).clone(),
        content_sync_service: (*content_sync_service).clone(),
        wall_social_service: (*wall_social_service).clone(),
        board_service: (*board_service).clone(),
        media_service: (*media_service).clone(),
        mentions_service: (*mentions_service).clone(),
    };
    start_network_with_services(app, network, services, enable_mdns, bootstrap_nodes).await
}

/// Internal implementation for starting the P2P network
async fn start_network_with_services(
    app: AppHandle,
    network: State<'_, NetworkState>,
    services: StartNetworkServices,
    enable_mdns: Option<bool>,
    bootstrap_nodes: Option<Vec<String>>,
) -> Result<(), AppError> {
    let mut runtime_guard = network.runtime.lock().await;
    let identity_service = &services.identity_service;
    // Check if identity is unlocked
    if !identity_service.is_unlocked() {
        return Err(AppError::IdentityLocked(
            "Identity must be unlocked to start network".to_string(),
        ));
    }

    // The lifecycle mutex is held through construction and publication. This is
    // deliberately not a check-then-set sequence: concurrent starts must share
    // the same serialized decision.
    if runtime_guard.is_some() {
        return Ok(());
    }

    // Fail closed before constructing the swarm or publishing a network handle.
    let keypair = validated_network_keypair(identity_service)?;

    // Build the actor config from durable settings before constructing the
    // swarm. Database nodes retain their priority order, with profile-local
    // settings appended and de-duplicated.
    let config = build_network_config(&services.db, enable_mdns, bootstrap_nodes)?;

    // Create network service - clone the Arc to pass to the service
    let identity_arc: Arc<IdentityService> = services.identity_service.clone();
    let (mut service, handle, mut event_rx) = NetworkService::new(config, identity_arc, keypair)?;

    // Inject services for message processing, contact storage, permissions, content sync, boards, and calls
    service.set_messaging_service(services.messaging_service.clone());
    service.set_calling_service(services.calling_service.clone());
    service.set_contacts_service(services.contacts_service.clone());
    service.set_permissions_service(services.permissions_service.clone());
    service.set_posts_service(services.posts_service.clone());
    service.set_content_sync_service(services.content_sync_service.clone());
    service.set_wall_social_service(services.wall_social_service.clone());
    service.set_board_service(services.board_service.clone());
    service.set_media_service(services.media_service.clone());
    service.set_mentions_service(services.mentions_service.clone());

    // Bind before exposing the handle. A failed listener must not appear as a
    // running network to commands or the UI.
    service.start_listening().await?;

    let (cancel, mut mention_cancel) = watch::channel(false);
    let relay_handle = handle.clone();
    let mention_delivery = services.mentions_service.clone();
    let mention_worker = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            tokio::select! {
                changed = mention_cancel.changed() => {
                    if changed.is_err() || *mention_cancel.borrow() {
                        break;
                    }
                }
                _ = async {
                    interval.tick().await;
                    let Ok(relay) = relay_handle.active_relay().await else {
                        return;
                    };
                    if let Ok(items) = mention_delivery
                        .queued_outbound(chrono::Utc::now().timestamp(), 25)
                    {
                        for item in items {
                            if let Ok((id, _)) = relay_handle
                                .submit_introduction(
                                    relay,
                                    item.target,
                                    item.mention_id.clone(),
                                    item.ephemeral_public_key,
                                    item.ciphertext,
                                    item.expires_at,
                                )
                                .await
                            {
                                if id == item.mention_id {
                                    let _ = mention_delivery.mark_outbound_delivered(&id);
                                }
                            }
                        }
                    }
                    let _ = relay_handle.fetch_introductions(relay, 50).await;
                } => {
                }
            }
        }
    });

    // Spawn the network service in a background task
    let (actor_exit_tx, actor_exit_rx) = oneshot::channel();
    let service_worker = tokio::spawn(async move {
        info!("Network service starting in background task");
        service.run_after_listening().await;
        let _ = actor_exit_tx.send(());
        info!("Network service stopped");
    });

    // Spawn a task to process network events and forward to frontend
    let app_clone = app.clone();
    let event_worker = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            log_network_event(&event);
            // Offers and answers are the signaling transitions the frontend
            // cannot reconstruct from persisted call metadata because they
            // carry SDP.
            // Re-emit them over a short bounded window so a WebView listener
            // that is briefly mounting or processing another event cannot lose
            // the only incoming-call transition. Frontend offer handling is
            // idempotent for the current call.
            let delivery_attempts = match &event {
                NetworkEvent::CallSignalingReceived { message, .. }
                    if matches!(
                        &message.payload,
                        crate::p2p::protocols::signaling::SignalingPayload::Offer(_)
                            | crate::p2p::protocols::signaling::SignalingPayload::Answer(_)
                    ) =>
                {
                    5
                }
                _ => 1,
            };
            for attempt in 0..delivery_attempts {
                if let Err(e) = app_clone.emit("harbor:network", &event) {
                    tracing::warn!("Failed to emit network event: {}", e);
                }
                if attempt + 1 < delivery_attempts {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    });

    let runtime_id = network.next_runtime_id.fetch_add(1, Ordering::Relaxed);
    let mentions_for_cleanup = services.mentions_service.clone();
    *runtime_guard = Some(NetworkRuntime {
        id: runtime_id,
        handle,
        cancel,
        workers: vec![mention_worker, service_worker, event_worker],
        runtime_cache_cleanup: Some(Arc::new(move || {
            mentions_for_cleanup.clear_runtime_cache();
        })),
    });
    drop(runtime_guard);

    // The actor owns the live sockets. If it exits without an explicit stop,
    // immediately withdraw the public handle and drain its profile workers.
    // Runtime IDs prevent a delayed exit notification from clearing a newer
    // clean start.
    let runtime_slot = network.runtime.clone();
    tokio::spawn(async move {
        let _ = actor_exit_rx.await;
        if let Err(error) = clear_runtime_after_actor_exit(&runtime_slot, runtime_id).await {
            tracing::warn!("Network actor exited during runtime cleanup: {error}");
        }
    });

    info!("Network started successfully");
    Ok(())
}

async fn clear_runtime_after_actor_exit(
    runtime_slot: &Arc<Mutex<Option<NetworkRuntime>>>,
    runtime_id: u64,
) -> Result<(), AppError> {
    let mut guard = runtime_slot.lock().await;
    if guard.as_ref().map(|runtime| runtime.id) != Some(runtime_id) {
        return Ok(());
    }
    stop_runtime(&mut guard).await
}

/// Stop the P2P network
#[tauri::command]
pub async fn stop_network(network: State<'_, NetworkState>) -> Result<(), AppError> {
    stop_network_state(&network).await
}

pub(crate) async fn stop_network_state(network: &NetworkState) -> Result<(), AppError> {
    let mut guard = network.runtime.lock().await;
    stop_runtime(&mut guard).await
}

async fn stop_runtime(runtime_slot: &mut Option<NetworkRuntime>) -> Result<(), AppError> {
    let Some(mut runtime) = runtime_slot.take() else {
        return Ok(());
    };

    // Taking the runtime first rejects new scoped work immediately. Cancellation
    // interrupts workers blocked on network requests; shutdown then lets the
    // swarm actor release sockets and event senders normally.
    let _ = runtime.cancel.send(true);
    if let Some(cleanup) = runtime.runtime_cache_cleanup.take() {
        cleanup();
    }
    let shutdown_error = runtime.handle.shutdown().await.err();
    let mut join_error = None;
    for worker in runtime.workers.drain(..) {
        if let Err(error) = worker.await {
            join_error.get_or_insert_with(|| error.to_string());
        }
    }

    if let Some(error) = shutdown_error {
        return Err(error);
    }
    if let Some(error) = join_error {
        return Err(AppError::Internal(format!(
            "Network worker failed during shutdown: {error}"
        )));
    }

    info!("Network stopped");
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

/// Public discovery metadata. Nothing in this unsigned bundle is trusted or
/// materialized as a contact until the signed identity handshake completes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct ContactBundle {
    pub version: u8,
    pub peer_id: String,
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
    /// Optional relay-signed name claim. This remains untrusted discovery
    /// metadata until the receiving client verifies it against a pinned relay key.
    pub relay_name_claim: Option<NameClaimDto>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddContactResult {
    pub request_id: String,
    pub peer_id: String,
    pub status: String,
    pub delivery: String,
}

const CONTACT_INVITE_VERSION: u8 = 1;
const NATIVE_CONTACT_INVITE_PREFIX: &str = "harbor://contact/v1/";
const WEB_CONTACT_INVITE_PREFIX: &str = "https://social-harbor.com/add-friend/v1/";
const WWW_WEB_CONTACT_INVITE_PREFIX: &str = "https://www.social-harbor.com/add-friend/v1/";
const WEB_CONTACT_INVITE_QUERY_PREFIX: &str = "https://social-harbor.com/add-friend?c=";
const WWW_WEB_CONTACT_INVITE_QUERY_PREFIX: &str = "https://www.social-harbor.com/add-friend?c=";
const MAX_CONTACT_INVITE_INPUT_LENGTH: usize = 8_192;
const MAX_CONTACT_INVITE_PAYLOAD_LENGTH: usize = 6_144;
const MAX_CONTACT_INVITE_DECODED_BYTES: usize = 4_096;

#[derive(Debug, Clone)]
struct ValidatedContactInvite {
    bundle: ContactBundle,
    peer_id: libp2p::PeerId,
    address: libp2p::Multiaddr,
}

fn decode_canonical_key(value: &str, label: &str) -> Result<Vec<u8>, AppError> {
    use base64::Engine;
    if value.len() > 64 {
        return Err(AppError::Validation(format!(
            "Invalid contact invite: {label} is malformed"
        )));
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| {
            AppError::Validation(format!("Invalid contact invite: {label} is malformed"))
        })?;
    if decoded.len() != 32 || base64::engine::general_purpose::STANDARD.encode(&decoded) != value {
        return Err(AppError::Validation(format!(
            "Invalid contact invite: {label} must be one canonical 32-byte key"
        )));
    }
    Ok(decoded)
}

fn extract_contact_invite_payload(input: &str) -> Result<&str, AppError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(AppError::Validation(
            "Invalid contact invite: the payload is missing".into(),
        ));
    }
    if input.len() > MAX_CONTACT_INVITE_INPUT_LENGTH {
        return Err(AppError::Validation(
            "Invalid contact invite: the link is too large".into(),
        ));
    }
    let payload = input
        .strip_prefix(NATIVE_CONTACT_INVITE_PREFIX)
        .or_else(|| input.strip_prefix(WEB_CONTACT_INVITE_PREFIX))
        .or_else(|| input.strip_prefix(WWW_WEB_CONTACT_INVITE_PREFIX))
        .or_else(|| input.strip_prefix(WEB_CONTACT_INVITE_QUERY_PREFIX))
        .or_else(|| input.strip_prefix(WWW_WEB_CONTACT_INVITE_QUERY_PREFIX))
        .ok_or_else(|| {
            AppError::Validation("Invalid contact invite: use a canonical Harbor v1 invite".into())
        })?;
    if payload.is_empty()
        || payload.len() > MAX_CONTACT_INVITE_PAYLOAD_LENGTH
        || !payload
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(AppError::Validation(
            "Invalid contact invite: malformed or oversized payload".into(),
        ));
    }
    Ok(payload)
}

fn parse_contact_invite_payload(payload: &str) -> Result<ValidatedContactInvite, AppError> {
    use base64::Engine;
    let json_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| AppError::Validation("Invalid contact invite: malformed payload".into()))?;
    if json_bytes.len() > MAX_CONTACT_INVITE_DECODED_BYTES
        || base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&json_bytes) != payload
    {
        return Err(AppError::Validation(
            "Invalid contact invite: non-canonical or oversized payload".into(),
        ));
    }
    let bundle: ContactBundle = serde_json::from_slice(&json_bytes)
        .map_err(|_| AppError::Validation("Invalid contact invite: malformed data".into()))?;
    if bundle.version != CONTACT_INVITE_VERSION
        || bundle.display_name.is_empty()
        || bundle.display_name.chars().count() > 128
        || bundle.multiaddr.is_empty()
        || bundle.multiaddr.len() > 2_048
        || bundle
            .bio
            .as_ref()
            .is_some_and(|value| value.chars().count() > 2_048)
        || bundle
            .avatar_hash
            .as_ref()
            .is_some_and(|value| value.len() > 512)
    {
        return Err(AppError::Validation(
            "Invalid contact invite: data is malformed or oversized".into(),
        ));
    }
    let peer_id = bundle
        .peer_id
        .parse::<libp2p::PeerId>()
        .map_err(|_| AppError::Validation("Invalid contact invite: malformed peer ID".into()))?;
    let address = bundle
        .multiaddr
        .parse::<libp2p::Multiaddr>()
        .map_err(|_| AppError::Validation("Invalid contact invite: malformed address".into()))?;
    let mut dial_route = address.clone();
    dial_route.pop();
    if dial_route.is_empty() {
        return Err(AppError::Validation(
            "Invalid contact invite: address has no dialable route".into(),
        ));
    }
    let mut address_peer = None;
    for protocol in address.iter() {
        if let libp2p::multiaddr::Protocol::P2p(peer) = protocol {
            address_peer = Some(peer);
        }
    }
    if address_peer.as_ref() != Some(&peer_id)
        || !bundle
            .multiaddr
            .ends_with(&format!("/p2p/{}", bundle.peer_id))
    {
        return Err(AppError::Validation(
            "Invalid contact invite: address and peer ID do not match".into(),
        ));
    }
    let public_key = decode_canonical_key(&bundle.public_key, "public key")?;
    let x25519_public = decode_canonical_key(&bundle.x25519_public, "encryption key")?;
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(
        public_key
            .as_slice()
            .try_into()
            .map_err(|_| AppError::Validation("Invalid contact invite: public key".into()))?,
    )
    .map_err(|_| AppError::Validation("Invalid contact invite: public key".into()))?;
    let derived_peer_id =
        crate::services::CryptoService::derive_peer_id_from_verifying_key(&verifying_key)?;
    if derived_peer_id != bundle.peer_id {
        return Err(AppError::Validation(
            "Invalid contact invite: public key and peer ID do not match".into(),
        ));
    }
    if let Some(claim) = bundle.relay_name_claim.as_ref() {
        if claim.request.peer_id != bundle.peer_id
            || claim.request.domain != crate::models::domain::NAME_CLAIM_REQUEST
            || claim.request.version != crate::models::PROTOCOL_VERSION
            || claim.request.ed25519_public_key != public_key
            || claim.request.x25519_public_key != x25519_public
            || claim.status != "active"
            || claim.request.local_name.is_empty()
            || claim.request.local_name.len() > 32
            || claim.request.relay.is_empty()
            || claim.request.relay.len() > 253
            || claim.request.sequence == 0
            || claim.request.nonce.len() < 16
            || claim.user_signature.len() != 64
            || claim.relay_signature.len() != 64
            || claim.not_after <= claim.not_before
        {
            return Err(AppError::Validation(
                "Invalid contact invite: relay name claim does not match the invited identity"
                    .into(),
            ));
        }
    }
    Ok(ValidatedContactInvite {
        bundle,
        peer_id,
        address,
    })
}

fn parse_contact_invite(input: &str) -> Result<ValidatedContactInvite, AppError> {
    parse_contact_invite_payload(extract_contact_invite_payload(input)?)
}

/// Normalize all supported first-party invite forms at the command boundary.
/// The frontend performs the same checks for immediate feedback, but the
/// backend remains authoritative because invoke callers are untrusted.
pub(crate) fn normalize_contact_invite(input: &str) -> Result<String, AppError> {
    let payload = extract_contact_invite_payload(input)?;
    parse_contact_invite_payload(payload)?;
    Ok(format!("{NATIVE_CONTACT_INVITE_PREFIX}{payload}"))
}

/// Generate a shareable contact string that includes all info needed to add as contact
/// Format: harbor://<base64_encoded_json>
#[tauri::command]
pub async fn get_shareable_contact_string(
    network: State<'_, NetworkState>,
    identity_service: State<'_, Arc<IdentityService>>,
    db: State<'_, Arc<Database>>,
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

    let relay_name_claim = crate::services::name_claim_service::verified_name_claim(
        &RelayNamesRepository::new(&db),
        &identity.peer_id,
        chrono::Utc::now().timestamp(),
    )
    .map_err(|error| AppError::Crypto(error.to_string()))?
    .map(|(claim, _)| NameClaimDto::from(claim));

    let bundle = ContactBundle {
        version: CONTACT_INVITE_VERSION,
        peer_id: identity.peer_id.clone(),
        multiaddr,
        display_name: identity.display_name,
        public_key: keys.public_key,
        x25519_public: keys.x25519_public,
        bio: identity.bio,
        avatar_hash: identity.avatar_hash,
        relay_name_claim,
    };

    let json = serde_json::to_string(&bundle)
        .map_err(|e| AppError::Serialization(format!("Failed to serialize contact: {}", e)))?;

    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes());

    Ok(format!("{NATIVE_CONTACT_INVITE_PREFIX}{encoded}"))
}

/// Add a contact from a shareable contact string and connect to them
/// This is the simplified flow - no handshake needed
#[tauri::command]
pub async fn add_contact_from_string(
    network: State<'_, NetworkState>,
    contacts_service: State<'_, Arc<ContactsService>>,
    identity_service: State<'_, Arc<IdentityService>>,
    contact_string: String,
) -> Result<AddContactResult, AppError> {
    let invite = parse_contact_invite(&contact_string)?;
    let local_peer_id = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::IdentityNotFound("Identity not found".into()))?
        .peer_id;
    if invite.bundle.peer_id == local_peer_id {
        return Err(AppError::Validation(
            "Cannot add yourself as a contact".into(),
        ));
    }
    if contacts_service.is_contact(&invite.bundle.peer_id)? {
        return Err(AppError::Validation(
            "This person is already a contact".into(),
        ));
    }
    let request_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    contacts_service.record_contact_request(
        &request_id,
        &invite.bundle.peer_id,
        "outgoing",
        Some(&invite.bundle.display_name),
        None,
        None,
        invite.bundle.avatar_hash.as_deref(),
        invite.bundle.bio.as_deref(),
        "pending",
        Some("request"),
        None,
        now,
    )?;

    let handle = match network.get_handle().await {
        Ok(handle) => handle,
        Err(error) => {
            contacts_service.update_contact_request(
                &request_id,
                "failed",
                Some("request"),
                Some(&error.to_string()),
                chrono::Utc::now().timestamp(),
            )?;
            return Err(error);
        }
    };
    let mut dial_address = invite.address.clone();
    dial_address.pop();
    let _ = handle.dial(invite.peer_id, vec![dial_address]).await;
    let connected = handle
        .get_connected_peers()
        .await
        .map(|peers| {
            peers
                .iter()
                .any(|candidate| candidate.peer_id == invite.bundle.peer_id)
        })
        .unwrap_or(false);
    if let Err(error) = handle
        .request_identity_action(invite.peer_id, request_id.clone(), "request".into())
        .await
    {
        contacts_service.update_contact_request(
            &request_id,
            "failed",
            Some("request"),
            Some(&error.to_string()),
            chrono::Utc::now().timestamp(),
        )?;
        return Err(error);
    }

    info!(
        "Queued signed contact request for {} ({}) from invite discovery metadata",
        invite.bundle.display_name, invite.bundle.peer_id
    );

    Ok(AddContactResult {
        request_id,
        peer_id: invite.bundle.peer_id,
        status: "pending".into(),
        delivery: if connected { "connected" } else { "offline" }.into(),
    })
}

#[cfg(test)]
mod contact_invite_tests {
    use super::{
        normalize_contact_invite, parse_contact_invite, ContactBundle,
        MAX_CONTACT_INVITE_INPUT_LENGTH, NATIVE_CONTACT_INVITE_PREFIX,
    };
    use crate::db::{Capability, ContactData, Database};
    use crate::models::CreateIdentityRequest;
    use crate::services::{
        ContactsService, IdentityService, PermissionGrantMessage, PermissionsService, Signable,
        SignablePermissionGrant,
    };
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};
    use std::sync::Arc;

    fn bundle(key: &SigningKey) -> ContactBundle {
        let peer_id =
            crate::services::CryptoService::derive_peer_id_from_verifying_key(&key.verifying_key())
                .unwrap();
        ContactBundle {
            version: 1,
            peer_id: peer_id.clone(),
            multiaddr: format!("/ip4/127.0.0.1/tcp/4001/p2p/{peer_id}"),
            display_name: "Alice".into(),
            public_key: base64::engine::general_purpose::STANDARD
                .encode(key.verifying_key().to_bytes()),
            x25519_public: base64::engine::general_purpose::STANDARD.encode([8; 32]),
            bio: Some("Hello".into()),
            avatar_hash: None,
            relay_name_claim: None,
        }
    }

    fn invite(bundle: &ContactBundle) -> String {
        let json = serde_json::to_vec(bundle).unwrap();
        format!(
            "{NATIVE_CONTACT_INVITE_PREFIX}{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)
        )
    }

    #[test]
    fn normalizes_only_native_and_official_versioned_web_invites() {
        let native = invite(&bundle(&SigningKey::from_bytes(&[31; 32])));
        let payload = native.strip_prefix(NATIVE_CONTACT_INVITE_PREFIX).unwrap();
        assert_eq!(normalize_contact_invite(&native).unwrap(), native);
        assert_eq!(
            normalize_contact_invite(&format!(
                "https://social-harbor.com/add-friend/v1/{payload}"
            ))
            .unwrap(),
            native
        );
        assert_eq!(
            normalize_contact_invite(&format!("https://social-harbor.com/add-friend?c={payload}"))
                .unwrap(),
            native
        );
    }

    #[test]
    fn relay_name_claim_must_bind_to_the_invited_identity() {
        let key = SigningKey::from_bytes(&[35; 32]);
        let mut contact = bundle(&key);
        let other = bundle(&SigningKey::from_bytes(&[36; 32]));
        contact.relay_name_claim = Some(
            crate::models::NameClaim {
                request: crate::models::NameClaimRequest {
                    domain: crate::models::domain::NAME_CLAIM_REQUEST.into(),
                    version: crate::models::PROTOCOL_VERSION,
                    local_name: "alice".into(),
                    relay: "harbor.social".into(),
                    peer_id: other.peer_id,
                    ed25519_public_key: key.verifying_key().to_bytes().to_vec(),
                    x25519_public_key: vec![8; 32],
                    sequence: 1,
                    issued_at: 100,
                    nonce: vec![1; 16],
                },
                user_signature: vec![2; 64],
                status: "active".into(),
                not_before: 100,
                not_after: 200,
                relay_key_id: "relay-key".into(),
                relay_signature: vec![3; 64],
            }
            .into(),
        );

        assert!(parse_contact_invite(&invite(&contact)).is_err());
    }

    #[test]
    fn rejects_tampered_mismatched_double_encoded_and_malformed_invites() {
        let key = SigningKey::from_bytes(&[32; 32]);
        let original = bundle(&key);

        let mut mismatched_key = original.clone();
        mismatched_key.public_key = bundle(&SigningKey::from_bytes(&[33; 32])).public_key;
        assert!(parse_contact_invite(&invite(&mismatched_key)).is_err());

        let mut mismatched_address = original.clone();
        let other = bundle(&SigningKey::from_bytes(&[34; 32]));
        mismatched_address.multiaddr = other.multiaddr;
        assert!(parse_contact_invite(&invite(&mismatched_address)).is_err());

        let mut double_encoded = original;
        double_encoded.public_key =
            base64::engine::general_purpose::STANDARD.encode(double_encoded.public_key.as_bytes());
        assert!(parse_contact_invite(&invite(&double_encoded)).is_err());

        for malformed in [
            "harbor://legacy",
            "harbor://contact/v1/not+base64",
            "https://social-harbor.com.evil.test/add-friend/v1/abc",
            "https://social-harbor.com/add-friend/v1/abc?redirect=evil",
            "https://social-harbor.com/add-friend/v1/abc/extra",
            "https://social-harbor.com/add-friend?c=abc&redirect=evil",
        ] {
            assert!(normalize_contact_invite(malformed).is_err(), "{malformed}");
        }
        assert!(
            normalize_contact_invite(&"x".repeat(MAX_CONTACT_INVITE_INPUT_LENGTH + 1)).is_err()
        );
    }

    #[test]
    fn valid_invite_is_discovery_only_until_signed_atomic_acceptance() {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity = Arc::new(IdentityService::new(db.clone()));
        let local = identity
            .create_identity(CreateIdentityRequest {
                display_name: "Local".into(),
                passphrase: "password123".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        let contacts = ContactsService::new(db.clone(), identity.clone());
        let permissions = PermissionsService::new(db.clone(), identity);
        let remote_key = SigningKey::from_bytes(&[35; 32]);
        let parsed = parse_contact_invite(&invite(&bundle(&remote_key))).unwrap();
        let request_id = "invite-handshake-request";
        contacts
            .record_contact_request(
                request_id,
                &parsed.bundle.peer_id,
                "outgoing",
                Some(&parsed.bundle.display_name),
                None,
                None,
                parsed.bundle.avatar_hash.as_deref(),
                parsed.bundle.bio.as_deref(),
                "pending",
                Some("request"),
                None,
                10,
            )
            .unwrap();

        assert!(!contacts.is_contact(&parsed.bundle.peer_id).unwrap());
        let request = contacts.contact_request(request_id).unwrap().unwrap();
        assert!(request.public_key.is_none());
        assert!(request.x25519_public.is_none());
        for capability in [Capability::Chat, Capability::WallRead, Capability::Call] {
            assert!(!permissions
                .we_have_capability(&parsed.bundle.peer_id, capability)
                .unwrap());
        }

        let grants: Vec<_> = [Capability::Chat, Capability::WallRead, Capability::Call]
            .into_iter()
            .enumerate()
            .map(|(index, capability)| {
                let signable = SignablePermissionGrant {
                    grant_id: format!("remote-grant-{index}"),
                    issuer_peer_id: parsed.bundle.peer_id.clone(),
                    subject_peer_id: local.peer_id.clone(),
                    capability: capability.as_str().into(),
                    scope: None,
                    lamport_clock: index as u64 + 1,
                    issued_at: 20,
                    expires_at: None,
                };
                let payload_cbor = signable.signable_bytes().unwrap();
                PermissionGrantMessage {
                    grant_id: signable.grant_id.clone(),
                    issuer_peer_id: signable.issuer_peer_id.clone(),
                    subject_peer_id: signable.subject_peer_id.clone(),
                    capability: signable.capability.clone(),
                    scope: None,
                    lamport_clock: signable.lamport_clock,
                    issued_at: signable.issued_at,
                    expires_at: None,
                    signature: remote_key.sign(&payload_cbor).to_bytes().to_vec(),
                    payload_cbor,
                }
            })
            .collect();
        for grant in &grants {
            permissions
                .validate_incoming_grant(
                    grant,
                    &base64::engine::general_purpose::STANDARD
                        .decode(&parsed.bundle.public_key)
                        .unwrap(),
                )
                .unwrap();
        }
        contacts
            .accept_contact_request_atomically(
                request_id,
                "outgoing",
                &ContactData {
                    peer_id: parsed.bundle.peer_id.clone(),
                    public_key: base64::engine::general_purpose::STANDARD
                        .decode(&parsed.bundle.public_key)
                        .unwrap(),
                    x25519_public: base64::engine::general_purpose::STANDARD
                        .decode(&parsed.bundle.x25519_public)
                        .unwrap(),
                    display_name: parsed.bundle.display_name,
                    avatar_hash: parsed.bundle.avatar_hash,
                    bio: parsed.bundle.bio,
                },
                &grants,
                20,
            )
            .unwrap();
        assert!(contacts.is_contact(&request.peer_id).unwrap());
        for capability in [Capability::Chat, Capability::WallRead, Capability::Call] {
            assert!(permissions
                .we_have_capability(&request.peer_id, capability)
                .unwrap());
        }
    }
}

#[cfg(test)]
mod identity_preflight_tests {
    use super::{
        build_network_config, clear_runtime_after_actor_exit, stop_runtime,
        validated_network_keypair, NetworkRuntime, NetworkState,
    };
    use crate::db::repositories::bootstrap_repo::{AddBootstrapNodeInput, BootstrapNodesRepo};
    use crate::db::Database;
    use crate::error::AppError;
    use crate::models::CreateIdentityRequest;
    use crate::p2p::NetworkHandle;
    use crate::services::{CryptoService, IdentityService};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use tokio::sync::watch;

    enum PublicIdentityCorruption {
        PeerId,
        Ed25519Public,
        X25519Public,
    }

    fn corrupted_unlocked_identity(
        corruption: PublicIdentityCorruption,
    ) -> (Arc<Database>, IdentityService) {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity = IdentityService::new(db.clone());
        identity
            .create_identity(CreateIdentityRequest {
                display_name: "Preflight Test".into(),
                passphrase: "test-passphrase".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();

        db.with_connection(|connection| {
            match corruption {
                PublicIdentityCorruption::PeerId => {
                    let signing = CryptoService::generate_ed25519_keypair().0;
                    let peer_id = CryptoService::derive_peer_id_from_signing_key(&signing).unwrap();
                    connection
                        .execute("UPDATE local_identity SET peer_id=? WHERE id=1", [peer_id])?;
                }
                PublicIdentityCorruption::Ed25519Public => {
                    let public = CryptoService::generate_ed25519_keypair()
                        .1
                        .to_bytes()
                        .to_vec();
                    connection.execute(
                        "UPDATE local_identity SET public_key=? WHERE id=1",
                        [public],
                    )?;
                }
                PublicIdentityCorruption::X25519Public => {
                    let public = CryptoService::generate_x25519_keypair()
                        .1
                        .to_bytes()
                        .to_vec();
                    connection.execute(
                        "UPDATE local_identity SET x25519_public=? WHERE id=1",
                        [public],
                    )?;
                }
            }
            Ok(())
        })
        .unwrap();

        (db, identity)
    }

    async fn assert_preflight_rejects_without_handle(corruption: PublicIdentityCorruption) {
        let (_db, identity) = corrupted_unlocked_identity(corruption);
        let network = NetworkState::new();

        assert!(matches!(
            validated_network_keypair(&identity),
            Err(AppError::Crypto(_))
        ));
        assert!(!identity.is_unlocked());
        assert!(matches!(
            network.get_handle().await,
            Err(AppError::NetworkNotInitialized(_))
        ));
    }

    #[tokio::test]
    async fn network_preflight_rejects_inconsistent_peer_id_without_publishing_handle() {
        assert_preflight_rejects_without_handle(PublicIdentityCorruption::PeerId).await;
    }

    #[tokio::test]
    async fn network_preflight_rejects_inconsistent_ed25519_public_without_publishing_handle() {
        assert_preflight_rejects_without_handle(PublicIdentityCorruption::Ed25519Public).await;
    }

    #[tokio::test]
    async fn network_preflight_rejects_inconsistent_x25519_public_without_publishing_handle() {
        assert_preflight_rejects_without_handle(PublicIdentityCorruption::X25519Public).await;
    }

    #[tokio::test]
    async fn lock_stops_and_joins_profile_runtime_before_clearing_keys() {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity = IdentityService::new(db);
        identity
            .create_identity(CreateIdentityRequest {
                display_name: "Runtime Test".into(),
                passphrase: "test-passphrase".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        assert!(identity.is_unlocked());

        let network = NetworkState::new();
        let (handle, actor, mut shutdown_rx) = NetworkHandle::test_shutdown_runtime();
        let (cancel, mut cancel_rx) = watch::channel(false);
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancelled_worker = cancelled.clone();
        let worker = tokio::spawn(async move {
            if cancel_rx.changed().await.is_ok() && *cancel_rx.borrow() {
                cancelled_worker.store(true, Ordering::SeqCst);
            }
        });
        *network.runtime.lock().await = Some(NetworkRuntime {
            id: 1,
            handle,
            cancel,
            workers: vec![actor, worker],
            runtime_cache_cleanup: None,
        });

        network.stop_and_lock_identity(&identity).await.unwrap();

        assert!(!identity.is_unlocked());
        assert!(cancelled.load(Ordering::SeqCst));
        assert!(shutdown_rx.try_recv().is_ok());
        assert!(!network.is_running().await);
    }

    #[tokio::test]
    async fn stop_awaits_scoped_work_and_rejects_work_afterward() {
        let network = NetworkState::new();
        let (handle, actor, _shutdown_rx) = NetworkHandle::test_shutdown_runtime();
        let (cancel, _cancel_rx) = watch::channel(false);
        let runtime_cache_cleared = Arc::new(AtomicBool::new(false));
        let cleanup_observer = runtime_cache_cleared.clone();
        *network.runtime.lock().await = Some(NetworkRuntime {
            id: 1,
            handle,
            cancel,
            workers: vec![actor],
            runtime_cache_cleanup: Some(Arc::new(move || {
                cleanup_observer.store(true, Ordering::SeqCst);
            })),
        });

        let completed = Arc::new(AtomicBool::new(false));
        let worker_completed = completed.clone();
        network
            .spawn_scoped(async move {
                tokio::task::yield_now().await;
                worker_completed.store(true, Ordering::SeqCst);
            })
            .await
            .unwrap();

        super::stop_network_state(&network).await.unwrap();

        assert!(completed.load(Ordering::SeqCst));
        assert!(runtime_cache_cleared.load(Ordering::SeqCst));
        assert!(matches!(
            network.spawn_scoped(async {}).await,
            Err(AppError::NetworkNotInitialized(_))
        ));
    }

    fn bootstrap_address(port: u16) -> String {
        let peer_id = libp2p::PeerId::from(libp2p::identity::Keypair::generate_ed25519().public());
        format!("/ip4/127.0.0.1/tcp/{port}/p2p/{peer_id}")
    }

    #[test]
    fn persisted_network_config_honours_mdns_priority_and_enabled_nodes() {
        let db = Database::in_memory().unwrap();
        let lower_priority = bootstrap_address(4101);
        let higher_priority = bootstrap_address(4102);
        let disabled = bootstrap_address(4103);
        let requested = bootstrap_address(4104);

        BootstrapNodesRepo::add(
            &db,
            AddBootstrapNodeInput {
                address: lower_priority.clone(),
                name: None,
                priority: Some(20),
                is_default: None,
            },
        )
        .unwrap();
        BootstrapNodesRepo::add(
            &db,
            AddBootstrapNodeInput {
                address: higher_priority.clone(),
                name: None,
                priority: Some(5),
                is_default: None,
            },
        )
        .unwrap();
        let disabled_id = BootstrapNodesRepo::add(
            &db,
            AddBootstrapNodeInput {
                address: disabled,
                name: None,
                priority: Some(0),
                is_default: None,
            },
        )
        .unwrap();
        BootstrapNodesRepo::update(&db, disabled_id, None, Some(false), None).unwrap();

        let config = build_network_config(
            &db,
            Some(false),
            Some(vec![higher_priority.clone(), requested.clone()]),
        )
        .unwrap();

        assert!(!config.enable_mdns);
        assert_eq!(
            config
                .bootstrap_nodes
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec![higher_priority, lower_priority, requested]
        );
    }

    #[tokio::test]
    async fn actor_exit_clears_only_its_runtime_and_allows_a_clean_restart() {
        let network = NetworkState::new();
        let (handle, actor, _shutdown_rx) = NetworkHandle::test_shutdown_runtime();
        let (cancel, _cancel_rx) = watch::channel(false);
        *network.runtime.lock().await = Some(NetworkRuntime {
            id: 11,
            handle,
            cancel,
            workers: vec![actor],
            runtime_cache_cleanup: None,
        });

        clear_runtime_after_actor_exit(&network.runtime, 11)
            .await
            .unwrap();
        assert!(!network.is_running().await);

        let (handle, actor, _shutdown_rx) = NetworkHandle::test_shutdown_runtime();
        let (cancel, _cancel_rx) = watch::channel(false);
        *network.runtime.lock().await = Some(NetworkRuntime {
            id: 12,
            handle,
            cancel,
            workers: vec![actor],
            runtime_cache_cleanup: None,
        });
        clear_runtime_after_actor_exit(&network.runtime, 11)
            .await
            .unwrap();
        assert!(network.is_running().await);

        let mut guard = network.runtime.lock().await;
        stop_runtime(&mut guard).await.unwrap();
    }
}
