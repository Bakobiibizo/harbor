pub mod commands;
pub mod db;
pub mod error;
pub mod models;
pub mod p2p;
pub mod services;

use commands::NetworkState;
use db::Database;
use services::{ContactsService, IdentityService, MessagingService, PermissionsService};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize logging
fn init_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "harbor_lib=debug,tauri=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Get the database path for the application
fn get_db_path(app: &tauri::AppHandle) -> PathBuf {
    let app_data = app
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory");

    app_data.join("harbor.db")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();
    info!("Starting Harbor...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialize database
            let db_path = get_db_path(&app.handle());
            info!("Database path: {:?}", db_path);

            let db = Arc::new(Database::new(db_path)
                .expect("Failed to initialize database"));

            // Initialize services
            let identity_service = Arc::new(IdentityService::new(db.clone()));
            let contacts_service = Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
            let permissions_service = Arc::new(PermissionsService::new(db.clone(), identity_service.clone()));
            let messaging_service = Arc::new(MessagingService::new(
                db.clone(),
                identity_service.clone(),
                contacts_service.clone(),
                permissions_service.clone(),
            ));

            // Initialize network state (will be populated when identity is unlocked)
            let network_state = NetworkState::new();

            // Register state
            app.manage(db);
            app.manage(identity_service);
            app.manage(contacts_service);
            app.manage(permissions_service);
            app.manage(messaging_service);
            app.manage(network_state);

            info!("Application setup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Identity commands
            commands::has_identity,
            commands::is_identity_unlocked,
            commands::get_identity_info,
            commands::create_identity,
            commands::unlock_identity,
            commands::lock_identity,
            commands::update_display_name,
            commands::update_bio,
            commands::get_peer_id,
            // Network commands
            commands::get_connected_peers,
            commands::get_network_stats,
            commands::is_network_running,
            commands::bootstrap_network,
            commands::start_network,
            commands::stop_network,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
