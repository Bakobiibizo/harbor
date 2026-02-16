use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    DatabaseError,
    DatabaseConnection,
    DatabaseMigration,
    CryptoError,
    CryptoKeyGeneration,
    CryptoEncryption,
    CryptoDecryption,
    IdentityError,
    IdentityNotFound,
    IdentityLocked,
    IdentityInvalidPassphrase,
    SerializationError,
    IoError,
    InvalidData,
    NotFound,
    AlreadyExists,
    PermissionDenied,
    Unauthorized,
    ValidationError,
    NetworkError,
    NetworkConnectionFailed,
    NetworkNotInitialized,
    NetworkServiceUnavailable,
    NetworkPeerUnreachable,
    NetworkTimeout,
    InternalError,
}

impl ErrorCode {
    pub fn user_message(&self) -> &'static str {
        match self {
            ErrorCode::DatabaseError => "A database error occurred",
            ErrorCode::DatabaseConnection => "Could not connect to the database",
            ErrorCode::DatabaseMigration => "Database migration failed",
            ErrorCode::CryptoError => "A cryptographic operation failed",
            ErrorCode::CryptoKeyGeneration => "Failed to generate encryption keys",
            ErrorCode::CryptoEncryption => "Failed to encrypt data",
            ErrorCode::CryptoDecryption => "Failed to decrypt data",
            ErrorCode::IdentityError => "An identity error occurred",
            ErrorCode::IdentityNotFound => "No identity found. Please create one first",
            ErrorCode::IdentityLocked => "Identity is locked. Please unlock it first",
            ErrorCode::IdentityInvalidPassphrase => "Invalid passphrase",
            ErrorCode::SerializationError => "Failed to process data",
            ErrorCode::IoError => "A file operation failed",
            ErrorCode::InvalidData => "The data provided is invalid",
            ErrorCode::NotFound => "The requested item was not found",
            ErrorCode::AlreadyExists => "This item already exists",
            ErrorCode::PermissionDenied => "You don't have permission for this action",
            ErrorCode::Unauthorized => "Authentication required",
            ErrorCode::ValidationError => "Please check your input",
            ErrorCode::NetworkError => "A network error occurred",
            ErrorCode::NetworkConnectionFailed => "Failed to connect to the network",
            ErrorCode::NetworkNotInitialized => "Network has not been started",
            ErrorCode::NetworkServiceUnavailable => "Network service is unavailable",
            ErrorCode::NetworkPeerUnreachable => "Could not reach the peer",
            ErrorCode::NetworkTimeout => "The connection timed out",
            ErrorCode::InternalError => "An unexpected error occurred",
        }
    }

    pub fn recovery_suggestion(&self) -> Option<&'static str> {
        match self {
            ErrorCode::DatabaseConnection => Some("Try restarting the application"),
            ErrorCode::IdentityLocked => Some("Go to Settings and unlock your identity"),
            ErrorCode::IdentityInvalidPassphrase => Some("Check your passphrase and try again"),
            ErrorCode::NetworkConnectionFailed => {
                Some("Check your internet connection and try again")
            }
            ErrorCode::NetworkNotInitialized => {
                Some("Start the network from the Network page first")
            }
            ErrorCode::NetworkServiceUnavailable => {
                Some("Try restarting the network or the application")
            }
            ErrorCode::NetworkPeerUnreachable => Some("The peer may be offline. Try again later"),
            ErrorCode::NetworkTimeout => Some("Try again or check your connection"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<String>,
}

impl ErrorResponse {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
            recovery: code.recovery_suggestion().map(String::from),
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Database error: {0}")]
    DatabaseString(String),

    #[error("Cryptography error: {0}")]
    Crypto(String),

    #[error("Encryption error: {0}")]
    CryptoEncryption(String),

    #[error("Decryption error: {0}")]
    CryptoDecryption(String),

    #[error("Identity error: {0}")]
    IdentityLocked(String),

    #[error("Identity error: {0}")]
    IdentityNotFound(String),

    #[error("Identity error: {0}")]
    IdentityInvalidPassphrase(String),

    #[error("Identity error: {0}")]
    IdentityGeneric(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Network error: {0}")]
    NetworkConnectionFailed(String),

    #[error("Network not initialized: {0}")]
    NetworkNotInitialized(String),

    #[error("Network service unavailable: {0}")]
    NetworkServiceUnavailable(String),

    #[error("Network error: {0}")]
    NetworkPeerUnreachable(String),

    #[error("Network error: {0}")]
    NetworkTimeout(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn error_code(&self) -> ErrorCode {
        match self {
            AppError::Database(_) => ErrorCode::DatabaseError,
            AppError::DatabaseString(_) => ErrorCode::DatabaseError,
            AppError::Crypto(_) => ErrorCode::CryptoError,
            AppError::CryptoEncryption(_) => ErrorCode::CryptoEncryption,
            AppError::CryptoDecryption(_) => ErrorCode::CryptoDecryption,
            AppError::IdentityLocked(_) => ErrorCode::IdentityLocked,
            AppError::IdentityNotFound(_) => ErrorCode::IdentityNotFound,
            AppError::IdentityInvalidPassphrase(_) => ErrorCode::IdentityInvalidPassphrase,
            AppError::IdentityGeneric(_) => ErrorCode::IdentityError,
            AppError::Serialization(_) => ErrorCode::SerializationError,
            AppError::Io(_) => ErrorCode::IoError,
            AppError::InvalidData(_) => ErrorCode::InvalidData,
            AppError::NotFound(_) => ErrorCode::NotFound,
            AppError::AlreadyExists(_) => ErrorCode::AlreadyExists,
            AppError::PermissionDenied(_) => ErrorCode::PermissionDenied,
            AppError::Unauthorized(_) => ErrorCode::Unauthorized,
            AppError::Validation(_) => ErrorCode::ValidationError,
            AppError::Network(_) => ErrorCode::NetworkError,
            AppError::NetworkConnectionFailed(_) => ErrorCode::NetworkConnectionFailed,
            AppError::NetworkNotInitialized(_) => ErrorCode::NetworkNotInitialized,
            AppError::NetworkServiceUnavailable(_) => ErrorCode::NetworkServiceUnavailable,
            AppError::NetworkPeerUnreachable(_) => ErrorCode::NetworkPeerUnreachable,
            AppError::NetworkTimeout(_) => ErrorCode::NetworkTimeout,
            AppError::Internal(_) => ErrorCode::InternalError,
        }
    }

    pub fn to_response(&self) -> ErrorResponse {
        let code = self.error_code();
        ErrorResponse::new(code, code.user_message()).with_details(self.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_response().serialize(serializer)
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

// Implement From for libp2p transport error
impl From<libp2p::TransportError<std::io::Error>> for AppError {
    fn from(err: libp2p::TransportError<std::io::Error>) -> Self {
        AppError::Network(err.to_string())
    }
}

// Implement From for libp2p dial error
impl From<libp2p::swarm::DialError> for AppError {
    fn from(err: libp2p::swarm::DialError) -> Self {
        AppError::Network(err.to_string())
    }
}
