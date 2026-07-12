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
    delivery_keys: std::sync::Mutex<std::collections::HashMap<String, (Vec<u8>, i64)>>,
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
            || (!p.recipient_peer_id.is_empty() && p.recipient_peer_id != local)
            || p.issued_at != envelope.issued_at
            || p.expires_at != envelope.expires_at
            || p.expires_at <= now
            || uuid::Uuid::parse_str(&p.nonce).is_err()
        {
            return Err(AppError::Validation(
                "Mention envelope binding failed".into(),
            ));
        }
        if p.recipient_peer_id.is_empty() {
            let claim = crate::db::repositories::RelayNamesRepository::new(&self.db)
                .active_for_peer(&local, now)?
                .ok_or_else(|| AppError::Validation("Local verified name unavailable".into()))?;
            let c: NameClaim = ciborium::de::from_reader(claim.as_slice())
                .map_err(|e| AppError::Serialization(e.to_string()))?;
            if format!("@{}@{}", c.request.local_name, c.request.relay) != p.recipient_name {
                return Err(AppError::Validation(
                    "Mention recipient name mismatch".into(),
                ));
            }
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
            delivery_keys: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
    pub fn cache_delivery_key(&self, name: &str, key: Vec<u8>, expires: i64) -> Result<()> {
        if key.len() != 32 || expires <= chrono::Utc::now().timestamp() {
            return Err(AppError::Validation("Invalid delivery key".into()));
        }
        self.delivery_keys
            .lock()
            .map_err(|_| AppError::Internal("Delivery-key cache unavailable".into()))?
            .insert(name.into(), (key, expires));
        Ok(())
    }
    fn cached_delivery_key(&self, name: &str) -> Option<Vec<u8>> {
        let now = chrono::Utc::now().timestamp();
        self.delivery_keys
            .lock()
            .ok()?
            .get(name)
            .filter(|(_, e)| *e >= now)
            .map(|(k, _)| k.clone())
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
            if resolved.status == "unknown" && self.cached_delivery_key(&m.qualified_name).is_none()
            {
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
            let resolved_key =
                MentionsRepository::new(&self.db).resolve_claim(&m.qualified_name)?;
            let (recipient_peer, recipient_key) = match resolved_key {
                Some((peer, _, key)) => (peer, key),
                None => (
                    String::new(),
                    self.cached_delivery_key(&m.qualified_name)
                        .ok_or_else(|| AppError::Validation("Delivery key expired".into()))?,
                ),
            };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::repositories::RelayNamesRepository,
        models::{CreateIdentityRequest, NameClaimRequest},
        services::{
            name_claim_service::{relay_signing_bytes, verify_and_cache},
            PermissionsService,
        },
    };
    use ed25519_dalek::{Signer, SigningKey};

    struct Profile {
        db: Arc<Database>,
        identity: Arc<IdentityService>,
        mentions: MentionsService,
        info: crate::models::IdentityInfo,
    }

    fn profile(name: &str) -> Profile {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity = Arc::new(IdentityService::new(db.clone()));
        let info = identity
            .create_identity(CreateIdentityRequest {
                display_name: name.into(),
                passphrase: "test passphrase".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        let contacts = Arc::new(ContactsService::new(db.clone(), identity.clone()));
        let permissions = Arc::new(PermissionsService::new(db.clone(), identity.clone()));
        let posts = Arc::new(PostsService::new(
            db.clone(),
            identity.clone(),
            contacts.clone(),
            permissions,
        ));
        let mentions = MentionsService::new(db.clone(), identity.clone(), contacts, posts);
        Profile {
            db,
            identity,
            mentions,
            info,
        }
    }

    fn claim(profile: &Profile, local_name: &str, relay_key: &SigningKey, now: i64) -> NameClaim {
        let stored = profile.identity.get_identity().unwrap().unwrap();
        let request = NameClaimRequest {
            domain: domain::NAME_CLAIM_REQUEST.into(),
            version: PROTOCOL_VERSION,
            local_name: local_name.into(),
            relay: "relay.test".into(),
            peer_id: profile.info.peer_id.clone(),
            ed25519_public_key: stored.public_key,
            x25519_public_key: stored.x25519_public,
            sequence: 1,
            issued_at: now - 1,
            nonce: vec![9; 16],
        };
        let user_signature = profile
            .identity
            .get_unlocked_keys()
            .unwrap()
            .ed25519_signing
            .sign(&canonical_cbor(&request).unwrap())
            .to_bytes()
            .to_vec();
        let mut claim = NameClaim {
            request,
            user_signature,
            status: "active".into(),
            not_before: now - 1,
            not_after: now + 3_600,
            relay_key_id: "relay-key-1".into(),
            relay_signature: Vec::new(),
        };
        claim.relay_signature = relay_key
            .sign(&relay_signing_bytes(&claim).unwrap())
            .to_bytes()
            .to_vec();
        claim
    }

    fn trust_and_cache(profile: &Profile, claim: &NameClaim, relay_key: &SigningKey, now: i64) {
        let repo = RelayNamesRepository::new(&profile.db);
        repo.pin_key(
            "relay.test",
            "relay-key-1",
            &relay_key.verifying_key().to_bytes(),
            now - 60,
            Some(now + 3_600),
        )
        .unwrap();
        verify_and_cache(&repo, claim, now).unwrap();
    }

    #[test]
    fn unknown_name_sealed_delivery_round_trip_rejects_wrong_recipient_tamper_and_expiry() {
        let now = chrono::Utc::now().timestamp();
        let relay_key = SigningKey::from_bytes(&[71; 32]);
        let alice = profile("alice");
        let bob = profile("bob");
        let charlie = profile("charlie");
        let alice_claim = claim(&alice, "alice", &relay_key, now);
        let bob_claim = claim(&bob, "bob", &relay_key, now);
        trust_and_cache(&alice, &alice_claim, &relay_key, now);
        trust_and_cache(&bob, &bob_claim, &relay_key, now);
        RelayNamesRepository::new(&charlie.db)
            .pin_key(
                "relay.test",
                "relay-key-1",
                &relay_key.verifying_key().to_bytes(),
                now - 60,
                Some(now + 3_600),
            )
            .unwrap();
        alice
            .db
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO identity_migration_state VALUES(?, 'verified', ?)",
                        rusqlite::params![alice.info.peer_id, now],
                    )
                    .map(|_| ())
            })
            .unwrap();

        assert_eq!(
            alice.mentions.resolve("@bob@relay.test").unwrap().status,
            "unknown"
        );
        alice
            .mentions
            .cache_delivery_key(
                "@bob@relay.test",
                bob.identity.get_identity().unwrap().unwrap().x25519_public,
                now + 300,
            )
            .unwrap();
        alice
            .mentions
            .publish(PublishMentionedPostRequest {
                content_type: "text".into(),
                content_text: "hello @bob@relay.test".into(),
                visibility: "public".into(),
                mentions: vec![SignedMentionInput {
                    qualified_name: "@bob@relay.test".into(),
                    intent: "notify".into(),
                    authorized_peer_id: None,
                    claim_digest: None,
                }],
            })
            .unwrap();
        let queued = alice.mentions.queued_outbound(now - 1, 10).unwrap();
        assert_eq!(queued.len(), 1);
        let item = &queued[0];
        let envelope = IncomingMentionEnvelope {
            request_id: item.mention_id.clone(),
            requester_peer_id: alice.info.peer_id.clone(),
            ephemeral_public_key: item.ephemeral_public_key.clone(),
            ciphertext: item.ciphertext.clone(),
            issued_at: item.expires_at - 300,
            expires_at: item.expires_at,
        };
        let delivery_time = envelope.issued_at + 1;

        assert!(charlie
            .mentions
            .ingest_queued_envelope(&envelope, delivery_time)
            .is_err());
        let mut tampered = envelope.clone();
        tampered.ciphertext[0] ^= 1;
        assert!(bob
            .mentions
            .ingest_queued_envelope(&tampered, delivery_time)
            .is_err());
        assert!(bob
            .mentions
            .ingest_queued_envelope(&envelope, item.expires_at)
            .is_err());
        assert!(bob
            .mentions
            .ingest_queued_envelope(&envelope, delivery_time)
            .unwrap());
        let pending = bob.mentions.pending().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].qualified_name, "@alice@relay.test");
        assert_eq!(pending[0].sender_peer_id, alice.info.peer_id);
    }
}
