use crate::error::AppError;
use serde::{Deserialize, Serialize};
use url::Url;

pub const TRUSTED_GAMES_ORIGIN: &str = "https://games.social-harbor.com";
pub(crate) const AUTH_DOMAIN: &str = "harbor.games.auth.v1";
pub(crate) const PACKAGE_DOMAIN: &str = "harbor.game-package.v1";
pub(crate) const MAX_DEEP_LINK_BYTES: usize = 8_192;
pub(crate) const MAX_REQUEST_BYTES: usize = 4_096;
pub(crate) const MAX_PENDING_REQUESTS: usize = 16;
const MAX_CLOCK_SKEW_SECONDS: i64 = 30;
const MAX_AUTH_LIFETIME_SECONDS: i64 = 300;
const MAX_PACKAGE_LIFETIME_SECONDS: i64 = 600;
const ALLOWED_PERMISSIONS: [&str; 6] = [
    "achievements",
    "analytics",
    "identity",
    "leaderboards",
    "multiplayer_signals",
    "save_data",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HarborGameAuthRequest {
    pub account_id: String,
    pub audience: String,
    pub callback_url: String,
    pub challenge_id: String,
    pub expires_at: i64,
    pub issued_at: i64,
    pub nonce: String,
    pub request_id: String,
    pub version: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HarborGamePackageSigningRequest {
    pub callback_url: String,
    pub expires_at: i64,
    pub game_id: String,
    pub issued_at: i64,
    pub package_digest: String,
    pub permissions: Vec<String>,
    pub request_id: String,
    pub title: String,
    pub version: u16,
    pub version_id: String,
}

#[derive(Debug, Clone)]
pub(crate) enum PendingSigningRequest {
    Auth(HarborGameAuthRequest),
    Package(HarborGamePackageSigningRequest),
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GameSigningRequestPresentation {
    Auth {
        approval_id: String,
        account_id: String,
        audience: String,
        challenge_id: String,
        expires_at: i64,
        request_id: String,
    },
    Package {
        approval_id: String,
        expires_at: i64,
        game_id: String,
        package_digest: String,
        permissions: Vec<String>,
        request_id: String,
        title: String,
        version_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HarborGameAuthPayload {
    pub account_id: String,
    pub audience: String,
    pub challenge_id: String,
    pub creator_peer_id: String,
    pub creator_public_key: String,
    pub domain: String,
    pub expires_at: i64,
    pub issued_at: i64,
    pub nonce: String,
    pub request_id: String,
    pub version: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ed25519SignatureValue {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarborGameAuthProof {
    pub format: String,
    pub payload: HarborGameAuthPayload,
    pub signature: Ed25519SignatureValue,
    pub version: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HarborGamePackageSigningPayload {
    pub creator_peer_id: String,
    pub creator_public_key: String,
    pub domain: String,
    pub game_id: String,
    pub issued_at: i64,
    pub package_digest: String,
    pub permissions: Vec<String>,
    pub request_id: String,
    pub title: String,
    pub version: u16,
    pub version_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarborGameCreatorSignatureEnvelope {
    pub format: String,
    pub payload: HarborGamePackageSigningPayload,
    pub signature: Ed25519SignatureValue,
    pub version: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GameSigningDelivery {
    Delivered { request_id: String },
    Pending { request_id: String, error: String },
}

impl PendingSigningRequest {
    pub(crate) fn expires_at(&self) -> i64 {
        match self {
            Self::Auth(request) => request.expires_at,
            Self::Package(request) => request.expires_at,
        }
    }

    pub(crate) fn validate(&self, now: i64) -> Result<(), AppError> {
        match self {
            Self::Auth(request) => validate_auth_request(request, now),
            Self::Package(request) => validate_package_request(request, now),
        }
    }

    pub(crate) fn presentation(&self, approval_id: String) -> GameSigningRequestPresentation {
        match self {
            Self::Auth(request) => GameSigningRequestPresentation::Auth {
                approval_id,
                account_id: request.account_id.clone(),
                audience: request.audience.clone(),
                challenge_id: request.challenge_id.clone(),
                expires_at: request.expires_at,
                request_id: request.request_id.clone(),
            },
            Self::Package(request) => GameSigningRequestPresentation::Package {
                approval_id,
                expires_at: request.expires_at,
                game_id: request.game_id.clone(),
                package_digest: request.package_digest.clone(),
                permissions: request.permissions.clone(),
                request_id: request.request_id.clone(),
                title: request.title.clone(),
                version_id: request.version_id.clone(),
            },
        }
    }
}

pub(crate) fn validate_auth_request(
    request: &HarborGameAuthRequest,
    now: i64,
) -> Result<(), AppError> {
    if request.version != 1 || request.audience != TRUSTED_GAMES_ORIGIN {
        return Err(validation(
            "Harbor authentication request has an unsupported version or audience",
        ));
    }
    validate_identifier("account ID", &request.account_id, 128)?;
    validate_identifier("challenge ID", &request.challenge_id, 128)?;
    validate_identifier("request ID", &request.request_id, 128)?;
    validate_hex("challenge nonce", &request.nonce, 64)?;
    validate_times(
        request.issued_at,
        request.expires_at,
        now,
        MAX_AUTH_LIFETIME_SECONDS,
    )?;
    validate_callback(
        &request.callback_url,
        &format!("/api/auth/harbor/challenges/{}/proof", request.challenge_id),
    )
}

pub(crate) fn validate_package_request(
    request: &HarborGamePackageSigningRequest,
    now: i64,
) -> Result<(), AppError> {
    if request.version != 1 {
        return Err(validation(
            "Package signing request has an unsupported version",
        ));
    }
    validate_identifier("game ID", &request.game_id, 128)?;
    validate_identifier("version ID", &request.version_id, 128)?;
    validate_identifier("request ID", &request.request_id, 128)?;
    validate_text("title", &request.title, 200)?;
    validate_hex("package digest", &request.package_digest, 64)?;
    if request.permissions.len() > ALLOWED_PERMISSIONS.len()
        || request
            .permissions
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request
            .permissions
            .iter()
            .any(|permission| !ALLOWED_PERMISSIONS.contains(&permission.as_str()))
    {
        return Err(validation(
            "Package permissions must be supported, unique, and sorted",
        ));
    }
    validate_times(
        request.issued_at,
        request.expires_at,
        now,
        MAX_PACKAGE_LIFETIME_SECONDS,
    )?;
    validate_callback(
        &request.callback_url,
        &format!(
            "/api/harbor-store/signing-requests/{}/proof",
            request.request_id
        ),
    )
}

fn validate_times(
    issued_at: i64,
    expires_at: i64,
    now: i64,
    maximum_lifetime: i64,
) -> Result<(), AppError> {
    if issued_at > now.saturating_add(MAX_CLOCK_SKEW_SECONDS)
        || expires_at <= now
        || expires_at <= issued_at
        || expires_at.saturating_sub(issued_at) > maximum_lifetime
    {
        return Err(validation(
            "Game signing request is expired or has an invalid lifetime",
        ));
    }
    Ok(())
}

fn validate_callback(callback: &str, expected_path: &str) -> Result<(), AppError> {
    let parsed = Url::parse(callback).map_err(|_| validation("Malformed proof callback URL"))?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("games.social-harbor.com")
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.path() != expected_path
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(validation(
            "Proof callback must use the exact trusted Neo Grounds endpoint",
        ));
    }
    Ok(())
}

pub(crate) fn validate_identifier(
    label: &str,
    value: &str,
    maximum_bytes: usize,
) -> Result<(), AppError> {
    if value.is_empty()
        || value.len() > maximum_bytes
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(validation(&format!("Invalid {label}")));
    }
    Ok(())
}

fn validate_text(label: &str, value: &str, maximum_bytes: usize) -> Result<(), AppError> {
    if value.trim().is_empty() || value.len() > maximum_bytes || value.chars().any(char::is_control)
    {
        return Err(validation(&format!("Invalid {label}")));
    }
    Ok(())
}

fn validate_hex(label: &str, value: &str, expected_length: usize) -> Result<(), AppError> {
    if value.len() != expected_length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(validation(&format!("Invalid {label}")));
    }
    Ok(())
}

pub(crate) fn validation(message: &str) -> AppError {
    AppError::Validation(message.into())
}
