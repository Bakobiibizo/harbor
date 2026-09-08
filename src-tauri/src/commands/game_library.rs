use crate::{
    error::AppError,
    services::{
        verify_game_package, verify_store_approval, GameDiscoveryPreview, GameDiscoveryResult,
        GameInstallation, GameLibraryService, GameRuntimeBundle, StoreApproval,
        VerifiedGamePackage, MAX_HARBOR_GAME_BYTES,
    },
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tauri::State;

const STORE_ORIGIN: &str = "https://games.social-harbor.com";
const MAX_METADATA_BYTES: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
struct ApiSuccess<T> {
    ok: bool,
    data: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoreGameMetadata {
    approval: StoreApproval,
    archive_digest: String,
    byte_length: usize,
    creator_peer_id: String,
    game_id: String,
    package_digest: String,
    permissions: Vec<String>,
    title: String,
    version_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreGamePreview {
    pub archive_digest: String,
    pub byte_length: usize,
    pub creator_peer_id: String,
    pub game_id: String,
    pub package_digest: String,
    pub permissions: Vec<String>,
    pub title: String,
    pub version_id: String,
}

#[tauri::command]
pub async fn inspect_game_package(
    file_path: String,
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<VerifiedGamePackage, AppError> {
    let service = service.inner().clone();
    tokio::task::spawn_blocking(move || service.inspect_package(&PathBuf::from(file_path)))
        .await
        .map_err(|error| AppError::Internal(format!("Game inspection worker failed: {error}")))?
}

#[tauri::command]
pub async fn import_game_package(
    file_path: String,
    approved_permissions: Vec<String>,
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<GameInstallation, AppError> {
    let service = service.inner().clone();
    tokio::task::spawn_blocking(move || {
        service.import_manual(
            &PathBuf::from(file_path),
            &approved_permissions,
            chrono::Utc::now().timestamp(),
        )
    })
    .await
    .map_err(|error| AppError::Internal(format!("Game import worker failed: {error}")))?
}

#[tauri::command]
pub async fn configure_game_discovery_folder(
    folder_path: String,
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<(), AppError> {
    service.configure_discovery_folder(&PathBuf::from(folder_path))
}

#[tauri::command]
pub async fn get_game_discovery_folder(
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<Option<String>, AppError> {
    Ok(service
        .configured_discovery_folder()?
        .map(|path| path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn inspect_discovered_game_packages(
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<Vec<GameDiscoveryPreview>, AppError> {
    let service = service.inner().clone();
    tokio::task::spawn_blocking(move || service.inspect_discovery_folder())
        .await
        .map_err(|error| AppError::Internal(format!("Game discovery worker failed: {error}")))?
}

#[tauri::command]
pub async fn discover_game_packages(
    approved_permissions: Vec<String>,
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<Vec<GameDiscoveryResult>, AppError> {
    let service = service.inner().clone();
    tokio::task::spawn_blocking(move || {
        service.discover_and_install(&approved_permissions, chrono::Utc::now().timestamp())
    })
    .await
    .map_err(|error| AppError::Internal(format!("Game discovery worker failed: {error}")))?
}

#[tauri::command]
pub async fn get_store_game_metadata(
    game_id: String,
    version_id: String,
) -> Result<StoreGamePreview, AppError> {
    let client = store_client()?;
    let metadata = fetch_store_metadata(&client, &game_id, &version_id).await?;
    Ok(StoreGamePreview {
        archive_digest: metadata.archive_digest,
        byte_length: metadata.byte_length,
        creator_peer_id: metadata.creator_peer_id,
        game_id: metadata.game_id,
        package_digest: metadata.package_digest,
        permissions: metadata.permissions,
        title: metadata.title,
        version_id: metadata.version_id,
    })
}

#[tauri::command]
pub async fn install_store_game(
    game_id: String,
    version_id: String,
    approved_permissions: Vec<String>,
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<GameInstallation, AppError> {
    let client = store_client()?;
    let metadata = fetch_store_metadata(&client, &game_id, &version_id).await?;
    if metadata.permissions != approved_permissions {
        return Err(AppError::InvalidData(
            "Store permissions changed after user confirmation".into(),
        ));
    }
    let package_url =
        format!("{STORE_ORIGIN}/api/harbor-store/games/{game_id}/versions/{version_id}/package");
    let package_response =
        client.get(package_url).send().await.map_err(|error| {
            AppError::Network(format!("Could not download store package: {error}"))
        })?;
    if !package_response.status().is_success() {
        return Err(AppError::Network(format!(
            "Store package request failed with HTTP {}",
            package_response.status()
        )));
    }
    let package_bytes = read_bounded_response(package_response, MAX_HARBOR_GAME_BYTES).await?;
    let service = service.inner().clone();
    tokio::task::spawn_blocking(move || {
        let verified = verify_game_package(&package_bytes)?;
        if verified.archive_digest != metadata.archive_digest
            || verified.package_digest != metadata.package_digest
            || verified.creator_peer_id != metadata.creator_peer_id
            || verified.title != metadata.title
            || verified.permissions != metadata.permissions
        {
            return Err(AppError::InvalidData(
                "Downloaded package does not match independently resolved store metadata".into(),
            ));
        }
        verify_store_approval(&metadata.approval, &verified, &metadata.approval.public_key)?;
        service.trust_store_public_key(&metadata.approval.public_key)?;
        service.install_store_package(
            &package_bytes,
            &approved_permissions,
            metadata.approval.clone(),
            &metadata.approval.public_key,
            chrono::Utc::now().timestamp(),
        )
    })
    .await
    .map_err(|error| AppError::Internal(format!("Game installation worker failed: {error}")))?
}

#[tauri::command]
pub async fn load_game_runtime(
    game_id: String,
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<GameRuntimeBundle, AppError> {
    let service = service.inner().clone();
    tokio::task::spawn_blocking(move || service.load_runtime_bundle(&game_id))
        .await
        .map_err(|error| AppError::Internal(format!("Game runtime load worker failed: {error}")))?
}

#[tauri::command]
pub async fn record_game_launch(
    game_id: String,
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<(), AppError> {
    service.record_launch(&game_id, chrono::Utc::now().timestamp())
}

#[tauri::command]
pub async fn list_installed_games(
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<Vec<GameInstallation>, AppError> {
    service.list()
}

#[tauri::command]
pub async fn uninstall_game(
    game_id: String,
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<(), AppError> {
    service.uninstall(&game_id)
}

#[tauri::command]
pub async fn read_game_save(
    game_id: String,
    slot: String,
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<Option<Vec<u8>>, AppError> {
    service.read_save(&game_id, &slot)
}

#[tauri::command]
pub async fn write_game_save(
    game_id: String,
    slot: String,
    data: Vec<u8>,
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<(), AppError> {
    service.write_save(&game_id, &slot, &data, chrono::Utc::now().timestamp())
}

#[tauri::command]
pub async fn delete_game_saves(
    game_id: String,
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<usize, AppError> {
    service.delete_saves(&game_id)
}

fn store_client() -> Result<reqwest::Client, AppError> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| AppError::Network(format!("Could not initialize store client: {error}")))
}

async fn fetch_store_metadata(
    client: &reqwest::Client,
    game_id: &str,
    version_id: &str,
) -> Result<StoreGameMetadata, AppError> {
    validate_identifier(game_id, "game ID")?;
    validate_identifier(version_id, "version ID")?;
    let metadata_url =
        format!("{STORE_ORIGIN}/api/harbor-store/games/{game_id}/versions/{version_id}");
    let response =
        client.get(metadata_url).send().await.map_err(|error| {
            AppError::Network(format!("Could not load store metadata: {error}"))
        })?;
    if !response.status().is_success() {
        return Err(AppError::Network(format!(
            "Store metadata request failed with HTTP {}",
            response.status()
        )));
    }
    let bytes = read_bounded_response(response, MAX_METADATA_BYTES).await?;
    let response: ApiSuccess<StoreGameMetadata> = serde_json::from_slice(&bytes)
        .map_err(|_| AppError::InvalidData("Store metadata response is malformed".into()))?;
    let metadata = response.data;
    let allowed_permissions = [
        "achievements",
        "analytics",
        "identity",
        "leaderboards",
        "multiplayer_signals",
        "save_data",
    ];
    if !response.ok
        || metadata.game_id != game_id
        || metadata.version_id != version_id
        || metadata.approval.payload.archive_digest != metadata.archive_digest
        || metadata.approval.payload.creator_peer_id != metadata.creator_peer_id
        || metadata.approval.payload.game_id != metadata.game_id
        || metadata.approval.payload.version_id != metadata.version_id
        || metadata.byte_length == 0
        || metadata.byte_length > MAX_HARBOR_GAME_BYTES
        || metadata.title.trim().is_empty()
        || metadata.title.len() > 200
        || !is_hash(&metadata.archive_digest)
        || !is_hash(&metadata.package_digest)
        || metadata
            .permissions
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || metadata
            .permissions
            .iter()
            .any(|permission| !allowed_permissions.contains(&permission.as_str()))
    {
        return Err(AppError::InvalidData(
            "Store metadata does not match the requested approved game".into(),
        ));
    }
    Ok(metadata)
}

async fn read_bounded_response(
    response: reqwest::Response,
    maximum_bytes: usize,
) -> Result<Vec<u8>, AppError> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum_bytes as u64)
    {
        return Err(AppError::InvalidData(
            "Store response exceeds the allowed size".into(),
        ));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|error| AppError::Network(format!("Store response failed: {error}")))?;
        if bytes.len().saturating_add(chunk.len()) > maximum_bytes {
            return Err(AppError::InvalidData(
                "Store response exceeds the allowed size".into(),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_identifier(value: &str, label: &str) -> Result<(), AppError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(AppError::Validation(format!("Invalid {label}")));
    }
    Ok(())
}
