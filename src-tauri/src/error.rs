use thiserror::Error;

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

// Implement conversion to a serializable error for Tauri commands
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
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
