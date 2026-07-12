use aes_gcm::{
    aead::{Aead, Payload},
    Aes256Gcm, KeyInit, Nonce,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use libp2p::{identity, PeerId};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use x25519_dalek::{PublicKey as X25519Public, StaticSecret};

use crate::{
    db::repositories::{IntroductionDecision, PrivateIntroductionsRepository},
    models::{
        domain, CapabilityGrantRecord, CapabilityRevocationRecord, ContactCard,
        IntroductionRequest, QualifiedRelayName, PROTOCOL_VERSION,
    },
    services::signing::canonical_cbor,
};

const MAX_AGE: i64 = 300;
const ALLOWED_CAPABILITIES: [&str; 5] = [
    "wall:read",
    "message:send",
    "call:initiate",
    "media:read",
    "mention:send",
];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IntroductionError {
    #[error("invalid request")]
    Invalid,
    #[error("request is stale")]
    Stale,
    #[error("requester is blocked")]
    Blocked,
    #[error("cryptographic verification failed")]
    Crypto,
    #[error("capability is not allowed")]
    Capability,
    #[error("database error")]
    Database,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedIntroduction {
    pub request: IntroductionRequest,
    pub requester_name: String,
    pub requester_signature: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedContactCard {
    pub card: ContactCard,
    pub signature: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedContactCard {
    pub sender_ephemeral_public_key: Vec<u8>,
    pub recipient_peer_id: String,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedCapabilityGrant {
    pub grant: CapabilityGrantRecord,
    pub signature: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedCapabilityRevocation {
    pub revocation: CapabilityRevocationRecord,
    pub signature: Vec<u8>,
}

fn peer_for_key(raw: [u8; 32]) -> Result<PeerId, IntroductionError> {
    let key = identity::ed25519::PublicKey::try_from_bytes(&raw)
        .map_err(|_| IntroductionError::Invalid)?;
    Ok(PeerId::from_public_key(&identity::PublicKey::from(key)))
}
pub fn verify_and_apply_grant(
    repo: &PrivateIntroductionsRepository<'_>,
    signed: &SignedCapabilityGrant,
    issuer_key: [u8; 32],
    now: i64,
) -> Result<bool, IntroductionError> {
    let g = &signed.grant;
    if g.domain != domain::CAPABILITY_GRANT
        || g.version != PROTOCOL_VERSION
        || g.revision == 0
        || g.issuer_peer_id != peer_for_key(issuer_key)?.to_string()
        || g.issued_at > now
        || g.expires_at.is_some_and(|e| e <= now)
        || !ALLOWED_CAPABILITIES.contains(&g.capability.as_str())
    {
        return Err(IntroductionError::Capability);
    }
    let key = VerifyingKey::from_bytes(&issuer_key).map_err(|_| IntroductionError::Invalid)?;
    let sig = Signature::from_slice(&signed.signature).map_err(|_| IntroductionError::Crypto)?;
    key.verify(
        &canonical_cbor(g).map_err(|_| IntroductionError::Invalid)?,
        &sig,
    )
    .map_err(|_| IntroductionError::Crypto)?;
    repo.apply_grant(g, now)
        .map_err(|_| IntroductionError::Database)
}
pub fn verify_and_apply_revocation(
    repo: &PrivateIntroductionsRepository<'_>,
    signed: &SignedCapabilityRevocation,
    issuer_key: [u8; 32],
) -> Result<bool, IntroductionError> {
    let r = &signed.revocation;
    if r.domain != domain::CAPABILITY_REVOCATION
        || r.version != PROTOCOL_VERSION
        || r.revision == 0
        || r.issuer_peer_id != peer_for_key(issuer_key)?.to_string()
    {
        return Err(IntroductionError::Capability);
    }
    let key = VerifyingKey::from_bytes(&issuer_key).map_err(|_| IntroductionError::Invalid)?;
    let sig = Signature::from_slice(&signed.signature).map_err(|_| IntroductionError::Crypto)?;
    key.verify(
        &canonical_cbor(r).map_err(|_| IntroductionError::Invalid)?,
        &sig,
    )
    .map_err(|_| IntroductionError::Crypto)?;
    repo.apply_revocation(r)
        .map_err(|_| IntroductionError::Database)
}

pub fn receive(
    repo: &PrivateIntroductionsRepository<'_>,
    signed: &SignedIntroduction,
    now: i64,
) -> Result<bool, IntroductionError> {
    let r = &signed.request;
    if r.domain != domain::INTRODUCTION
        || r.version != PROTOCOL_VERSION
        || uuid::Uuid::parse_str(&r.request_id).is_err()
        || r.requester_ephemeral_x25519_key.len() != 32
        || r.message_ciphertext.is_empty()
    {
        return Err(IntroductionError::Invalid);
    }
    let _: QualifiedRelayName = r.target.parse().map_err(|_| IntroductionError::Invalid)?;
    let _: QualifiedRelayName = signed
        .requester_name
        .parse()
        .map_err(|_| IntroductionError::Invalid)?;
    if r.issued_at > now
        || now - r.issued_at > MAX_AGE
        || r.expires_at < now
        || r.expires_at <= r.issued_at
    {
        return Err(IntroductionError::Stale);
    }
    let raw: [u8; 32] = r
        .requester_signing_key
        .as_slice()
        .try_into()
        .map_err(|_| IntroductionError::Invalid)?;
    let key = VerifyingKey::from_bytes(&raw).map_err(|_| IntroductionError::Invalid)?;
    let lp = identity::ed25519::PublicKey::try_from_bytes(&raw)
        .map_err(|_| IntroductionError::Invalid)?;
    if PeerId::from_public_key(&identity::PublicKey::from(lp)).to_string() != r.requester_peer_id {
        return Err(IntroductionError::Invalid);
    }
    if repo
        .is_blocked(&r.requester_peer_id)
        .map_err(|_| IntroductionError::Database)?
    {
        return Err(IntroductionError::Blocked);
    }
    let sig = Signature::from_slice(&signed.requester_signature)
        .map_err(|_| IntroductionError::Crypto)?;
    key.verify(
        &canonical_cbor(&(&signed.request, &signed.requester_name))
            .map_err(|_| IntroductionError::Invalid)?,
        &sig,
    )
    .map_err(|_| IntroductionError::Crypto)?;
    let digest = Sha256::digest(canonical_cbor(signed).map_err(|_| IntroductionError::Invalid)?);
    repo.receive(
        &r.request_id,
        &r.requester_peer_id,
        &signed.requester_name,
        &digest,
        now,
    )
    .map_err(|_| IntroductionError::Database)
}
pub fn decide(
    repo: &PrivateIntroductionsRepository<'_>,
    request_id: &str,
    decision: IntroductionDecision,
    at: i64,
) -> Result<bool, IntroductionError> {
    repo.decide(request_id, decision, at)
        .map_err(|_| IntroductionError::Database)
}

fn derive_key(
    secret: &StaticSecret,
    public: &X25519Public,
    recipient: &str,
) -> Result<[u8; 32], IntroductionError> {
    let shared = secret.diffie_hellman(public);
    let hk = Hkdf::<Sha256>::new(Some(b"harbor/contact-card/1"), shared.as_bytes());
    let mut key = [0; 32];
    hk.expand(recipient.as_bytes(), &mut key)
        .map_err(|_| IntroductionError::Crypto)?;
    Ok(key)
}
fn aad(recipient: &str, ephemeral: &[u8]) -> Vec<u8> {
    [
        b"harbor/contact-card/1".as_slice(),
        recipient.as_bytes(),
        ephemeral,
    ]
    .concat()
}

pub fn encrypt_contact_card(
    card: ContactCard,
    issuer_key: &SigningKey,
    recipient_peer_id: &str,
    recipient_public: [u8; 32],
) -> Result<EncryptedContactCard, IntroductionError> {
    if card.capabilities.len() > 5
        || card.capabilities.iter().any(|g| {
            g.subject_peer_id != recipient_peer_id
                || !ALLOWED_CAPABILITIES.contains(&g.capability.as_str())
                || g.revision == 0
        })
    {
        return Err(IntroductionError::Capability);
    }
    let bytes = canonical_cbor(&card).map_err(|_| IntroductionError::Invalid)?;
    let signed = SignedContactCard {
        card,
        signature: issuer_key.sign(&bytes).to_bytes().to_vec(),
    };
    let plaintext = canonical_cbor(&signed).map_err(|_| IntroductionError::Invalid)?;
    let ephemeral = StaticSecret::random_from_rng(OsRng);
    let ephemeral_public = X25519Public::from(&ephemeral);
    let key = derive_key(
        &ephemeral,
        &X25519Public::from(recipient_public),
        recipient_peer_id,
    )?;
    let mut nonce = [0; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| IntroductionError::Crypto)?
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: &aad(recipient_peer_id, ephemeral_public.as_bytes()),
            },
        )
        .map_err(|_| IntroductionError::Crypto)?;
    Ok(EncryptedContactCard {
        sender_ephemeral_public_key: ephemeral_public.as_bytes().to_vec(),
        recipient_peer_id: recipient_peer_id.into(),
        nonce: nonce.to_vec(),
        ciphertext,
    })
}
pub fn decrypt_contact_card(
    envelope: &EncryptedContactCard,
    recipient_secret: &StaticSecret,
    expected_issuer: [u8; 32],
    expected_claim_digest: &[u8],
    recipient_peer_id: &str,
    now: i64,
) -> Result<SignedContactCard, IntroductionError> {
    if envelope.recipient_peer_id != recipient_peer_id {
        return Err(IntroductionError::Crypto);
    }
    let ep: [u8; 32] = envelope
        .sender_ephemeral_public_key
        .as_slice()
        .try_into()
        .map_err(|_| IntroductionError::Invalid)?;
    let nonce: [u8; 12] = envelope
        .nonce
        .as_slice()
        .try_into()
        .map_err(|_| IntroductionError::Invalid)?;
    let key = derive_key(recipient_secret, &X25519Public::from(ep), recipient_peer_id)?;
    let plain = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| IntroductionError::Crypto)?
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &envelope.ciphertext,
                aad: &aad(recipient_peer_id, &ep),
            },
        )
        .map_err(|_| IntroductionError::Crypto)?;
    let signed: SignedContactCard =
        ciborium::de::from_reader(plain.as_slice()).map_err(|_| IntroductionError::Invalid)?;
    let vk = VerifyingKey::from_bytes(&expected_issuer).map_err(|_| IntroductionError::Invalid)?;
    let sig = Signature::from_slice(&signed.signature).map_err(|_| IntroductionError::Crypto)?;
    vk.verify(
        &canonical_cbor(&signed.card).map_err(|_| IntroductionError::Invalid)?,
        &sig,
    )
    .map_err(|_| IntroductionError::Crypto)?;
    let derived_peer = peer_for_key(expected_issuer)?;
    if signed.card.domain != domain::CONTACT_CARD
        || signed.card.version != PROTOCOL_VERSION
        || signed.card.revision == 0
        || signed.card.ed25519_public_key != expected_issuer
        || signed.card.peer_id != derived_peer.to_string()
        || signed.card.name_claim_digest != expected_claim_digest
        || signed.card.x25519_public_key.len() != 32
        || signed.card.issued_at > now
        || signed.card.expires_at <= signed.card.issued_at
        || signed.card.expires_at <= now
        || signed.card.capabilities.iter().any(|g| {
            g.domain != domain::CAPABILITY_GRANT
                || g.version != PROTOCOL_VERSION
                || g.issuer_peer_id != signed.card.peer_id
                || g.subject_peer_id != recipient_peer_id
                || g.revision == 0
                || g.issued_at < signed.card.issued_at
                || g.issued_at > now
                || g.expires_at.is_some_and(|e| e <= now)
                || g.expires_at.is_some_and(|e| e > signed.card.expires_at)
                || !ALLOWED_CAPABILITIES.contains(&g.capability.as_str())
        })
    {
        return Err(IntroductionError::Capability);
    }
    Ok(signed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{domain, CapabilityGrantRecord};

    fn card(subject: &str, now: i64, issuer: &SigningKey) -> ContactCard {
        let public = issuer.verifying_key().to_bytes();
        let peer_id = peer_for_key(public).unwrap().to_string();
        ContactCard {
            domain: domain::CONTACT_CARD.into(),
            version: 1,
            name_claim_digest: vec![1; 32],
            peer_id: peer_id.clone(),
            ed25519_public_key: public.to_vec(),
            x25519_public_key: vec![3; 32],
            routing: vec![],
            capabilities: vec![CapabilityGrantRecord {
                domain: domain::CAPABILITY_GRANT.into(),
                version: 1,
                grant_id: "g1".into(),
                issuer_peer_id: peer_id,
                subject_peer_id: subject.into(),
                capability: "wall:read".into(),
                revision: 1,
                issued_at: now,
                expires_at: Some(now + 60),
                revocation_id: "r1".into(),
            }],
            issued_at: now,
            expires_at: now + 60,
            revision: 1,
            revocation_id: "card-r1".into(),
        }
    }

    #[test]
    fn contact_card_round_trip_and_tamper_rejection() {
        let now = 1_000;
        let issuer = SigningKey::from_bytes(&[7; 32]);
        let recipient = StaticSecret::from([8; 32]);
        let recipient_public = X25519Public::from(&recipient);
        let peer = "recipient";
        let envelope = encrypt_contact_card(
            card(peer, now, &issuer),
            &issuer,
            peer,
            recipient_public.to_bytes(),
        )
        .unwrap();
        let decoded = decrypt_contact_card(
            &envelope,
            &recipient,
            issuer.verifying_key().to_bytes(),
            &[1; 32],
            peer,
            now,
        )
        .unwrap();
        assert_eq!(decoded.card.capabilities[0].capability, "wall:read");
        let mut altered = envelope.clone();
        altered.ciphertext[0] ^= 1;
        assert!(matches!(
            decrypt_contact_card(
                &altered,
                &recipient,
                issuer.verifying_key().to_bytes(),
                &[1; 32],
                peer,
                now
            ),
            Err(IntroductionError::Crypto)
        ));
        assert!(matches!(
            decrypt_contact_card(
                &envelope,
                &StaticSecret::from([9; 32]),
                issuer.verifying_key().to_bytes(),
                &[1; 32],
                peer,
                now
            ),
            Err(IntroductionError::Crypto)
        ));
    }

    #[test]
    fn rejects_broad_or_wrong_subject_grants() {
        let issuer = SigningKey::from_bytes(&[7; 32]);
        let recipient = StaticSecret::from([8; 32]);
        let mut value = card("someone-else", 1_000, &issuer);
        assert!(matches!(
            encrypt_contact_card(
                value.clone(),
                &issuer,
                "recipient",
                X25519Public::from(&recipient).to_bytes()
            ),
            Err(IntroductionError::Capability)
        ));
        value.capabilities[0].subject_peer_id = "recipient".into();
        value.capabilities[0].capability = "admin:*".into();
        assert!(matches!(
            encrypt_contact_card(
                value,
                &issuer,
                "recipient",
                X25519Public::from(&recipient).to_bytes()
            ),
            Err(IntroductionError::Capability)
        ));
    }

    #[test]
    fn signed_grant_and_revocation_are_verified_monotonically() {
        let db = crate::db::Database::in_memory().unwrap();
        let repo = PrivateIntroductionsRepository::new(&db);
        let issuer = SigningKey::from_bytes(&[11; 32]);
        let peer = peer_for_key(issuer.verifying_key().to_bytes())
            .unwrap()
            .to_string();
        let now = 2_000;
        let grant = CapabilityGrantRecord {
            domain: domain::CAPABILITY_GRANT.into(),
            version: PROTOCOL_VERSION,
            grant_id: "g".into(),
            issuer_peer_id: peer.clone(),
            subject_peer_id: "subject".into(),
            capability: "wall:read".into(),
            revision: 1,
            issued_at: now,
            expires_at: Some(now + 100),
            revocation_id: "r".into(),
        };
        let signed = SignedCapabilityGrant {
            signature: issuer
                .sign(&canonical_cbor(&grant).unwrap())
                .to_bytes()
                .to_vec(),
            grant,
        };
        assert!(
            verify_and_apply_grant(&repo, &signed, issuer.verifying_key().to_bytes(), now).unwrap()
        );
        assert!(
            !verify_and_apply_grant(&repo, &signed, issuer.verifying_key().to_bytes(), now)
                .unwrap()
        );
        let mut forged = signed.clone();
        forged.grant.subject_peer_id = "attacker".into();
        assert!(matches!(
            verify_and_apply_grant(&repo, &forged, issuer.verifying_key().to_bytes(), now),
            Err(IntroductionError::Crypto)
        ));
        let rev = CapabilityRevocationRecord {
            domain: domain::CAPABILITY_REVOCATION.into(),
            version: PROTOCOL_VERSION,
            grant_id: "g".into(),
            issuer_peer_id: peer,
            revision: 2,
            revoked_at: now + 1,
            revocation_id: "r".into(),
        };
        let signed_rev = SignedCapabilityRevocation {
            signature: issuer
                .sign(&canonical_cbor(&rev).unwrap())
                .to_bytes()
                .to_vec(),
            revocation: rev,
        };
        assert!(
            verify_and_apply_revocation(&repo, &signed_rev, issuer.verifying_key().to_bytes())
                .unwrap()
        );
        assert!(!verify_and_apply_revocation(
            &repo,
            &signed_rev,
            issuer.verifying_key().to_bytes()
        )
        .unwrap());
        assert_eq!(
            repo.capability_decision(
                &signed_rev.revocation.issuer_peer_id,
                "subject",
                "wall:read",
                now + 2
            )
            .unwrap(),
            Some(false)
        );
    }

    fn signed_intro(key: &SigningKey) -> SignedIntroduction {
        let raw = key.verifying_key().to_bytes();
        let peer = peer_for_key(raw).unwrap().to_string();
        let request = IntroductionRequest {
            domain: domain::INTRODUCTION.into(),
            version: PROTOCOL_VERSION,
            request_id: uuid::Uuid::new_v4().to_string(),
            target: "@alice@relay.test".into(),
            requester_peer_id: peer,
            requester_signing_key: raw.to_vec(),
            requester_ephemeral_x25519_key: vec![5; 32],
            purpose: "contact".into(),
            message_ciphertext: vec![6; 48],
            issued_at: 1_000,
            expires_at: 1_200,
            challenge_id: "challenge".into(),
            work_nonce: 1,
        };
        let requester_name = "@bob@relay.test".to_string();
        let signature = key
            .sign(&canonical_cbor(&(&request, &requester_name)).unwrap())
            .to_bytes()
            .to_vec();
        SignedIntroduction {
            request,
            requester_name,
            requester_signature: signature,
        }
    }
    #[test]
    fn mutated_signed_introductions_never_create_decision_rows() {
        let key = SigningKey::from_bytes(&[14; 32]);
        let original = signed_intro(&key);
        for mutate in 0..4 {
            let db = crate::db::Database::in_memory().unwrap();
            let repo = PrivateIntroductionsRepository::new(&db);
            let mut value = original.clone();
            match mutate {
                0 => value.request.message_ciphertext[0] ^= 1,
                1 => value.request.target = "@mallory@relay.test".into(),
                2 => value.request.requester_peer_id = "12D3KooWForged".into(),
                _ => value.requester_signature[0] ^= 1,
            }
            assert!(receive(&repo, &value, 1_100).is_err());
            let count: i64 = db
                .with_connection(|c| {
                    c.query_row("SELECT COUNT(*) FROM introduction_decisions", [], |r| {
                        r.get(0)
                    })
                })
                .unwrap();
            assert_eq!(count, 0);
        }
    }
}
