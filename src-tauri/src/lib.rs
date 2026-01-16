pub mod commands;
pub mod db;
pub mod error;
pub mod models;
pub mod p2p;
pub mod services;

use commands::NetworkState;
use db::Database;
use services::IdentityService;
use std::path::PathBuf;
use tauri::Manager;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize logging
fn init_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "chat_app_lib=debug,tauri=info".into()),
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

    app_data.join("chat-app.db")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();
    info!("Starting chat-app...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialize database
            let db_path = get_db_path(&app.handle());
            info!("Database path: {:?}", db_path);

            let db = Database::new(db_path)
                .expect("Failed to initialize database");

            // Initialize services
            let identity_service = IdentityService::new(db.clone());

            // Initialize network state (will be populated when identity is unlocked)
            let network_state = NetworkState::new();

            // Register state
            app.manage(db);
            app.manage(identity_service);
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
