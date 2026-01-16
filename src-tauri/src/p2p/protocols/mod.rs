pub mod identity_exchange;
pub mod messaging;

pub use identity_exchange::*;
pub use messaging::*;

/// Protocol version string for identity exchange
pub const IDENTITY_PROTOCOL: &str = "/chat-app/identity/1.0.0";

/// Protocol version string for direct messaging
pub const MESSAGING_PROTOCOL: &str = "/chat-app/messaging/1.0.0";

/// Protocol version string for content sync
pub const CONTENT_SYNC_PROTOCOL: &str = "/chat-app/content-sync/1.0.0";

/// Protocol version string for signaling (voice calls)
pub const SIGNALING_PROTOCOL: &str = "/chat-app/signaling/1.0.0";
