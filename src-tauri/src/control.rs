//! Opt-in loopback control plane for validation and automation.

use crate::commands;
use crate::models::CreateIdentityRequest;
use crate::services::{
    AccountsService, BoardService, CallingService, ContactsService, ContentSyncService,
    FeedService, IdentityService, MediaStorageService, MentionsService, MessagingService,
    PermissionsService, PostsService, WallSocialService,
};
use crate::{Database, PendingDeepLink};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Listener, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

use crate::commands::network::NetworkState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlRequest {
    pub id: String,
    pub token: String,
    #[serde(flatten)]
    pub command: ControlCommand,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum ControlCommand {
    Status,
    IdentityCreate {
        display_name: String,
        passphrase: String,
        bio: Option<String>,
    },
    IdentityUnlock {
        passphrase: String,
    },
    IdentityLock,
    NetworkStart,
    NetworkStop,
    NetworkPeers,
    ContactString,
    ContactAdd {
        contact_string: String,
    },
    PermissionGrantAll {
        peer_id: String,
    },
    Frontend {
        action: String,
        payload: Value,
    },
    Shutdown,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlResponse {
    pub id: String,
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
}

impl ControlResponse {
    fn success(id: String, result: Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    fn failure(id: String, error: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrontendControlEvent {
    id: String,
    action: String,
    payload: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrontendControlResult {
    id: String,
    ok: bool,
    result: Option<Value>,
    error: Option<String>,
}

type PendingFrontendRequests = Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>;
static PENDING_FRONTEND_REQUESTS: OnceLock<PendingFrontendRequests> = OnceLock::new();

pub fn spawn_if_configured(app: AppHandle) {
    let Ok(token) = std::env::var("HARBOR_CONTROL_TOKEN") else {
        return;
    };
    if token.len() < 16 {
        tracing::error!("HARBOR_CONTROL_TOKEN must contain at least 16 characters");
        return;
    }
    let port = std::env::var("HARBOR_CONTROL_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(19420);
    app.listen("harbor:control-result", |event| {
        let Ok(response) = serde_json::from_str::<FrontendControlResult>(event.payload()) else {
            tracing::warn!("Ignoring malformed frontend control response");
            return;
        };
        let result = if response.ok {
            Ok(response.result.unwrap_or_else(|| json!({})))
        } else {
            Err(response
                .error
                .unwrap_or_else(|| "frontend control action failed".to_string()))
        };
        if let Some(sender) = pending_frontend_requests()
            .lock()
            .expect("frontend request mutex poisoned")
            .remove(&response.id)
        {
            let _ = sender.send(result);
        }
    });
    tauri::async_runtime::spawn(async move {
        let address = format!("127.0.0.1:{port}");
        match TcpListener::bind(&address).await {
            Ok(listener) => {
                tracing::info!("Harbor control plane listening on {}", address);
                loop {
                    match listener.accept().await {
                        Ok((stream, _)) => {
                            let app = app.clone();
                            let token = token.clone();
                            tokio::spawn(
                                async move { handle_connection(stream, app, token).await },
                            );
                        }
                        Err(error) => tracing::warn!("Control accept failed: {}", error),
                    }
                }
            }
            Err(error) => tracing::error!("Could not bind Harbor control plane: {}", error),
        }
    });
}

async fn handle_connection(stream: TcpStream, app: AppHandle, expected_token: String) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let response = match serde_json::from_str::<ControlRequest>(&line) {
            Ok(request)
                if constant_time_eq(request.token.as_bytes(), expected_token.as_bytes()) =>
            {
                execute(request, &app).await
            }
            Ok(request) => ControlResponse::failure(request.id, "authentication failed"),
            Err(error) => ControlResponse::failure("invalid".to_string(), error.to_string()),
        };
        if let Ok(mut encoded) = serde_json::to_vec(&response) {
            encoded.push(b'\n');
            if writer.write_all(&encoded).await.is_err() {
                break;
            }
        }
    }
}

async fn execute(request: ControlRequest, app: &AppHandle) -> ControlResponse {
    let id = request.id;
    let result: Result<Value, String> = match request.command {
        ControlCommand::Status => {
            let identity = app.state::<Arc<IdentityService>>();
            let network = app.state::<NetworkState>();
            let info = identity
                .get_identity_info()
                .map_err(|error| error.to_string());
            match info {
                Ok(info) => Ok(json!({
                    "identity": info,
                    "identityUnlocked": identity.is_unlocked(),
                    "networkRunning": network.handle.read().await.is_some(),
                })),
                Err(error) => Err(error),
            }
        }
        ControlCommand::IdentityCreate {
            display_name,
            passphrase,
            bio,
        } => commands::create_identity(
            app.clone(),
            app.state::<Arc<IdentityService>>(),
            app.state::<Arc<AccountsService>>(),
            CreateIdentityRequest {
                display_name,
                passphrase,
                bio,
                passphrase_hint: None,
            },
        )
        .await
        .map(|identity| json!(identity))
        .map_err(|error| error.to_string()),
        ControlCommand::IdentityUnlock { passphrase } => {
            commands::unlock_identity(app.clone(), app.state::<Arc<IdentityService>>(), passphrase)
                .await
                .map(|identity| json!(identity))
                .map_err(|error| error.to_string())
        }
        ControlCommand::IdentityLock => {
            commands::lock_identity(app.state::<Arc<IdentityService>>())
                .await
                .map(|_| json!({}))
                .map_err(|error| error.to_string())
        }
        ControlCommand::NetworkStart => commands::start_network(
            app.clone(),
            app.state::<NetworkState>(),
            app.state::<Arc<IdentityService>>(),
            app.state::<Arc<MessagingService>>(),
            app.state::<Arc<CallingService>>(),
            app.state::<Arc<ContactsService>>(),
            app.state::<Arc<PermissionsService>>(),
            app.state::<Arc<PostsService>>(),
            app.state::<Arc<ContentSyncService>>(),
            app.state::<Arc<WallSocialService>>(),
            app.state::<Arc<BoardService>>(),
            app.state::<Arc<MediaStorageService>>(),
            app.state::<Arc<MentionsService>>(),
        )
        .await
        .map(|_| json!({}))
        .map_err(|error| error.to_string()),
        ControlCommand::NetworkStop => commands::stop_network(app.state::<NetworkState>())
            .await
            .map(|_| json!({}))
            .map_err(|error| error.to_string()),
        ControlCommand::NetworkPeers => commands::get_connected_peers(app.state::<NetworkState>())
            .await
            .map(|peers| json!(peers))
            .map_err(|error| error.to_string()),
        ControlCommand::ContactString => commands::get_shareable_contact_string(
            app.state::<NetworkState>(),
            app.state::<Arc<IdentityService>>(),
        )
        .await
        .map(|contact| json!({ "contactString": contact }))
        .map_err(|error| error.to_string()),
        ControlCommand::ContactAdd { contact_string } => commands::add_contact_from_string(
            app.state::<NetworkState>(),
            app.state::<Arc<ContactsService>>(),
            app.state::<Arc<PermissionsService>>(),
            contact_string,
        )
        .await
        .map(|peer_id| json!({ "peerId": peer_id }))
        .map_err(|error| error.to_string()),
        ControlCommand::PermissionGrantAll { peer_id } => {
            commands::grant_all_permissions(app.state::<Arc<PermissionsService>>(), peer_id)
                .await
                .map(|grants| json!(grants))
                .map_err(|error| error.to_string())
        }
        ControlCommand::Frontend { action, payload } => {
            let (sender, receiver) = oneshot::channel();
            pending_frontend_requests()
                .lock()
                .expect("frontend request mutex poisoned")
                .insert(id.clone(), sender);
            if let Err(error) = app.emit(
                "harbor:control",
                FrontendControlEvent {
                    id: id.clone(),
                    action,
                    payload,
                },
            ) {
                pending_frontend_requests()
                    .lock()
                    .expect("frontend request mutex poisoned")
                    .remove(&id);
                Err(error.to_string())
            } else {
                match tokio::time::timeout(std::time::Duration::from_secs(30), receiver).await {
                    Ok(Ok(result)) => result,
                    Ok(Err(_)) => Err("frontend control response channel closed".to_string()),
                    Err(_) => {
                        pending_frontend_requests()
                            .lock()
                            .expect("frontend request mutex poisoned")
                            .remove(&id);
                        Err("frontend control action timed out".to_string())
                    }
                }
            }
        }
        ControlCommand::Shutdown => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                app.exit(0);
            });
            Ok(json!({ "shuttingDown": true }))
        }
    };
    match result {
        Ok(value) => ControlResponse::success(id, value),
        Err(error) => ControlResponse::failure(id, error),
    }
}

fn pending_frontend_requests() -> &'static PendingFrontendRequests {
    PENDING_FRONTEND_REQUESTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_comparison_requires_equal_content_and_length() {
        assert!(constant_time_eq(
            b"a sufficiently long token",
            b"a sufficiently long token"
        ));
        assert!(!constant_time_eq(
            b"a sufficiently long token",
            b"a sufficiently long tokem"
        ));
        assert!(!constant_time_eq(b"token", b"token-extra"));
    }

    #[test]
    fn request_deserializes_flat_command_arguments() {
        let request: ControlRequest = serde_json::from_value(json!({
            "id": "request-1",
            "token": "a sufficiently long token",
            "command": "contact_add",
            "contact_string": "harbor://contact"
        }))
        .expect("control request should deserialize");

        assert_eq!(request.id, "request-1");
        assert!(matches!(
            request.command,
            ControlCommand::ContactAdd { contact_string } if contact_string == "harbor://contact"
        ));
    }
}

// Keep state types visible to rustdoc and ensure control setup stays aligned
// with application-managed state.
#[allow(dead_code)]
fn managed_state_contract(_db: Arc<Database>, _feed: Arc<FeedService>, _pending: PendingDeepLink) {}
