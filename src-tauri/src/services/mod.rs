pub mod accounts_service;
pub mod board_service;
pub mod calling_service;
pub mod contacts_service;
pub mod content_sync_service;
pub mod crypto_service;
pub mod feed_service;
pub mod identity_service;
pub mod media_service;
pub mod messaging_service;
pub mod permissions_service;
pub mod posts_service;
pub mod signing;

pub use accounts_service::AccountsService;
pub use board_service::BoardService;
pub use calling_service::{
    Call, CallState, CallingService, OutgoingAnswer, OutgoingHangup, OutgoingIce, OutgoingOffer,
};
pub use contacts_service::ContactsService;
pub use content_sync_service::{
    ContentSyncService, OutgoingManifestRequest, OutgoingManifestResponse,
};
pub use crypto_service::CryptoService;
pub use feed_service::{FeedItem, FeedService};
pub use identity_service::IdentityService;
pub use media_service::MediaStorageService;
pub use messaging_service::{DecryptedMessage, MessagingService, OutgoingMessage};
pub use permissions_service::{
    PermissionGrantMessage, PermissionRequestMessage, PermissionRevokeMessage, PermissionsService,
};
pub use posts_service::{OutgoingPost, OutgoingPostDelete, OutgoingPostUpdate, PostsService};
pub use signing::{
    sign,
    verify,
    PermissionProof,
    PostSummary,
    Signable,
    // Board messages
    SignableBoardListRequest,
    SignableBoardPost,
    SignableBoardPostDelete,
    SignableBoardPostsRequest,
    // Content sync
    SignableContentManifestRequest,
    SignableContentManifestResponse,
    // Direct messages
    SignableDirectMessage,
    // Wall post relay sync
    SignableGetWallPosts,
    // Identity messages
    SignableIdentityRequest,
    SignableIdentityResponse,
    SignableMessageAck,
    SignablePeerRegistration,
    SignablePermissionGrant,
    // Permission messages
    SignablePermissionRequest,
    SignablePermissionRevoke,
    // Post messages
    SignablePost,
    SignablePostDelete,
    SignablePostUpdate,
    SignableSignalingAnswer,
    SignableSignalingHangup,
    SignableSignalingIce,
    // Signaling messages (voice calls)
    SignableSignalingOffer,
    SignableWallPostDelete,
    SignableWallPostSubmit,
};
