use crate::{
    db::{Database, MentionsRepository, PostVisibility},
    error::{AppError, Result},
    models::QualifiedRelayName,
    services::{ContactsService, CryptoService, IdentityService, PostsService, Signable},
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedMention {
    pub qualified_name: String,
    pub status: String,
    pub peer_id: Option<String>,
    pub claim_digest: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedMentionInput {
    pub qualified_name: String,
    pub intent: String,
    pub authorized_peer_id: Option<String>,
    pub claim_digest: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishMentionedPostRequest {
    pub content_type: String,
    pub content_text: String,
    pub visibility: String,
    pub mentions: Vec<SignedMentionInput>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishMentionedPostResult {
    pub post_id: String,
    pub created_at: i64,
    pub tracking_wall: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MentionReceipt {
    pub mention_id: String,
    pub post_id: String,
    pub qualified_name: String,
    pub intent: String,
    pub status: String,
    pub sender_peer_id: String,
    pub preview: String,
    pub created_at: i64,
}
#[derive(Serialize)]
struct MentionSignature<'a> {
    domain: &'static str,
    post_id: &'a str,
    qualified_name: &'a str,
    intent: &'a str,
    sender_peer_id: &'a str,
    authorized_peer_id: Option<&'a str>,
    claim_digest: Option<&'a str>,
    created_at: i64,
}
impl Signable for MentionSignature<'_> {}

pub struct MentionsService {
    db: Arc<Database>,
    identity: Arc<IdentityService>,
    contacts: Arc<ContactsService>,
    posts: Arc<PostsService>,
}
impl MentionsService {
    pub fn new(
        db: Arc<Database>,
        identity: Arc<IdentityService>,
        contacts: Arc<ContactsService>,
        posts: Arc<PostsService>,
    ) -> Self {
        Self {
            db,
            identity,
            contacts,
            posts,
        }
    }
    pub fn resolve(&self, name: &str) -> Result<ResolvedMention> {
        let _: QualifiedRelayName = name.parse().map_err(|_| {
            AppError::Validation("Mention must use canonical @name@relay form".into())
        })?;
        let found = MentionsRepository::new(&self.db).resolve_claim(name)?;
        let Some((peer, digest)) = found else {
            return Ok(ResolvedMention {
                qualified_name: name.into(),
                status: "unknown".into(),
                peer_id: None,
                claim_digest: None,
            });
        };
        if self.contacts.is_blocked(&peer)? {
            return Ok(ResolvedMention {
                qualified_name: name.into(),
                status: "blocked".into(),
                peer_id: None,
                claim_digest: Some(digest),
            });
        }
        let known = self.contacts.is_contact(&peer)?;
        Ok(ResolvedMention {
            qualified_name: name.into(),
            status: if known { "known" } else { "private" }.into(),
            peer_id: if known { Some(peer.to_string()) } else { None },
            claim_digest: Some(digest),
        })
    }
    pub fn publish(&self, r: PublishMentionedPostRequest) -> Result<PublishMentionedPostResult> {
        if r.mentions.is_empty() {
            return Err(AppError::Validation(
                "Mentioned post requires a recipient".into(),
            ));
        }
        let vis = match r.visibility.as_str() {
            "public" => PostVisibility::Public,
            "contacts" => PostVisibility::Contacts,
            _ => return Err(AppError::Validation("Invalid visibility".into())),
        };
        for m in &r.mentions {
            if m.intent != "notify" && m.intent != "repost-request" {
                return Err(AppError::Validation("Invalid mention intent".into()));
            }
            let resolved = self.resolve(&m.qualified_name)?;
            if resolved.status == "blocked" {
                return Err(AppError::Validation("Blocked mention recipient".into()));
            }
            if resolved.status == "known"
                && (m.authorized_peer_id != resolved.peer_id
                    || m.claim_digest != resolved.claim_digest)
            {
                return Err(AppError::Validation(
                    "Mention peer or claim binding changed".into(),
                ));
            }
            if resolved.status != "known" && m.authorized_peer_id.is_some() {
                return Err(AppError::Validation(
                    "Private mentions cannot assert an authorized peer".into(),
                ));
            }
        }
        let out = self
            .posts
            .create_post(&r.content_type, Some(&r.content_text), vis)?;
        let sender = self.identity.get_peer_id()?;
        for m in &r.mentions {
            let signed = MentionSignature {
                domain: "harbor/private-mention/1",
                post_id: &out.post_id,
                qualified_name: &m.qualified_name,
                intent: &m.intent,
                sender_peer_id: &sender,
                authorized_peer_id: m.authorized_peer_id.as_deref(),
                claim_digest: m.claim_digest.as_deref(),
                created_at: out.created_at,
            };
            let sig = self.identity.sign(&signed)?;
            let plain =
                serde_json::to_vec(&signed).map_err(|e| AppError::Serialization(e.to_string()))?;
            let cipher = if let Some(peer) = m.authorized_peer_id.as_deref() {
                let p = self.contacts.get_x25519_public(peer)?.ok_or_else(|| {
                    AppError::Validation("Contact encryption key unavailable".into())
                })?;
                let raw: [u8; 32] = p
                    .try_into()
                    .map_err(|_| AppError::Validation("Invalid contact encryption key".into()))?;
                let keys = self.identity.get_unlocked_keys()?;
                let shared = keys
                    .x25519_secret
                    .diffie_hellman(&x25519_dalek::PublicKey::from(raw));
                CryptoService::encrypt_message(
                    &CryptoService::derive_symmetric_key(
                        shared.as_bytes(),
                        b"harbor/private-mention/1",
                    ),
                    &plain,
                )?
            } else {
                let mut key = [0u8; 32];
                OsRng.fill_bytes(&mut key);
                CryptoService::encrypt_message(&key, &plain)?
            };
            MentionsRepository::new(&self.db).insert(
                &Uuid::new_v4().to_string(),
                &out.post_id,
                &m.qualified_name,
                &m.intent,
                &sender,
                m.authorized_peer_id.as_deref(),
                m.claim_digest.as_deref(),
                &r.content_text.chars().take(240).collect::<String>(),
                &cipher,
                &sig,
                out.created_at,
            )?
        }
        Ok(PublishMentionedPostResult {
            post_id: out.post_id,
            created_at: out.created_at,
            tracking_wall: r
                .mentions
                .iter()
                .find(|m| m.intent == "repost-request")
                .map(|m| format!("harbor://name/{}", m.qualified_name.trim_start_matches('@'))),
        })
    }
    pub fn pending(&self) -> Result<Vec<MentionReceipt>> {
        Ok(MentionsRepository::new(&self.db)
            .pending()?
            .into_iter()
            .map(|m| MentionReceipt {
                mention_id: m.mention_id,
                post_id: m.post_id,
                qualified_name: m.qualified_name,
                intent: m.intent,
                status: m.status,
                sender_peer_id: m.sender_peer_id,
                preview: m.preview,
                created_at: m.created_at,
            })
            .collect())
    }
    pub fn review(&self, id: &str, decision: &str) -> Result<()> {
        let repo = MentionsRepository::new(&self.db);
        let mention = repo
            .get(id)?
            .ok_or_else(|| AppError::NotFound("Mention not found".into()))?;
        let status = match decision {
            "accept-notification" => "accepted",
            "accept-repost" if mention.intent == "repost-request" => {
                self.posts.create_post("shared", Some(&mention.preview), PostVisibility::Contacts)?;
                "accepted"
            },
            "decline" => "declined",
            "block" => "blocked",
            _ => {
                return Err(AppError::Validation(
                    "Decision is not allowed for this mention".into(),
                ))
            }
        };
        if !repo.review(id, status, chrono::Utc::now().timestamp())? {
            return Err(AppError::Validation("Mention was already reviewed".into()));
        }
        Ok(())
    }
}
