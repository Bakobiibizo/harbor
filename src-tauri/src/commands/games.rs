use crate::{
    error::AppError,
    services::{GameSigningDelivery, GameSigningService},
};
use std::sync::Arc;
use tauri::State;

/// Approve one backend-validated deep-link request. The command accepts only an
/// opaque pending approval ID; it cannot sign caller-provided bytes or payloads.
#[tauri::command]
pub async fn approve_game_signing_request(
    game_signing_service: State<'_, Arc<GameSigningService>>,
    approval_id: String,
) -> Result<GameSigningDelivery, AppError> {
    game_signing_service
        .approve(&approval_id, chrono::Utc::now().timestamp())
        .await
}
