use crate::commands::network::NetworkState;
use crate::{
    error::AppError,
    services::{
        MentionReceipt, MentionsService, PublishMentionedPostRequest, PublishMentionedPostResult,
        ResolvedMention,
    },
};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn resolve_private_mention(
    service: State<'_, Arc<MentionsService>>,
    network: State<'_, NetworkState>,
    qualified_name: String,
) -> Result<ResolvedMention, AppError> {
    let resolved = service.resolve(&qualified_name)?;
    if resolved.status == "unknown" {
        let handle = network.get_handle().await?;
        let relay = handle.active_relay().await?;
        let (key, expires) = handle
            .resolve_delivery_key(relay, qualified_name.clone())
            .await?;
        service.cache_delivery_key(&qualified_name, key, expires)?;
    }
    service.resolve(&qualified_name)
}
#[tauri::command]
pub fn create_post_with_mentions(
    service: State<'_, Arc<MentionsService>>,
    request: PublishMentionedPostRequest,
) -> Result<PublishMentionedPostResult, AppError> {
    service.publish(request)
}
#[tauri::command]
pub fn list_pending_mentions(
    service: State<'_, Arc<MentionsService>>,
) -> Result<Vec<MentionReceipt>, AppError> {
    service.pending()
}
#[tauri::command]
pub fn review_private_mention(
    service: State<'_, Arc<MentionsService>>,
    mention_id: String,
    decision: String,
) -> Result<(), AppError> {
    service.review(&mention_id, &decision)
}
