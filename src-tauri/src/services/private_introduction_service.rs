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
    models::{ContactCard, IntroductionRequest, QualifiedRelayName},
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

pub fn receive(
    repo: &PrivateIntroductionsRepository<'_>,
    signed: &SignedIntroduction,
    now: i64,
) -> Result<bool, IntroductionError> {
    let r = &signed.request;
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
    if signed.card.expires_at <= now
        || signed.card.capabilities.iter().any(|g| {
            g.subject_peer_id != recipient_peer_id
                || g.expires_at.is_some_and(|e| e <= now)
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

    fn card(subject: &str, now: i64) -> ContactCard {
        ContactCard {
            domain: domain::CONTACT_CARD.into(),
            version: 1,
            name_claim_digest: vec![1; 32],
            peer_id: "issuer".into(),
            ed25519_public_key: vec![2; 32],
            x25519_public_key: vec![3; 32],
            routing: vec![],
            capabilities: vec![CapabilityGrantRecord {
                domain: domain::CAPABILITY_GRANT.into(),
                version: 1,
                grant_id: "g1".into(),
                issuer_peer_id: "issuer".into(),
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
        let envelope =
            encrypt_contact_card(card(peer, now), &issuer, peer, recipient_public.to_bytes())
                .unwrap();
        let decoded = decrypt_contact_card(
            &envelope,
            &recipient,
            issuer.verifying_key().to_bytes(),
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
        let mut value = card("someone-else", 1_000);
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
}
