use crate::{
    error::AppError,
    services::{
        verify_game_package, verify_store_approval, GameDiscoveryResult, GameInstallation,
        GameLibraryService, StoreApproval, VerifiedGamePackage, MAX_HARBOR_GAME_BYTES,
    },
};
use futures::StreamExt;
use serde::Deserialize;
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
    creator_peer_id: String,
    game_id: String,
    package_digest: String,
    permissions: Vec<String>,
    version_id: String,
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
pub async fn install_store_game(
    game_id: String,
    version_id: String,
    approved_permissions: Vec<String>,
    service: State<'_, Arc<GameLibraryService>>,
) -> Result<GameInstallation, AppError> {
    validate_identifier(&game_id, "game ID")?;
    validate_identifier(&version_id, "version ID")?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| {
            AppError::Network(format!("Could not initialize store client: {error}"))
        })?;
    let metadata_url =
        format!("{STORE_ORIGIN}/api/harbor-store/games/{game_id}/versions/{version_id}");
    let metadata_response =
        client.get(metadata_url).send().await.map_err(|error| {
            AppError::Network(format!("Could not load store metadata: {error}"))
        })?;
    if !metadata_response.status().is_success() {
        return Err(AppError::Network(format!(
            "Store metadata request failed with HTTP {}",
            metadata_response.status()
        )));
    }
    let metadata_bytes = read_bounded_response(metadata_response, MAX_METADATA_BYTES).await?;
    let response: ApiSuccess<StoreGameMetadata> = serde_json::from_slice(&metadata_bytes)
        .map_err(|_| AppError::InvalidData("Store metadata response is malformed".into()))?;
    if !response.ok {
        return Err(AppError::InvalidData(
            "Store metadata response did not report success".into(),
        ));
    }
    let metadata = response.data;
    if metadata.game_id != game_id
        || metadata.version_id != version_id
        || metadata.approval.payload.archive_digest != metadata.archive_digest
        || metadata.approval.payload.creator_peer_id != metadata.creator_peer_id
        || metadata.package_digest.len() != 64
        || metadata.permissions != approved_permissions
    {
        return Err(AppError::InvalidData(
            "Store metadata does not match the requested game or approved permissions".into(),
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
