pub mod board_sync;
pub mod content_sync;
pub mod identity_exchange;
pub mod messaging;

pub use board_sync::*;
pub use content_sync::*;
pub use identity_exchange::*;
pub use messaging::*;

/// Protocol version string for identity exchange
pub const IDENTITY_PROTOCOL: &str = "/harbor/identity/1.0.0";

/// Protocol version string for direct messaging
pub const MESSAGING_PROTOCOL: &str = "/harbor/messaging/1.0.0";

/// Protocol version string for content sync
pub const CONTENT_SYNC_PROTOCOL: &str = "/harbor/content/1.0.0";

/// Protocol version string for signaling (voice calls)
pub const SIGNALING_PROTOCOL: &str = "/harbor/signaling/1.0.0";

/// Protocol version string for board sync (community boards)
pub const BOARD_SYNC_PROTOCOL: &str = "/harbor/board/1.0.0";
