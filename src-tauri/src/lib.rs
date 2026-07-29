pub mod commands;
pub mod control;
pub mod db;
pub mod error;
pub mod logging;
pub mod models;
pub mod p2p;
pub mod profile_root;
pub mod services;

use commands::NetworkState;
use commands::{
    startup_initialization_failure, IdentityInitializationFailureSource, StartupInitializationState,
};
use db::Database;
use error::AppError;
use logging::LogConfig;
use profile_root::ProfileRoot;
use services::{
    AccountBackupService, AccountsService, BoardService, CallingService, ContactsService,
    ContentSyncService, FeedService, IdentityService, MediaStorageService, MentionsService,
    MessagingService, PermissionsService, PostsService, WallSocialService,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use tracing::info;

pub struct LogDirectory(pub PathBuf);

/// Holds deep-link contact strings received while the identity was locked.
/// Multiple links can arrive before the user unlocks (e.g. clicking two share links
/// in quick succession). All are queued here and drained after a successful unlock.
pub struct PendingDeepLink(pub Mutex<Vec<String>>);

/// Owns the disposable directory used only to render a startup recovery state.
/// No account data is opened or mutated when startup validation has failed.
pub struct RecoveryDirectory(pub PathBuf);

impl Drop for RecoveryDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Every profile-scoped backend dependency is opened from one selected root.
/// Keeping construction here prevents a database from one account being paired
/// with media or services from another account during startup or future restarts.
pub struct ProfileServices {
    pub db: Arc<Database>,
    pub identity: Arc<IdentityService>,
    pub contacts: Arc<ContactsService>,
    pub permissions: Arc<PermissionsService>,
    pub messaging: Arc<MessagingService>,
    pub posts: Arc<PostsService>,
    pub feed: Arc<FeedService>,
    pub mentions: Arc<MentionsService>,
    pub calling: Arc<CallingService>,
    pub content_sync: Arc<ContentSyncService>,
    pub wall_social: Arc<WallSocialService>,
    pub boards: Arc<BoardService>,
    pub media: Arc<MediaStorageService>,
}

impl ProfileServices {
    pub fn open(profile_root: &ProfileRoot) -> Result<Self, AppError> {
        let db = Arc::new(Database::new(profile_root.database())?);
        Self::from_database(profile_root.path(), db, true)
    }

    fn open_recovery(profile_root: &ProfileRoot) -> Result<Self, AppError> {
        let db = Arc::new(Database::in_memory()?);
        Self::from_database(profile_root.path(), db, false)
    }

    fn from_database(
        data_dir: &std::path::Path,
        db: Arc<Database>,
        reconcile_media: bool,
    ) -> Result<Self, AppError> {
        let identity = Arc::new(IdentityService::new(db.clone()));
        let contacts = Arc::new(ContactsService::new(db.clone(), identity.clone()));
        let permissions = Arc::new(PermissionsService::new(db.clone(), identity.clone()));
        let messaging = Arc::new(MessagingService::new(
            db.clone(),
            identity.clone(),
            contacts.clone(),
            permissions.clone(),
        ));
        let posts = Arc::new(PostsService::new(
            db.clone(),
            identity.clone(),
            contacts.clone(),
            permissions.clone(),
        ));
        let feed = Arc::new(FeedService::new(
            db.clone(),
            identity.clone(),
            permissions.clone(),
            contacts.clone(),
        ));
        let mentions = Arc::new(MentionsService::new(
            db.clone(),
            identity.clone(),
            contacts.clone(),
            posts.clone(),
        ));
        let calling = Arc::new(CallingService::new(
            db.clone(),
            identity.clone(),
            contacts.clone(),
            permissions.clone(),
        ));
        let content_sync = Arc::new(ContentSyncService::new(
            db.clone(),
            identity.clone(),
            contacts.clone(),
            permissions.clone(),
        ));
        let wall_social = Arc::new(WallSocialService::new(
            db.clone(),
            identity.clone(),
            contacts.clone(),
            permissions.clone(),
        ));
        let boards = Arc::new(BoardService::new(db.clone(), identity.clone()));
        let media = Arc::new(MediaStorageService::new(data_dir, db.clone())?);
        if reconcile_media {
            if let Err(error) = media.reconstruct_transfers() {
                tracing::warn!("Failed to reconstruct media transfer state: {error}");
            }
            if let Err(error) = media.enforce_cache_policy() {
                tracing::warn!("Failed to reconcile the media cache: {error}");
            }
        }

        Ok(Self {
            db,
            identity,
            contacts,
            permissions,
            messaging,
            posts,
            feed,
            mentions,
            calling,
            content_sync,
            wall_social,
            boards,
            media,
        })
    }
}

/// Get the profile name from environment variable (for multi-instance support)
fn get_profile_name() -> Option<String> {
    std::env::var("HARBOR_PROFILE")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Get custom data directory from environment variable
fn get_custom_data_dir() -> Option<PathBuf> {
    std::env::var("HARBOR_DATA_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

fn headless_media_capture_validation_enabled() -> bool {
    matches!(
        std::env::var("HARBOR_HEADLESS_MEDIA_CAPTURE").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

const HEADLESS_MEDIA_CAPTURE_SCRIPT: &str = r#"
(() => {
  globalThis.__HARBOR_HEADLESS_MEDIA_CAPTURE__ = true;
})();
"#;

fn configure_headless_media_capture(
    window: &tauri::WebviewWindow,
    enabled: bool,
) -> tauri::Result<()> {
    if enabled {
        window.eval(HEADLESS_MEDIA_CAPTURE_SCRIPT)?;
        info!("Headless media-capture validation marker enabled");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn configure_linux_webkit_call_media(
    window: &tauri::WebviewWindow,
    allow_headless_permissions: bool,
) -> tauri::Result<()> {
    use webkit2gtk::{PermissionRequestExt, SettingsExt, WebViewExt};

    window.with_webview(move |webview| {
        let webview = webview.inner();

        if let Some(settings) = webview.settings() {
            settings.set_enable_webrtc(true);
            settings.set_enable_media_stream(true);
            if allow_headless_permissions {
                settings.set_enable_write_console_messages_to_stdout(true);
            }
            info!(
                enable_webrtc = settings.enables_webrtc(),
                enable_media_stream = settings.enables_media_stream(),
                enable_mock_capture_devices = settings.enables_mock_capture_devices(),
                "Configured Linux WebKit call media runtime"
            );
        } else {
            info!("WebKit settings unavailable while configuring call media runtime");
        }

        if allow_headless_permissions {
            webview.connect_permission_request(|_, request| {
                info!("Allowing WebKit permission request for headless media-capture validation");
                request.allow();
                true
            });
        }
    })
}

#[cfg(not(target_os = "linux"))]
fn configure_linux_webkit_call_media(
    _window: &tauri::WebviewWindow,
    _allow_headless_permissions: bool,
) -> tauri::Result<()> {
    Ok(())
}

/// Normalize, validate, and route a harbor:// URL to the frontend.
/// Called from both the deep-link on_open_url handler and the single-instance callback.
fn handle_deep_link(app: &tauri::AppHandle, url: &str) {
    let contact_string = match commands::network::normalize_contact_invite(url) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "Ignoring invalid or unsupported Harbor deep link");
            return;
        }
    };
    let identity_service = app.state::<Arc<IdentityService>>();
    if identity_service.is_unlocked() {
        let _ = app.emit("deep_link_contact", &contact_string);
    } else if let Ok(mut queue) = app.state::<PendingDeepLink>().0.lock() {
        queue.push(contact_string);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let profile = get_profile_name();
    let uses_isolated_profile = profile.is_some() || get_custom_data_dir().is_some();
    let mut context = tauri::generate_context!();

    // The selected account is not known until setup reads and validates the
    // installation registry. Always delay window construction so its WebView
    // storage can be isolated under exactly the same selected profile root.
    for window in &mut context.config_mut().app.windows {
        window.create = false;
    }

    let mut builder = tauri::Builder::default();

    // The single-instance plugin is intentionally disabled for explicit validation/dev profiles.
    // Voice-call and wall-sync evidence runs launch multiple desktop processes side by side with
    // different HARBOR_PROFILE/HARBOR_DATA_DIR values; a global single-instance lock would route the
    // second launch back to the first window and prevent the second isolated database/window from
    // being created.
    if !uses_isolated_profile {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // Bring the existing window to the foreground
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
            // Route the deep link URL if present in the launch arguments
            if let Some(url) = args.iter().find(|a| a.starts_with("harbor://")) {
                handle_deep_link(app, url);
            }
        }));
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(move |app| {
            // Get app data directory first so we can set up logging properly
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");
            let installation_root =
                ProfileRoot::resolve(&app_data_dir, get_custom_data_dir(), profile.as_deref());

            // The registry belongs to the installation boundary. Resolve and
            // verify its active account before any profile database, service, or
            // WebView storage is opened.
            let accounts_registry = AccountsService::new(installation_root.path().to_path_buf());
            let startup = (|| {
                std::fs::create_dir_all(installation_root.path()).map_err(|error| {
                    (
                        IdentityInitializationFailureSource::AccountRegistry,
                        AppError::Io(error),
                    )
                })?;
                AccountBackupService::reconcile_pending_deletions(&accounts_registry).map_err(
                    |error| (IdentityInitializationFailureSource::AccountRegistry, error),
                )?;
                if let Some(account) = accounts_registry
                    .migrate_legacy_account(&installation_root.database())
                    .map_err(|error| {
                        (IdentityInitializationFailureSource::AccountRegistry, error)
                    })?
                {
                    info!("Migrated legacy account: {}", account.display_name);
                }
                let profile_root =
                    ProfileRoot::from_path(accounts_registry.resolve_active_data_dir().map_err(
                        |error| (IdentityInitializationFailureSource::AccountRegistry, error),
                    )?);
                std::fs::create_dir_all(profile_root.path()).map_err(|error| {
                    (
                        IdentityInitializationFailureSource::IdentityDatabase,
                        AppError::Io(error),
                    )
                })?;
                let accounts_service = accounts_registry
                    .clone()
                    .with_runtime_data_dir(profile_root.path())
                    .map_err(|error| {
                        (IdentityInitializationFailureSource::AccountRegistry, error)
                    })?;
                let services = ProfileServices::open(&profile_root).map_err(|error| {
                    (IdentityInitializationFailureSource::IdentityDatabase, error)
                })?;
                Ok::<_, (IdentityInitializationFailureSource, AppError)>((
                    profile_root,
                    accounts_service,
                    services,
                ))
            })();

            let (profile_root, accounts_service, services, startup_failure, recovery_dir) =
                match startup {
                    Ok((profile_root, accounts_service, services)) => (
                        profile_root,
                        Arc::new(accounts_service),
                        services,
                        None,
                        None,
                    ),
                    Err((source, error)) => {
                        tracing::error!(%error, "Profile startup validation failed");
                        let failure = startup_initialization_failure(source, error);
                        let recovery_path = std::env::temp_dir()
                            .join(format!("harbor-recovery-{}", std::process::id()));
                        let _ = std::fs::remove_dir_all(&recovery_path);
                        std::fs::create_dir_all(&recovery_path)?;
                        let profile_root = ProfileRoot::from_path(recovery_path.clone());
                        let services = ProfileServices::open_recovery(&profile_root)?;
                        (
                            profile_root,
                            Arc::new(accounts_registry),
                            services,
                            Some(failure),
                            Some(RecoveryDirectory(recovery_path)),
                        )
                    }
                };

            let window_config = app
                .config()
                .app
                .windows
                .iter()
                .find(|window| window.label == "main")
                .ok_or_else(|| tauri::Error::AssetNotFound("main window config".into()))?;
            let allow_headless_permissions = headless_media_capture_validation_enabled();
            let mut window_builder =
                tauri::WebviewWindowBuilder::from_config(app.handle(), window_config)?
                    .data_directory(profile_root.webview());
            if allow_headless_permissions {
                // Run before every document loads. A post-build eval can race the
                // initial navigation on slower ARM64 WebKit processes and leave
                // the real device API in place during unattended validation.
                window_builder =
                    window_builder.initialization_script(HEADLESS_MEDIA_CAPTURE_SCRIPT);
            }
            window_builder.build()?;
            if let Some(window) = app.get_webview_window("main") {
                let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
                window.set_icon(icon)?;
            }

            // Set up log directory
            let log_dir = profile_root.logs();

            // Initialize logging with appropriate config based on build type
            #[cfg(debug_assertions)]
            {
                logging::init_logging(LogConfig::development());
            }
            #[cfg(not(debug_assertions))]
            {
                // Production: enable file logging with JSON format
                logging::init_logging(LogConfig::production(log_dir.clone()));
                // Clean up old log files
                if let Err(e) = logging::cleanup_old_logs(&log_dir, 5) {
                    // Can't use info! here as logging might not be fully set up
                    eprintln!("Could not clean up old logs: {}", e);
                }
            }

            if let Some(ref p) = profile {
                info!("Starting Harbor with profile: {}", p);
            } else {
                info!("Starting Harbor...");
            }

            // Update window title if running with a profile
            if let Some(ref profile_name) = profile {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_title(&format!("Harbor - {}", profile_name));
                }
            }

            if let Some(window) = app.get_webview_window("main") {
                configure_linux_webkit_call_media(&window, allow_headless_permissions)?;
                configure_headless_media_capture(&window, allow_headless_permissions)?;
                if allow_headless_permissions {
                    info!("Headless media-capture validation override enabled");
                }
            }

            app.manage(LogDirectory(log_dir));
            if let Some(recovery_dir) = recovery_dir {
                app.manage(recovery_dir);
            }

            info!("Database path: {:?}", profile_root.database());

            // Initialize network state (will be populated when identity is unlocked)
            let network_state = NetworkState::new();

            // Register state
            let account_backup_service = Arc::new(AccountBackupService::new(
                accounts_service.clone(),
                services.identity.clone(),
            ));
            app.manage(services.db);
            app.manage(accounts_service);
            app.manage(account_backup_service);
            app.manage(services.identity);
            app.manage(services.contacts);
            app.manage(services.permissions);
            app.manage(services.messaging);
            app.manage(services.posts);
            app.manage(services.mentions);
            app.manage(services.content_sync);
            app.manage(services.feed);
            app.manage(services.wall_social);
            app.manage(services.calling);
            app.manage(services.boards);
            app.manage(services.media);
            app.manage(network_state);
            app.manage(StartupInitializationState(startup_failure));
            app.manage(PendingDeepLink(Mutex::new(Vec::new())));

            control::spawn_if_configured(app.handle().clone());

            // Deep-link handler: receives harbor:// URLs from the OS
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    handle_deep_link(&handle, url.as_ref());
                }
            });

            // Windows cold start: URL arrives as a command-line argument, not via on_open_url
            for arg in std::env::args().skip(1) {
                if arg.starts_with("harbor://") {
                    handle_deep_link(app.handle(), &arg);
                    break;
                }
            }

            info!("Application setup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Account commands (multi-user support)
            commands::list_accounts,
            commands::get_account,
            commands::get_active_account,
            commands::has_accounts,
            commands::set_active_account,
            commands::remove_account,
            commands::export_identity_backup,
            commands::restore_identity_backup,
            commands::delete_account_profile,
            commands::update_account_metadata,
            // Identity commands
            commands::has_identity,
            commands::is_identity_unlocked,
            commands::get_identity_info,
            commands::get_identity_initialization_state,
            commands::create_identity,
            commands::unlock_identity,
            commands::change_identity_password,
            commands::lock_identity,
            commands::update_display_name,
            commands::update_bio,
            commands::update_profile_avatar,
            commands::update_passphrase_hint,
            commands::get_peer_id,
            commands::get_identity_entry_state,
            commands::get_identity_publishing_state,
            commands::set_identity_publishing_mode,
            commands::register_relay_name,
            commands::get_local_name_claim,
            commands::verify_name_claim,
            commands::apply_relay_key_rotation,
            commands::drain_private_mention_outbox,
            // Network commands
            commands::get_connected_peers,
            commands::get_network_stats,
            commands::is_network_running,
            commands::bootstrap_network,
            commands::start_network,
            commands::stop_network,
            commands::get_listening_addresses,
            commands::connect_to_peer,
            commands::sync_feed,
            commands::add_bootstrap_node,
            commands::get_shareable_addresses,
            commands::get_shareable_contact_string,
            commands::add_contact_from_string,
            commands::add_relay_server,
            commands::connect_to_public_relays,
            commands::get_nat_status,
            // Bootstrap configuration commands
            commands::get_bootstrap_nodes,
            commands::add_bootstrap_node_config,
            commands::update_bootstrap_node,
            commands::remove_bootstrap_node,
            commands::get_enabled_bootstrap_addresses,
            // Contact commands
            commands::get_contacts,
            commands::get_active_contacts,
            commands::get_contact,
            commands::add_contact,
            commands::block_contact,
            commands::unblock_contact,
            commands::remove_contact,
            commands::is_contact,
            commands::is_contact_blocked,
            commands::request_peer_identity,
            commands::get_contact_requests,
            commands::respond_contact_request,
            commands::retry_contact_request,
            // Permission commands
            commands::grant_permission,
            commands::revoke_permission,
            commands::peer_has_capability,
            commands::we_have_capability,
            commands::get_granted_permissions,
            commands::get_received_permissions,
            commands::get_chat_peers,
            commands::grant_all_permissions,
            // Messaging commands
            commands::send_message,
            commands::get_messages,
            commands::get_conversations,
            commands::mark_conversation_read,
            commands::get_messaging_privacy_policy,
            commands::set_read_receipts_enabled,
            commands::get_unread_count,
            commands::get_total_unread_count,
            commands::clear_conversation_history,
            commands::delete_conversation,
            commands::edit_message,
            // Post commands
            commands::create_post,
            commands::update_post,
            commands::delete_post,
            commands::get_post,
            commands::get_my_posts,
            commands::get_posts_by_author,
            commands::add_post_media,
            commands::get_post_media,
            commands::resolve_private_mention,
            commands::create_post_with_mentions,
            commands::list_pending_mentions,
            commands::review_private_mention,
            // Feed commands
            commands::get_feed,
            commands::get_wall,
            commands::get_wall_preview,
            commands::get_wall_visibility_stats,
            // RSS commands
            commands::generate_rss_feed,
            commands::get_peer_rss_feed,
            commands::get_rss_feed_url,
            // Like commands
            commands::like_post,
            commands::unlike_post,
            commands::get_post_likes,
            commands::get_posts_likes_batch,
            commands::get_my_liked_posts,
            // Comment commands
            commands::add_comment,
            commands::get_comments,
            commands::delete_comment,
            commands::get_comment_counts,
            commands::get_wall_social_events,
            // Calling commands
            commands::get_active_calls,
            commands::get_call_history,
            commands::get_active_group_calls,
            commands::send_group_membership,
            commands::start_call,
            commands::answer_call,
            commands::send_ice_candidate,
            commands::hangup_call,
            commands::decline_call,
            commands::busy_call,
            commands::process_offer,
            commands::process_answer,
            commands::process_ice_candidate,
            commands::process_hangup,
            // Logging commands
            commands::export_logs,
            commands::get_log_path,
            commands::cleanup_logs,
            // Content sync commands
            commands::request_content_manifest,
            commands::request_content_manifest_with_cursor,
            commands::request_content_fetch,
            commands::get_sync_cursor,
            commands::sync_with_all_peers,
            // Board commands
            commands::get_communities,
            commands::join_community,
            commands::leave_community,
            commands::get_boards,
            commands::get_board_posts,
            commands::submit_board_post,
            commands::delete_board_post,
            commands::sync_board,
            // Media commands (content-addressed storage)
            commands::store_media,
            commands::get_media_asset,
            commands::has_media,
            commands::ensure_media_transfer,
            commands::get_media_transfer,
            commands::retry_media_transfer,
            commands::preload_missing_media,
            commands::get_media_cache_diagnostics,
            commands::update_media_cache_settings,
            // Wall sync commands (relay-based wall post sync)
            commands::sync_wall_to_relay,
            commands::fetch_contact_wall_from_relay,
            commands::sync_feed_from_relay,
            commands::sync_wall_social_events_to_relay,
            commands::fetch_wall_social_events_from_relay,
            commands::delete_wall_post_on_relay,
            // File commands
            commands::save_to_downloads,
            // Link preview commands
            commands::fetch_link_preview,
        ])
        .run(context)
        .expect("error while running tauri application");
}
