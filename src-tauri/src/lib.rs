pub mod commands;
pub mod db;
pub mod error;
pub mod logging;
pub mod models;
pub mod p2p;
pub mod services;

use commands::NetworkState;
use db::Database;
use logging::{get_log_directory, LogConfig};
use services::{
    AccountsService, CallingService, ContactsService, ContentSyncService, FeedService,
    IdentityService, MessagingService, PermissionsService, PostsService,
};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use tracing::info;

pub struct LogDirectory(pub PathBuf);

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

/// Get the database path for the application
fn get_db_path(app: &tauri::AppHandle) -> PathBuf {
    // Check for custom data directory first
    let base_dir = if let Some(custom_dir) = get_custom_data_dir() {
        custom_dir
    } else {
        let app_data = app
            .path()
            .app_data_dir()
            .expect("Failed to get app data directory");

        // If a profile is specified, use a subdirectory for that profile
        if let Some(profile) = get_profile_name() {
            app_data.join(format!("profile-{}", profile))
        } else {
            app_data
        }
    };

    // Ensure the directory exists
    std::fs::create_dir_all(&base_dir).expect("Failed to create data directory");

    base_dir.join("harbor.db")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let profile = get_profile_name();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(move |app| {
            // Get app data directory first so we can set up logging properly
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            // Set up log directory
            let log_dir = get_log_directory(&app_data_dir);

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

            app.manage(LogDirectory(log_dir));

            // Initialize accounts service (manages multi-account registry)
            let accounts_service = Arc::new(AccountsService::new(app_data_dir.clone()));

            // Initialize database
            let db_path = get_db_path(app.handle());
            info!("Database path: {:?}", db_path);

            // Migrate legacy single-account setup if needed
            if let Ok(Some(account)) = accounts_service.migrate_legacy_account(&db_path) {
                info!("Migrated legacy account: {}", account.display_name);
            }

            let db = Arc::new(Database::new(db_path).expect("Failed to initialize database"));

            // Initialize services
            let identity_service = Arc::new(IdentityService::new(db.clone()));
            let contacts_service =
                Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
            let permissions_service = Arc::new(PermissionsService::new(
                db.clone(),
                identity_service.clone(),
            ));
            let messaging_service = Arc::new(MessagingService::new(
                db.clone(),
                identity_service.clone(),
                contacts_service.clone(),
                permissions_service.clone(),
            ));
            let posts_service = Arc::new(PostsService::new(
                db.clone(),
                identity_service.clone(),
                contacts_service.clone(),
                permissions_service.clone(),
            ));
            let feed_service = Arc::new(FeedService::new(
                db.clone(),
                identity_service.clone(),
                permissions_service.clone(),
                contacts_service.clone(),
            ));
            let calling_service = Arc::new(CallingService::new(
                db.clone(),
                identity_service.clone(),
                contacts_service.clone(),
                permissions_service.clone(),
            ));
            let content_sync_service = Arc::new(ContentSyncService::new(
                db.clone(),
                identity_service.clone(),
                contacts_service.clone(),
                permissions_service.clone(),
            ));

            // Initialize network state (will be populated when identity is unlocked)
            let network_state = NetworkState::new();

            // Register state
            app.manage(db);
            app.manage(accounts_service);
            app.manage(identity_service);
            app.manage(contacts_service);
            app.manage(permissions_service);
            app.manage(messaging_service);
            app.manage(posts_service);
            app.manage(content_sync_service);
            app.manage(feed_service);
            app.manage(calling_service);
            app.manage(network_state);

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
            commands::update_account_metadata,
            // Identity commands
            commands::has_identity,
            commands::is_identity_unlocked,
            commands::get_identity_info,
            commands::create_identity,
            commands::unlock_identity,
            commands::lock_identity,
            commands::update_display_name,
            commands::update_bio,
            commands::update_passphrase_hint,
            commands::get_peer_id,
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
            commands::get_unread_count,
            commands::get_total_unread_count,
            // Post commands
            commands::create_post,
            commands::update_post,
            commands::delete_post,
            commands::get_post,
            commands::get_my_posts,
            commands::get_posts_by_author,
            commands::add_post_media,
            commands::get_post_media,
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
            // Calling commands
            commands::start_call,
            commands::answer_call,
            commands::send_ice_candidate,
            commands::hangup_call,
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
            // File commands
            commands::save_to_downloads,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
