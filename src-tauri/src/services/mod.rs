pub mod crypto_service;
pub mod identity_service;
pub mod signing;

pub use crypto_service::CryptoService;
pub use identity_service::IdentityService;
pub use signing::{
    Signable, sign, verify,
    // Identity messages
    SignableIdentityRequest, SignableIdentityResponse,
    // Permission messages
    SignablePermissionRequest, SignablePermissionGrant, SignablePermissionRevoke,
    // Direct messages
    SignableDirectMessage, SignableMessageAck,
    // Post messages
    SignablePost, SignablePostUpdate, SignablePostDelete,
    // Signaling messages (voice calls)
    SignableSignalingOffer, SignableSignalingAnswer, SignableSignalingIce, SignableSignalingHangup,
    // Content sync
    SignableContentManifestRequest, SignableContentManifestResponse, PostSummary,
    PermissionProof,
};
