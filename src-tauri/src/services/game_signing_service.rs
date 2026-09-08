use crate::{db::Database, error::AppError, services::IdentityService};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::Signer;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard},
};
use url::Url;

use super::game_signing_protocol::*;

#[derive(Debug, Clone)]
struct SignedDelivery {
    callback_url: String,
    domain: &'static str,
    proof_json: String,
    request_id: String,
}

pub struct GameSigningService {
    database: Arc<Database>,
    identity: Arc<IdentityService>,
    pending: Mutex<HashMap<String, PendingSigningRequest>>,
    client: reqwest::Client,
    deliveries_in_flight: Arc<Mutex<HashSet<String>>>,
}

impl GameSigningService {
    pub fn new(database: Arc<Database>, identity: Arc<IdentityService>) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|error| {
                AppError::Network(format!("Could not initialize game proof delivery: {error}"))
            })?;
        Ok(Self {
            database,
            identity,
            pending: Mutex::new(HashMap::new()),
            client,
            deliveries_in_flight: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub fn prepare_deep_link(
        &self,
        deep_link: &str,
        now: i64,
    ) -> Result<GameSigningRequestPresentation, AppError> {
        if deep_link.len() > MAX_DEEP_LINK_BYTES {
            return Err(validation("Game signing link is too large"));
        }
        let parsed =
            Url::parse(deep_link).map_err(|_| validation("Malformed game signing link"))?;
        if parsed.scheme() != "harbor"
            || parsed.host_str() != Some("games")
            || parsed.port().is_some()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(validation("Unsupported game signing link"));
        }
        let segments: Vec<&str> = parsed
            .path_segments()
            .ok_or_else(|| validation("Malformed game signing link path"))?
            .filter(|segment| !segment.is_empty())
            .collect();
        if segments.len() != 2 {
            return Err(validation("Malformed game signing link path"));
        }
        if segments[1].len() > MAX_REQUEST_BYTES.saturating_mul(2) {
            return Err(validation("Encoded game signing request is too large"));
        }
        let request_bytes = URL_SAFE_NO_PAD
            .decode(segments[1])
            .map_err(|_| validation("Game signing request is not valid base64url"))?;
        if request_bytes.len() > MAX_REQUEST_BYTES {
            return Err(validation("Game signing request is too large"));
        }

        let approval_id = uuid::Uuid::new_v4().to_string();
        let request = match segments[0] {
            "auth" => {
                let request: HarborGameAuthRequest = serde_json::from_slice(&request_bytes)
                    .map_err(|_| validation("Malformed Harbor authentication request"))?;
                validate_auth_request(&request, now)?;
                PendingSigningRequest::Auth(request)
            }
            "package" => {
                let request: HarborGamePackageSigningRequest =
                    serde_json::from_slice(&request_bytes)
                        .map_err(|_| validation("Malformed Harbor package signing request"))?;
                validate_package_request(&request, now)?;
                PendingSigningRequest::Package(request)
            }
            _ => return Err(validation("Unsupported game signing request type")),
        };
        let presentation = request.presentation(approval_id.clone());
        let mut pending = self.pending_requests();
        pending.retain(|_, request| request.expires_at() > now);
        if pending.len() >= MAX_PENDING_REQUESTS {
            return Err(validation("Too many pending game signing requests"));
        }
        pending.insert(approval_id, request);
        Ok(presentation)
    }

    pub async fn approve(
        &self,
        approval_id: &str,
        now: i64,
    ) -> Result<GameSigningDelivery, AppError> {
        validate_identifier("approval ID", approval_id, 64)?;
        let request = self
            .pending_requests()
            .get(approval_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound("Game signing request is not pending".into()))?;
        request.validate(now)?;
        let signed = self.sign_request(&request)?;
        self.reserve_delivery(&signed, now)?;
        let _delivery_guard = DeliveryInFlightGuard::acquire(
            self.deliveries_in_flight.clone(),
            format!("{}:{}", signed.domain, signed.request_id),
        )?;

        let response = self
            .client
            .post(&signed.callback_url)
            .header("Origin", TRUSTED_GAMES_ORIGIN)
            .header("Content-Type", "application/json")
            .body(signed.proof_json.clone())
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                self.mark_delivered(signed.domain, &signed.request_id, now)?;
                self.pending_requests().remove(approval_id);
                Ok(GameSigningDelivery::Delivered {
                    request_id: signed.request_id,
                })
            }
            Ok(response) => Ok(GameSigningDelivery::Pending {
                request_id: signed.request_id,
                error: format!(
                    "Neo Grounds rejected proof delivery with HTTP {}",
                    response.status()
                ),
            }),
            Err(error) => Ok(GameSigningDelivery::Pending {
                request_id: signed.request_id,
                error: format!("Neo Grounds proof delivery failed: {error}"),
            }),
        }
    }

    fn sign_request(&self, request: &PendingSigningRequest) -> Result<SignedDelivery, AppError> {
        let keys = self.identity.get_validated_unlocked_keys()?;
        let identity = self
            .identity
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity found".into()))?;
        let creator_public_key = hex::encode(keys.ed25519_signing.verifying_key().to_bytes());
        if identity.public_key != keys.ed25519_signing.verifying_key().to_bytes() {
            return Err(AppError::Crypto(
                "Identity public key changed before game signing".into(),
            ));
        }

        match request {
            PendingSigningRequest::Auth(request) => {
                let payload = HarborGameAuthPayload {
                    account_id: request.account_id.clone(),
                    audience: request.audience.clone(),
                    challenge_id: request.challenge_id.clone(),
                    creator_peer_id: identity.peer_id,
                    creator_public_key,
                    domain: AUTH_DOMAIN.into(),
                    expires_at: request.expires_at,
                    issued_at: request.issued_at,
                    nonce: request.nonce.clone(),
                    request_id: request.request_id.clone(),
                    version: 1,
                };
                let bytes = serde_json::to_vec(&payload)
                    .map_err(|error| AppError::Serialization(error.to_string()))?;
                let proof = HarborGameAuthProof {
                    format: "harbor-game-auth-proof".into(),
                    signature: Ed25519SignatureValue {
                        algorithm: "ed25519".into(),
                        value: hex::encode(keys.ed25519_signing.sign(&bytes).to_bytes()),
                    },
                    payload,
                    version: 1,
                };
                Ok(SignedDelivery {
                    callback_url: request.callback_url.clone(),
                    domain: AUTH_DOMAIN,
                    proof_json: serde_json::to_string(&proof)
                        .map_err(|error| AppError::Serialization(error.to_string()))?,
                    request_id: request.request_id.clone(),
                })
            }
            PendingSigningRequest::Package(request) => {
                let payload = HarborGamePackageSigningPayload {
                    creator_peer_id: identity.peer_id,
                    creator_public_key,
                    domain: PACKAGE_DOMAIN.into(),
                    game_id: request.game_id.clone(),
                    issued_at: request.issued_at,
                    package_digest: request.package_digest.clone(),
                    permissions: request.permissions.clone(),
                    request_id: request.request_id.clone(),
                    title: request.title.clone(),
                    version: 1,
                    version_id: request.version_id.clone(),
                };
                let bytes = serde_json::to_vec(&payload)
                    .map_err(|error| AppError::Serialization(error.to_string()))?;
                let proof = HarborGameCreatorSignatureEnvelope {
                    format: "harbor-game-creator-signature".into(),
                    signature: Ed25519SignatureValue {
                        algorithm: "ed25519".into(),
                        value: hex::encode(keys.ed25519_signing.sign(&bytes).to_bytes()),
                    },
                    payload,
                    version: 1,
                };
                Ok(SignedDelivery {
                    callback_url: request.callback_url.clone(),
                    domain: PACKAGE_DOMAIN,
                    proof_json: serde_json::to_string(&proof)
                        .map_err(|error| AppError::Serialization(error.to_string()))?,
                    request_id: request.request_id.clone(),
                })
            }
        }
    }

    fn reserve_delivery(&self, delivery: &SignedDelivery, now: i64) -> Result<(), AppError> {
        self.database.with_connection(|connection| {
            let existing = connection.query_row(
                "SELECT callback_url, proof_json, status FROM game_signing_requests WHERE domain=?1 AND request_id=?2",
                rusqlite::params![delivery.domain, delivery.request_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
            );
            match existing {
                Ok((callback_url, proof_json, status)) => {
                    if status == "delivered" {
                        return Err(rusqlite::Error::QueryReturnedNoRows);
                    }
                    if callback_url != delivery.callback_url || proof_json != delivery.proof_json {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    Ok(())
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    connection.execute(
                        "INSERT INTO game_signing_requests(domain, request_id, callback_url, proof_json, status, created_at) VALUES(?1, ?2, ?3, ?4, 'pending', ?5)",
                        rusqlite::params![delivery.domain, delivery.request_id, delivery.callback_url, delivery.proof_json, now],
                    )?;
                    Ok(())
                }
                Err(error) => Err(error),
            }
        }).map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => validation("Game signing request was already delivered"),
            rusqlite::Error::InvalidQuery => validation("Game signing request ID was reused with different content"),
            other => AppError::Database(other),
        })
    }

    fn mark_delivered(&self, domain: &str, request_id: &str, now: i64) -> Result<(), AppError> {
        let changed = self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE game_signing_requests SET status='delivered', delivered_at=?3 WHERE domain=?1 AND request_id=?2 AND status='pending'",
                rusqlite::params![domain, request_id, now],
            )
        })?;
        if changed != 1 {
            return Err(AppError::InvalidData(
                "Game signing delivery state changed unexpectedly".into(),
            ));
        }
        Ok(())
    }

    fn pending_requests(&self) -> MutexGuard<'_, HashMap<String, PendingSigningRequest>> {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

struct DeliveryInFlightGuard {
    deliveries: Arc<Mutex<HashSet<String>>>,
    key: String,
}

impl DeliveryInFlightGuard {
    fn acquire(deliveries: Arc<Mutex<HashSet<String>>>, key: String) -> Result<Self, AppError> {
        let mut active = deliveries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !active.insert(key.clone()) {
            return Err(validation(
                "Game signing proof delivery is already in progress",
            ));
        }
        drop(active);
        Ok(Self { deliveries, key })
    }
}

impl Drop for DeliveryInFlightGuard {
    fn drop(&mut self) {
        self.deliveries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{models::CreateIdentityRequest, services::CryptoService};
    use ed25519_dalek::{Signature, Verifier};

    const NOW: i64 = 1_788_825_600;

    fn service() -> GameSigningService {
        let database = Arc::new(Database::in_memory().unwrap());
        let identity = Arc::new(IdentityService::new(database.clone()));
        identity
            .create_identity(CreateIdentityRequest {
                display_name: "Creator".into(),
                passphrase: "safe-test-password".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        GameSigningService::new(database, identity).unwrap()
    }

    fn encode_link(kind: &str, value: serde_json::Value) -> String {
        let encoded = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&value).unwrap());
        format!("harbor://games/{kind}/{encoded}")
    }

    fn auth_request() -> serde_json::Value {
        serde_json::json!({
            "accountId": "account-1",
            "audience": TRUSTED_GAMES_ORIGIN,
            "callbackUrl": "https://games.social-harbor.com/api/auth/harbor/challenges/challenge-1/proof",
            "challengeId": "challenge-1",
            "expiresAt": NOW + 120,
            "issuedAt": NOW,
            "nonce": "a".repeat(64),
            "requestId": "auth-request-1",
            "version": 1
        })
    }

    fn package_request() -> serde_json::Value {
        serde_json::json!({
            "callbackUrl": "https://games.social-harbor.com/api/harbor-store/signing-requests/package-request-1/proof",
            "expiresAt": NOW + 300,
            "gameId": "game-1",
            "issuedAt": NOW,
            "packageDigest": "b".repeat(64),
            "permissions": ["achievements", "save_data"],
            "requestId": "package-request-1",
            "title": "Fixture Game",
            "version": 1,
            "versionId": "version-1"
        })
    }

    #[test]
    fn prepares_closed_auth_and_package_requests() {
        let service = service();
        let auth = service
            .prepare_deep_link(&encode_link("auth", auth_request()), NOW)
            .unwrap();
        assert!(matches!(auth, GameSigningRequestPresentation::Auth { .. }));
        let auth_json = serde_json::to_value(&auth).unwrap();
        assert!(auth_json.get("approvalId").is_some());
        assert!(auth_json.get("approval_id").is_none());
        let package = service
            .prepare_deep_link(&encode_link("package", package_request()), NOW)
            .unwrap();
        assert!(matches!(
            package,
            GameSigningRequestPresentation::Package { .. }
        ));
    }

    #[test]
    fn rejects_wrong_origin_expiry_oversize_malformed_and_cross_domain_requests() {
        let service = service();
        let mut wrong_origin = auth_request();
        wrong_origin["audience"] = serde_json::json!("https://attacker.invalid");
        assert!(service
            .prepare_deep_link(&encode_link("auth", wrong_origin), NOW)
            .is_err());

        let mut expired = auth_request();
        expired["expiresAt"] = serde_json::json!(NOW);
        assert!(service
            .prepare_deep_link(&encode_link("auth", expired), NOW)
            .is_err());
        assert!(service
            .prepare_deep_link(&format!("harbor://games/auth/{}", "a".repeat(9_000)), NOW)
            .is_err());
        assert!(service
            .prepare_deep_link("harbor://games/auth/not-base64!", NOW)
            .is_err());
        assert!(service
            .prepare_deep_link(&encode_link("auth", package_request()), NOW)
            .is_err());
        assert!(service
            .prepare_deep_link(&encode_link("package", auth_request()), NOW)
            .is_err());
    }

    #[test]
    fn package_payload_matches_neo_grounds_canonical_field_order_and_signature() {
        let service = service();
        let request = service
            .prepare_deep_link(&encode_link("package", package_request()), NOW)
            .unwrap();
        let approval_id = match request {
            GameSigningRequestPresentation::Package { approval_id, .. } => approval_id,
            _ => unreachable!(),
        };
        let pending = service
            .pending_requests()
            .get(&approval_id)
            .cloned()
            .unwrap();
        let signed = service.sign_request(&pending).unwrap();
        let envelope: HarborGameCreatorSignatureEnvelope =
            serde_json::from_str(&signed.proof_json).unwrap();
        let payload_bytes = serde_json::to_vec(&envelope.payload).unwrap();
        assert!(String::from_utf8(payload_bytes.clone())
            .unwrap()
            .starts_with("{\"creatorPeerId\":"));
        let public: [u8; 32] = hex::decode(&envelope.payload.creator_public_key)
            .unwrap()
            .try_into()
            .unwrap();
        let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&public).unwrap();
        let signature =
            Signature::from_slice(&hex::decode(&envelope.signature.value).unwrap()).unwrap();
        verifying_key.verify(&payload_bytes, &signature).unwrap();
        assert_eq!(
            CryptoService::derive_peer_id_from_verifying_key(&verifying_key).unwrap(),
            envelope.payload.creator_peer_id
        );
    }

    #[test]
    fn locked_identity_cannot_sign_and_no_private_material_is_serialized() {
        let service = service();
        let presentation = service
            .prepare_deep_link(&encode_link("auth", auth_request()), NOW)
            .unwrap();
        let approval_id = match &presentation {
            GameSigningRequestPresentation::Auth { approval_id, .. } => approval_id.clone(),
            _ => unreachable!(),
        };
        service.identity.lock();
        let pending = service
            .pending_requests()
            .get(&approval_id)
            .cloned()
            .unwrap();
        assert!(matches!(
            service.sign_request(&pending),
            Err(AppError::IdentityLocked(_))
        ));
        let presentation_json = serde_json::to_string(&presentation).unwrap();
        assert!(!presentation_json.contains("private"));
        assert!(!presentation_json.contains("passphrase"));
    }

    #[test]
    fn concurrent_delivery_of_one_proof_is_rejected() {
        let deliveries = Arc::new(Mutex::new(HashSet::new()));
        let first =
            DeliveryInFlightGuard::acquire(deliveries.clone(), "auth:request-1".into()).unwrap();
        assert!(
            DeliveryInFlightGuard::acquire(deliveries.clone(), "auth:request-1".into()).is_err()
        );
        drop(first);
        assert!(DeliveryInFlightGuard::acquire(deliveries, "auth:request-1".into()).is_ok());
    }

    #[test]
    fn delivered_or_mutated_request_ids_cannot_be_replayed() {
        let service = service();
        let presentation = service
            .prepare_deep_link(&encode_link("package", package_request()), NOW)
            .unwrap();
        let approval_id = match presentation {
            GameSigningRequestPresentation::Package { approval_id, .. } => approval_id,
            _ => unreachable!(),
        };
        let pending = service
            .pending_requests()
            .get(&approval_id)
            .cloned()
            .unwrap();
        let signed = service.sign_request(&pending).unwrap();
        service.reserve_delivery(&signed, NOW).unwrap();
        service
            .mark_delivered(signed.domain, &signed.request_id, NOW)
            .unwrap();
        assert!(service.reserve_delivery(&signed, NOW).is_err());

        let mut changed = signed.clone();
        changed.proof_json.push(' ');
        assert!(service.reserve_delivery(&changed, NOW).is_err());
    }
}
