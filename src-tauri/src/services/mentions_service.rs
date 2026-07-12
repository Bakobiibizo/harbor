use crate::{
    db::{Database, MentionsRepository, PostVisibility},
    error::{AppError, Result},
    models::{domain, NameClaim, QualifiedRelayName, PROTOCOL_VERSION},
    services::{
        name_claim_service::{verify_and_cache, ClaimVerificationError},
        signing::canonical_cbor,
        ContactsService, CryptoService, IdentityService, PostsService, Signable,
    },
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateMentionPayload {
    pub domain: String,
    pub version: u16,
    pub mention_id: String,
    pub post_id: String,
    pub sender_name: String,
    pub sender_peer_id: String,
    pub sender_claim: NameClaim,
    pub recipient_name: String,
    pub recipient_peer_id: String,
    pub intent: String,
    pub preview: String,
    pub nonce: String,
    pub issued_at: i64,
    pub expires_at: i64,
}
impl Signable for PrivateMentionPayload {}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPrivateMentionPayload {
    pub payload: PrivateMentionPayload,
    pub signature: Vec<u8>,
}
#[derive(Debug, Clone)]
pub struct IncomingMentionEnvelope {
    pub request_id: String,
    pub requester_peer_id: String,
    pub ephemeral_public_key: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub issued_at: i64,
    pub expires_at: i64,
}

pub struct MentionsService {
    db: Arc<Database>,
    identity: Arc<IdentityService>,
    contacts: Arc<ContactsService>,
    posts: Arc<PostsService>,
}
impl MentionsService {
    pub fn queued_outbound(
        &self,
        now: i64,
        limit: u32,
    ) -> Result<Vec<crate::db::repositories::QueuedMentionEnvelope>> {
        Ok(MentionsRepository::new(&self.db).queued_outbound(now, limit)?)
    }
    pub fn mark_outbound_delivered(&self, id: &str) -> Result<bool> {
        Ok(MentionsRepository::new(&self.db).mark_delivered(id)?)
    }
    pub fn ingest_queued_envelope(
        &self,
        envelope: &IncomingMentionEnvelope,
        now: i64,
    ) -> Result<bool> {
        if envelope.expires_at <= now
            || envelope.issued_at > now
            || envelope.expires_at - envelope.issued_at > 300
        {
            return Err(AppError::Validation("Mention envelope expired".into()));
        }
        let ep: [u8; 32] = envelope
            .ephemeral_public_key
            .as_slice()
            .try_into()
            .map_err(|_| AppError::Validation("Invalid mention ephemeral key".into()))?;
        let keys = self.identity.get_unlocked_keys()?;
        let shared = keys
            .x25519_secret
            .diffie_hellman(&x25519_dalek::PublicKey::from(ep));
        let plain = CryptoService::decrypt_message(
            &CryptoService::derive_symmetric_key(shared.as_bytes(), domain::MENTION.as_bytes()),
            &envelope.ciphertext,
        )?;
        let signed: SignedPrivateMentionPayload = ciborium::de::from_reader(plain.as_slice())
            .map_err(|e| AppError::Serialization(e.to_string()))?;
        let p = &signed.payload;
        let local = self.identity.get_peer_id()?;
        if p.domain != domain::MENTION
            || p.version != PROTOCOL_VERSION
            || p.mention_id != envelope.request_id
            || p.sender_peer_id != envelope.requester_peer_id
            || p.recipient_peer_id != local
            || p.issued_at != envelope.issued_at
            || p.expires_at != envelope.expires_at
            || p.expires_at <= now
            || uuid::Uuid::parse_str(&p.nonce).is_err()
        {
            return Err(AppError::Validation(
                "Mention envelope binding failed".into(),
            ));
        }
        let sender_name = format!(
            "@{}@{}",
            p.sender_claim.request.local_name, p.sender_claim.request.relay
        );
        if sender_name != p.sender_name {
            return Err(AppError::Validation("Sender name claim mismatch".into()));
        }
        let repo_names = crate::db::repositories::RelayNamesRepository::new(&self.db);
        match verify_and_cache(&repo_names, &p.sender_claim, now) {
            Ok(_) | Err(ClaimVerificationError::Superseded) => {}
            Err(e) => return Err(AppError::Crypto(e.to_string())),
        };
        if p.sender_claim.request.peer_id != p.sender_peer_id {
            return Err(AppError::Validation("Sender peer mismatch".into()));
        }
        let raw: [u8; 32] = p
            .sender_claim
            .request
            .ed25519_public_key
            .as_slice()
            .try_into()
            .map_err(|_| AppError::Validation("Invalid sender key".into()))?;
        let sig = Signature::from_slice(&signed.signature)
            .map_err(|_| AppError::Validation("Invalid mention signature".into()))?;
        VerifyingKey::from_bytes(&raw)
            .map_err(|_| AppError::Validation("Invalid sender key".into()))?
            .verify(
                &canonical_cbor(p).map_err(|e| AppError::Serialization(e.to_string()))?,
                &sig,
            )
            .map_err(|_| AppError::Validation("Invalid mention signature".into()))?;
        if MentionsRepository::new(&self.db).is_sender_blocked(&p.sender_peer_id)? {
            return Ok(false);
        }
        let mut claim_bytes = Vec::new();
        ciborium::ser::into_writer(&p.sender_claim, &mut claim_bytes)
            .map_err(|e| AppError::Serialization(e.to_string()))?;
        let digest = hex::encode(Sha256::digest(&claim_bytes));
        MentionsRepository::new(&self.db)
            .insert_received(
                &p.mention_id,
                &p.post_id,
                &p.sender_name,
                &p.intent,
                &p.sender_peer_id,
                &digest,
                &p.preview,
                &envelope.ciphertext,
                &envelope.ephemeral_public_key,
                &signed.signature,
                p.issued_at,
            )
            .map_err(Into::into)
    }
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
        let Some((peer, digest, _)) = found else {
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
        crate::services::IdentityPublishingPolicy::enforce(&self.db, &self.identity)?;
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
            if resolved.status == "unknown" {
                return Err(AppError::Validation(
                    "The relay name could not be resolved securely; reconnect and retry".into(),
                ));
            }
        }
        let out = self
            .posts
            .create_post(&r.content_type, Some(&r.content_text), vis)?;
        let sender = self.identity.get_peer_id()?;
        let sender_claim_bytes = crate::db::repositories::RelayNamesRepository::new(&self.db)
            .active_for_peer(&sender, chrono::Utc::now().timestamp())?
            .ok_or_else(|| {
                AppError::Validation("A verified relay name is required to mention people".into())
            })?;
        let sender_claim: NameClaim = ciborium::de::from_reader(sender_claim_bytes.as_slice())
            .map_err(|e| AppError::Serialization(e.to_string()))?;
        let sender_name = format!(
            "@{}@{}",
            sender_claim.request.local_name, sender_claim.request.relay
        );
        for m in &r.mentions {
            let (recipient_peer, _, recipient_key) = MentionsRepository::new(&self.db)
                .resolve_claim(&m.qualified_name)?
                .ok_or_else(|| {
                    AppError::Validation("Verified recipient claim disappeared".into())
                })?;
            let raw: [u8; 32] = recipient_key
                .try_into()
                .map_err(|_| AppError::Validation("Invalid recipient encryption key".into()))?;
            let ephemeral_secret = x25519_dalek::StaticSecret::random_from_rng(OsRng);
            let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_secret);
            let shared = ephemeral_secret.diffie_hellman(&x25519_dalek::PublicKey::from(raw));
            let mention_id = Uuid::new_v4().to_string();
            let expires_at = out.created_at + 300;
            let payload = PrivateMentionPayload {
                domain: domain::MENTION.into(),
                version: PROTOCOL_VERSION,
                mention_id: mention_id.clone(),
                post_id: out.post_id.clone(),
                sender_name: sender_name.clone(),
                sender_peer_id: sender.clone(),
                sender_claim: sender_claim.clone(),
                recipient_name: m.qualified_name.clone(),
                recipient_peer_id: recipient_peer,
                intent: m.intent.clone(),
                preview: r.content_text.chars().take(240).collect(),
                nonce: Uuid::new_v4().to_string(),
                issued_at: out.created_at,
                expires_at,
            };
            let signature = self.identity.sign(&payload)?;
            let plain = canonical_cbor(&SignedPrivateMentionPayload {
                payload,
                signature: signature.clone(),
            })
            .map_err(|e| AppError::Serialization(e.to_string()))?;
            let cipher = CryptoService::encrypt_message(
                &CryptoService::derive_symmetric_key(
                    shared.as_bytes(),
                    crate::models::domain::MENTION.as_bytes(),
                ),
                &plain,
            )?;
            MentionsRepository::new(&self.db).insert(
                &mention_id,
                &out.post_id,
                &m.qualified_name,
                &m.intent,
                &sender,
                m.authorized_peer_id.as_deref(),
                m.claim_digest.as_deref(),
                &r.content_text.chars().take(240).collect::<String>(),
                &cipher,
                ephemeral_public.as_bytes(),
                &signature,
                out.created_at,
            )?;
            MentionsRepository::new(&self.db).enqueue_outbound(
                &mention_id,
                &m.qualified_name,
                ephemeral_public.as_bytes(),
                &cipher,
                expires_at,
            )?;
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
        let local_peer_id = self.identity.get_peer_id()?;
        Ok(MentionsRepository::new(&self.db)
            .pending(&local_peer_id)?
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
                let now = chrono::Utc::now().timestamp();
                if !repo.review(id, "accepted", now)? {
                    return Err(AppError::Validation("Mention was already reviewed".into()));
                }
                let repost = format!(
                    "{}\n\n[Harbor repost of {} by {}]",
                    mention.preview, mention.post_id, mention.sender_peer_id
                );
                if let Err(error) =
                    self.posts
                        .create_post("shared", Some(&repost), PostVisibility::Contacts)
                {
                    let _=self.db.with_connection(|c|c.execute("UPDATE private_mentions SET status='pending',reviewed_at=NULL WHERE mention_id=? AND status='accepted'",[id]).map(|_|()));
                    return Err(error);
                }
                return Ok(());
            }
            "decline" => "declined",
            "block" => {
                self.contacts.block_contact(&mention.sender_peer_id)?;
                repo.block_sender(&mention.sender_peer_id, chrono::Utc::now().timestamp())?;
                "blocked"
            }
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
