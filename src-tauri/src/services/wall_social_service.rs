//! Service for signed, syncable wall comments and reactions.

use std::sync::Arc;

use ed25519_dalek::VerifyingKey;
use uuid::Uuid;

use crate::db::repositories::{LikeData, LikesRepository};
use crate::db::{
    Capability, CommentData, CommentsRepository, Database, PostComment, PostVisibility,
    PostsRepository, WallSocialEvent, WallSocialEventData, WallSocialEventType,
    WallSocialEventsRepository,
};
use crate::error::{AppError, Result};
use crate::services::{
    verify, ContactsService, IdentityService, PermissionsService, Signable,
    SignableWallCommentCreate, SignableWallCommentDelete, SignableWallReactionAdd,
    SignableWallReactionRemove,
};

pub struct WallSocialService {
    db: Arc<Database>,
    identity_service: Arc<IdentityService>,
    contacts_service: Arc<ContactsService>,
    permissions_service: Arc<PermissionsService>,
}

pub struct IncomingWallSocialEventParams<'a> {
    pub event_id: &'a str,
    pub event_type: WallSocialEventType,
    pub post_id: &'a str,
    pub actor_peer_id: &'a str,
    pub author_name: Option<&'a str>,
    pub comment_id: Option<&'a str>,
    pub content: Option<&'a str>,
    pub reaction_type: Option<&'a str>,
    pub timestamp: i64,
    pub signature: &'a [u8],
}

impl WallSocialService {
    pub fn new(
        db: Arc<Database>,
        identity_service: Arc<IdentityService>,
        contacts_service: Arc<ContactsService>,
        permissions_service: Arc<PermissionsService>,
    ) -> Self {
        Self {
            db,
            identity_service,
            contacts_service,
            permissions_service,
        }
    }

    pub fn add_comment(&self, post_id: &str, content: &str) -> Result<PostComment> {
        crate::services::IdentityPublishingPolicy::enforce(&self.db, &self.identity_service)?;
        let content = content.trim();
        if content.is_empty() {
            return Err(AppError::Validation(
                "Comment content cannot be empty".to_string(),
            ));
        }
        self.ensure_current_user_can_read_post(post_id)?;
        let identity = self.current_identity()?;
        let event_id = Uuid::new_v4().to_string();
        let comment_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();
        let payload = SignableWallCommentCreate {
            event_id: event_id.clone(),
            post_id: post_id.to_string(),
            comment_id: comment_id.clone(),
            actor_peer_id: identity.peer_id.clone(),
            author_name: identity.display_name.clone(),
            content: content.to_string(),
            timestamp,
        };
        let signature = self.identity_service.sign(&payload)?;
        let payload_cbor = canonical_payload(&payload)?;
        WallSocialEventsRepository::record_event(
            &self.db,
            &WallSocialEventData {
                event_id: &event_id,
                event_type: WallSocialEventType::CommentCreate,
                post_id,
                actor_peer_id: &identity.peer_id,
                author_name: Some(&identity.display_name),
                comment_id: Some(&comment_id),
                content: Some(content),
                reaction_type: None,
                timestamp,
                payload_cbor: &payload_cbor,
                signature: &signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        CommentsRepository::add_comment(
            &self.db,
            &CommentData {
                comment_id: comment_id.clone(),
                post_id: post_id.to_string(),
                author_peer_id: identity.peer_id,
                author_name: identity.display_name,
                content: content.to_string(),
                created_at: timestamp,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        CommentsRepository::get_by_comment_id(&self.db, &comment_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::Internal("Failed to retrieve created comment".to_string()))
    }

    pub fn delete_comment(&self, comment_id: &str) -> Result<bool> {
        crate::services::IdentityPublishingPolicy::enforce(&self.db, &self.identity_service)?;
        let identity = self.current_identity()?;
        let comment = CommentsRepository::get_by_comment_id(&self.db, comment_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;
        if comment.author_peer_id != identity.peer_id {
            return Err(AppError::PermissionDenied(
                "You can only delete your own comments".to_string(),
            ));
        }
        let event_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();
        let payload = SignableWallCommentDelete {
            event_id: event_id.clone(),
            post_id: comment.post_id.clone(),
            comment_id: comment_id.to_string(),
            actor_peer_id: identity.peer_id.clone(),
            timestamp,
        };
        let signature = self.identity_service.sign(&payload)?;
        let payload_cbor = canonical_payload(&payload)?;
        WallSocialEventsRepository::record_event(
            &self.db,
            &WallSocialEventData {
                event_id: &event_id,
                event_type: WallSocialEventType::CommentDelete,
                post_id: &comment.post_id,
                actor_peer_id: &identity.peer_id,
                author_name: None,
                comment_id: Some(comment_id),
                content: None,
                reaction_type: None,
                timestamp,
                payload_cbor: &payload_cbor,
                signature: &signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        CommentsRepository::delete_comment(&self.db, comment_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    pub fn add_reaction(&self, post_id: &str, reaction_type: &str) -> Result<()> {
        crate::services::IdentityPublishingPolicy::enforce(&self.db, &self.identity_service)?;
        self.ensure_current_user_can_read_post(post_id)?;
        let identity = self.current_identity()?;
        let event_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();
        let payload = SignableWallReactionAdd {
            event_id: event_id.clone(),
            post_id: post_id.to_string(),
            actor_peer_id: identity.peer_id.clone(),
            reaction_type: reaction_type.to_string(),
            timestamp,
        };
        let signature = self.identity_service.sign(&payload)?;
        let payload_cbor = canonical_payload(&payload)?;
        WallSocialEventsRepository::record_event(
            &self.db,
            &WallSocialEventData {
                event_id: &event_id,
                event_type: WallSocialEventType::ReactionAdd,
                post_id,
                actor_peer_id: &identity.peer_id,
                author_name: None,
                comment_id: None,
                content: None,
                reaction_type: Some(reaction_type),
                timestamp,
                payload_cbor: &payload_cbor,
                signature: &signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        LikesRepository::add_like(
            &self.db,
            &LikeData {
                post_id: post_id.to_string(),
                liker_peer_id: identity.peer_id,
                reaction_type: reaction_type.to_string(),
                timestamp,
                signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        Ok(())
    }

    pub fn remove_reaction(&self, post_id: &str, reaction_type: &str) -> Result<()> {
        crate::services::IdentityPublishingPolicy::enforce(&self.db, &self.identity_service)?;
        let identity = self.current_identity()?;
        let event_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();
        let payload = SignableWallReactionRemove {
            event_id: event_id.clone(),
            post_id: post_id.to_string(),
            actor_peer_id: identity.peer_id.clone(),
            reaction_type: reaction_type.to_string(),
            timestamp,
        };
        let signature = self.identity_service.sign(&payload)?;
        let payload_cbor = canonical_payload(&payload)?;
        WallSocialEventsRepository::record_event(
            &self.db,
            &WallSocialEventData {
                event_id: &event_id,
                event_type: WallSocialEventType::ReactionRemove,
                post_id,
                actor_peer_id: &identity.peer_id,
                author_name: None,
                comment_id: None,
                content: None,
                reaction_type: Some(reaction_type),
                timestamp,
                payload_cbor: &payload_cbor,
                signature: &signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        LikesRepository::remove_like(&self.db, post_id, &identity.peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        Ok(())
    }

    pub fn process_incoming_event(
        &self,
        params: &IncomingWallSocialEventParams<'_>,
    ) -> Result<bool> {
        self.ensure_actor_can_read_post(params.post_id, params.actor_peer_id)?;
        self.verify_incoming_signature(params)?;
        if WallSocialEventsRepository::get_by_event_id(&self.db, params.event_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .is_some()
        {
            return Ok(false);
        }
        let payload_cbor = self.payload_cbor_from_params(params)?;
        let inserted = WallSocialEventsRepository::record_event(
            &self.db,
            &WallSocialEventData {
                event_id: params.event_id,
                event_type: params.event_type,
                post_id: params.post_id,
                actor_peer_id: params.actor_peer_id,
                author_name: params.author_name,
                comment_id: params.comment_id,
                content: params.content,
                reaction_type: params.reaction_type,
                timestamp: params.timestamp,
                payload_cbor: &payload_cbor,
                signature: params.signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        if inserted {
            self.apply_materialized_event(params)?;
        }
        Ok(inserted)
    }

    pub fn list_events_for_post(&self, post_id: &str) -> Result<Vec<WallSocialEvent>> {
        self.ensure_current_user_can_read_post(post_id)?;
        WallSocialEventsRepository::list_for_post(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    fn current_identity(&self) -> Result<crate::models::identity::LocalIdentity> {
        self.identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))
    }

    fn ensure_current_user_can_read_post(&self, post_id: &str) -> Result<()> {
        let identity = self.current_identity()?;
        self.ensure_actor_can_read_post(post_id, &identity.peer_id)
    }

    fn ensure_actor_can_read_post(&self, post_id: &str, actor_peer_id: &str) -> Result<()> {
        let post = PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Post not found".to_string()))?;
        if post.deleted_at.is_some() {
            return Err(AppError::NotFound("Post not found".to_string()));
        }
        if post.author_peer_id == actor_peer_id || post.visibility == PostVisibility::Public {
            return Ok(());
        }
        if self
            .permissions_service
            .peer_has_capability(actor_peer_id, Capability::WallRead)?
            || self
                .permissions_service
                .we_have_capability(&post.author_peer_id, Capability::WallRead)?
        {
            return Ok(());
        }
        Err(AppError::PermissionDenied(
            "Peer cannot comment or react to a post they cannot read".to_string(),
        ))
    }

    fn verify_incoming_signature(&self, params: &IncomingWallSocialEventParams<'_>) -> Result<()> {
        let public_key = self
            .contacts_service
            .get_public_key(params.actor_peer_id)?
            .or_else(|| {
                self.identity_service
                    .get_identity()
                    .ok()
                    .flatten()
                    .filter(|identity| identity.peer_id == params.actor_peer_id)
                    .map(|identity| identity.public_key)
            })
            .ok_or_else(|| AppError::NotFound("Actor not in contacts".to_string()))?;
        let verifying_key = VerifyingKey::from_bytes(
            public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;
        let is_valid = match params.event_type {
            WallSocialEventType::CommentCreate => verify(
                &verifying_key,
                &SignableWallCommentCreate {
                    event_id: params.event_id.to_string(),
                    post_id: params.post_id.to_string(),
                    comment_id: params.comment_id.unwrap_or_default().to_string(),
                    actor_peer_id: params.actor_peer_id.to_string(),
                    author_name: params.author_name.unwrap_or_default().to_string(),
                    content: params.content.unwrap_or_default().to_string(),
                    timestamp: params.timestamp,
                },
                params.signature,
            )?,
            WallSocialEventType::CommentDelete => verify(
                &verifying_key,
                &SignableWallCommentDelete {
                    event_id: params.event_id.to_string(),
                    post_id: params.post_id.to_string(),
                    comment_id: params.comment_id.unwrap_or_default().to_string(),
                    actor_peer_id: params.actor_peer_id.to_string(),
                    timestamp: params.timestamp,
                },
                params.signature,
            )?,
            WallSocialEventType::ReactionAdd => verify(
                &verifying_key,
                &SignableWallReactionAdd {
                    event_id: params.event_id.to_string(),
                    post_id: params.post_id.to_string(),
                    actor_peer_id: params.actor_peer_id.to_string(),
                    reaction_type: params.reaction_type.unwrap_or("like").to_string(),
                    timestamp: params.timestamp,
                },
                params.signature,
            )?,
            WallSocialEventType::ReactionRemove => verify(
                &verifying_key,
                &SignableWallReactionRemove {
                    event_id: params.event_id.to_string(),
                    post_id: params.post_id.to_string(),
                    actor_peer_id: params.actor_peer_id.to_string(),
                    reaction_type: params.reaction_type.unwrap_or("like").to_string(),
                    timestamp: params.timestamp,
                },
                params.signature,
            )?,
            WallSocialEventType::LegacyCommentCreate | WallSocialEventType::LegacyReactionAdd => {
                false
            }
        };
        if is_valid {
            Ok(())
        } else {
            Err(AppError::Crypto(
                "Invalid wall social event signature".to_string(),
            ))
        }
    }

    fn payload_cbor_from_params(
        &self,
        params: &IncomingWallSocialEventParams<'_>,
    ) -> Result<Vec<u8>> {
        match params.event_type {
            WallSocialEventType::CommentCreate => canonical_payload(&SignableWallCommentCreate {
                event_id: params.event_id.to_string(),
                post_id: params.post_id.to_string(),
                comment_id: params.comment_id.unwrap_or_default().to_string(),
                actor_peer_id: params.actor_peer_id.to_string(),
                author_name: params.author_name.unwrap_or_default().to_string(),
                content: params.content.unwrap_or_default().to_string(),
                timestamp: params.timestamp,
            }),
            WallSocialEventType::CommentDelete => canonical_payload(&SignableWallCommentDelete {
                event_id: params.event_id.to_string(),
                post_id: params.post_id.to_string(),
                comment_id: params.comment_id.unwrap_or_default().to_string(),
                actor_peer_id: params.actor_peer_id.to_string(),
                timestamp: params.timestamp,
            }),
            WallSocialEventType::ReactionAdd => canonical_payload(&SignableWallReactionAdd {
                event_id: params.event_id.to_string(),
                post_id: params.post_id.to_string(),
                actor_peer_id: params.actor_peer_id.to_string(),
                reaction_type: params.reaction_type.unwrap_or("like").to_string(),
                timestamp: params.timestamp,
            }),
            WallSocialEventType::ReactionRemove => canonical_payload(&SignableWallReactionRemove {
                event_id: params.event_id.to_string(),
                post_id: params.post_id.to_string(),
                actor_peer_id: params.actor_peer_id.to_string(),
                reaction_type: params.reaction_type.unwrap_or("like").to_string(),
                timestamp: params.timestamp,
            }),
            WallSocialEventType::LegacyCommentCreate | WallSocialEventType::LegacyReactionAdd => {
                Ok(Vec::new())
            }
        }
    }

    fn apply_materialized_event(&self, params: &IncomingWallSocialEventParams<'_>) -> Result<()> {
        match params.event_type {
            WallSocialEventType::CommentCreate => {
                CommentsRepository::add_comment(
                    &self.db,
                    &CommentData {
                        comment_id: params.comment_id.unwrap_or_default().to_string(),
                        post_id: params.post_id.to_string(),
                        author_peer_id: params.actor_peer_id.to_string(),
                        author_name: params
                            .author_name
                            .unwrap_or(params.actor_peer_id)
                            .to_string(),
                        content: params.content.unwrap_or_default().to_string(),
                        created_at: params.timestamp,
                    },
                )
                .map_err(|e| AppError::DatabaseString(e.to_string()))?;
            }
            WallSocialEventType::CommentDelete => {
                if let Some(comment_id) = params.comment_id {
                    CommentsRepository::delete_comment(&self.db, comment_id)
                        .map_err(|e| AppError::DatabaseString(e.to_string()))?;
                }
            }
            WallSocialEventType::ReactionAdd => {
                LikesRepository::add_like(
                    &self.db,
                    &LikeData {
                        post_id: params.post_id.to_string(),
                        liker_peer_id: params.actor_peer_id.to_string(),
                        reaction_type: params.reaction_type.unwrap_or("like").to_string(),
                        timestamp: params.timestamp,
                        signature: params.signature.to_vec(),
                    },
                )
                .map_err(|e| AppError::DatabaseString(e.to_string()))?;
            }
            WallSocialEventType::ReactionRemove => {
                LikesRepository::remove_like(&self.db, params.post_id, params.actor_peer_id)
                    .map_err(|e| AppError::DatabaseString(e.to_string()))?;
            }
            WallSocialEventType::LegacyCommentCreate | WallSocialEventType::LegacyReactionAdd => {}
        }
        Ok(())
    }
}

fn canonical_payload<T: Signable>(payload: &T) -> Result<Vec<u8>> {
    payload.signable_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{PostData, PostVisibility};
    use crate::models::identity::CreateIdentityRequest;

    fn create_service() -> (
        Arc<Database>,
        Arc<IdentityService>,
        WallSocialService,
        String,
    ) {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
        let permissions_service = Arc::new(PermissionsService::new(
            db.clone(),
            identity_service.clone(),
        ));
        let identity = identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Social Tester".to_string(),
                passphrase: "test-pass".to_string(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO identity_migration_state(peer_id, mode, updated_at) VALUES(?, 'compatibility', 1)",
                [&identity.peer_id],
            )
            .map(|_| ())
        })
        .unwrap();
        let service = WallSocialService::new(
            db.clone(),
            identity_service.clone(),
            contacts_service,
            permissions_service,
        );
        (db, identity_service, service, identity.peer_id)
    }

    fn insert_public_post(db: &Database, author_peer_id: &str, post_id: &str) {
        PostsRepository::insert_post(
            db,
            &PostData {
                post_id: post_id.to_string(),
                author_peer_id: author_peer_id.to_string(),
                content_type: "text".to_string(),
                content_text: Some("hello".to_string()),
                visibility: PostVisibility::Public,
                lamport_clock: 1,
                created_at: 100,
                signature: vec![1; 64],
            },
        )
        .unwrap();
    }

    #[test]
    fn signed_comment_create_records_event_and_materialized_comment() {
        let (db, _identity, service, peer_id) = create_service();
        insert_public_post(&db, &peer_id, "post-social-1");

        let comment = service
            .add_comment("post-social-1", "great post")
            .expect("comment should be created");

        assert_eq!(comment.content, "great post");
        let events = service.list_events_for_post("post-social-1").unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, WallSocialEventType::CommentCreate);
        assert_eq!(events[0].actor_peer_id, peer_id);
        assert!(!events[0].payload_cbor.is_empty());
        assert!(!events[0].signature.is_empty());
    }

    #[test]
    fn signed_reaction_add_and_remove_updates_like_state() {
        let (db, identity, service, peer_id) = create_service();
        insert_public_post(&db, &peer_id, "post-social-2");

        service.add_reaction("post-social-2", "like").unwrap();
        assert!(LikesRepository::has_liked(&db, "post-social-2", &peer_id).unwrap());

        service.remove_reaction("post-social-2", "like").unwrap();
        assert!(!LikesRepository::has_liked(&db, "post-social-2", &peer_id).unwrap());
        let events = service.list_events_for_post("post-social-2").unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, WallSocialEventType::ReactionAdd);
        assert_eq!(events[1].event_type, WallSocialEventType::ReactionRemove);
        assert!(identity.is_unlocked());
    }

    #[test]
    fn contacts_only_post_rejects_unauthorized_social_event() {
        let (db, _identity, service, peer_id) = create_service();
        PostsRepository::insert_post(
            &db,
            &PostData {
                post_id: "post-social-private".to_string(),
                author_peer_id: "author-peer".to_string(),
                content_type: "text".to_string(),
                content_text: Some("private".to_string()),
                visibility: PostVisibility::Contacts,
                lamport_clock: 1,
                created_at: 100,
                signature: vec![1; 64],
            },
        )
        .unwrap();

        let err = service
            .add_reaction("post-social-private", "like")
            .unwrap_err();
        assert!(matches!(err, AppError::PermissionDenied(_)));
        assert!(!LikesRepository::has_liked(&db, "post-social-private", &peer_id).unwrap());
    }
}
