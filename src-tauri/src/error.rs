use serde::{Serialize, Serializer};
use thiserror::Error;

/// Error codes for programmatic error handling by the frontend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    // Database errors (1xxx)
    DatabaseConnection = 1001,
    DatabaseQuery = 1002,
    DatabaseMigration = 1003,

    // Cryptography errors (2xxx)
    EncryptionFailed = 2001,
    DecryptionFailed = 2002,
    SignatureFailed = 2003,
    SignatureInvalid = 2004,
    KeyGenerationFailed = 2005,
    KeyDerivationFailed = 2006,

    // Identity errors (3xxx)
    IdentityNotFound = 3001,
    IdentityLocked = 3002,
    IdentityAlreadyExists = 3003,
    PassphraseIncorrect = 3004,

    // Network errors (4xxx)
    NetworkOffline = 4001,
    PeerUnreachable = 4002,
    ConnectionFailed = 4003,
    RelayUnavailable = 4004,
    BootstrapFailed = 4005,

    // Validation errors (5xxx)
    InvalidInput = 5001,
    InvalidFormat = 5002,
    InvalidAddress = 5003,

    // Permission errors (6xxx)
    PermissionDenied = 6001,
    CapabilityMissing = 6002,

    // Resource errors (7xxx)
    NotFound = 7001,
    AlreadyExists = 7002,
    ResourceLocked = 7003,

    // Internal errors (9xxx)
    InternalError = 9001,
    SerializationError = 9002,
    IoError = 9003,
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Database error: {0}")]
    DatabaseString(String),

    #[error("Cryptography error: {0}")]
    Crypto(String),

    #[error("Identity error: {0}")]
    Identity(String),

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

    #[error("Internal error: {0}")]
    Internal(String),
}

impl AppError {
    /// Get the error code for this error type
    pub fn code(&self) -> ErrorCode {
        match self {
            AppError::Database(_) => ErrorCode::DatabaseQuery,
            AppError::DatabaseString(_) => ErrorCode::DatabaseQuery,
            AppError::Crypto(msg) => {
                if msg.contains("decrypt") {
                    ErrorCode::DecryptionFailed
                } else if msg.contains("encrypt") {
                    ErrorCode::EncryptionFailed
                } else if msg.contains("signature") || msg.contains("sign") {
                    ErrorCode::SignatureFailed
                } else {
                    ErrorCode::KeyGenerationFailed
                }
            }
            AppError::Identity(msg) => {
                if msg.contains("locked") {
                    ErrorCode::IdentityLocked
                } else if msg.contains("passphrase") || msg.contains("password") {
                    ErrorCode::PassphraseIncorrect
                } else {
                    ErrorCode::IdentityNotFound
                }
            }
            AppError::Serialization(_) => ErrorCode::SerializationError,
            AppError::Io(_) => ErrorCode::IoError,
            AppError::InvalidData(_) => ErrorCode::InvalidFormat,
            AppError::NotFound(_) => ErrorCode::NotFound,
            AppError::AlreadyExists(_) => ErrorCode::AlreadyExists,
            AppError::PermissionDenied(_) => ErrorCode::PermissionDenied,
            AppError::Unauthorized(_) => ErrorCode::PermissionDenied,
            AppError::Validation(_) => ErrorCode::InvalidInput,
            AppError::Network(msg) => {
                if msg.contains("offline") {
                    ErrorCode::NetworkOffline
                } else if msg.contains("relay") {
                    ErrorCode::RelayUnavailable
                } else if msg.contains("bootstrap") {
                    ErrorCode::BootstrapFailed
                } else {
                    ErrorCode::ConnectionFailed
                }
            }
            AppError::Internal(_) => ErrorCode::InternalError,
        }
    }

    /// Get a user-friendly message for this error
    pub fn user_message(&self) -> String {
        match self {
            AppError::Database(_) | AppError::DatabaseString(_) => {
                "A database error occurred. Please try again or restart the application.".to_string()
            }
            AppError::Crypto(msg) => {
                if msg.contains("decrypt") {
                    "Failed to decrypt message. The encryption key may have changed.".to_string()
                } else if msg.contains("passphrase") || msg.contains("password") {
                    "Incorrect passphrase. Please try again.".to_string()
                } else {
                    "A cryptographic operation failed. Please try again.".to_string()
                }
            }
            AppError::Identity(msg) => {
                if msg.contains("locked") {
                    "Your identity is locked. Please unlock it to continue.".to_string()
                } else if msg.contains("passphrase") {
                    "Incorrect passphrase. Please try again.".to_string()
                } else {
                    "Identity operation failed. Please check your settings.".to_string()
                }
            }
            AppError::Serialization(_) => {
                "Failed to process data. The format may be corrupted.".to_string()
            }
            AppError::Io(_) => {
                "A file operation failed. Please check your disk space and permissions.".to_string()
            }
            AppError::InvalidData(msg) => format!("Invalid data: {}", msg),
            AppError::NotFound(resource) => format!("{} not found.", resource),
            AppError::AlreadyExists(resource) => format!("{} already exists.", resource),
            AppError::PermissionDenied(_) => {
                "You don't have permission to perform this action.".to_string()
            }
            AppError::Unauthorized(_) => {
                "You're not authorized. Please unlock your identity.".to_string()
            }
            AppError::Validation(msg) => format!("Validation error: {}", msg),
            AppError::Network(msg) => {
                if msg.contains("not initialized") {
                    "Network is not running. Please start the network first.".to_string()
                } else {
                    "A network error occurred. Please check your connection.".to_string()
                }
            }
            AppError::Internal(_) => {
                "An unexpected error occurred. Please try again or restart the application."
                    .to_string()
            }
        }
    }

    /// Check if this error is recoverable (user can retry)
    pub fn is_recoverable(&self) -> bool {
        match self {
            AppError::Network(_) => true,
            AppError::Unauthorized(_) => true,
            AppError::PermissionDenied(_) => false,
            AppError::NotFound(_) => false,
            AppError::AlreadyExists(_) => false,
            AppError::Validation(_) => true,
            AppError::Identity(msg) => msg.contains("passphrase") || msg.contains("locked"),
            _ => false,
        }
    }
}

/// Serializable error response for the frontend
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// The error code for programmatic handling
    pub code: ErrorCode,
    /// User-friendly message to display
    pub message: String,
    /// Technical details (for logging)
    pub details: String,
    /// Whether the error is recoverable
    pub recoverable: bool,
}

impl From<&AppError> for ErrorResponse {
    fn from(err: &AppError) -> Self {
        ErrorResponse {
            code: err.code(),
            message: err.user_message(),
            details: err.to_string(),
            recoverable: err.is_recoverable(),
        }
    }
}

// Implement conversion to a serializable error for Tauri commands
// Uses the structured ErrorResponse format
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let response = ErrorResponse::from(self);
        response.serialize(serializer)
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
