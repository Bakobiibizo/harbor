use crate::{
    commands::network::NetworkState,
    db::{
        repositories::{MentionsRepository, RelayNamesRepository},
        Database,
    },
    error::{AppError, Result},
    models::{NameClaim, SignedRelayKeyRotation},
    services::{
        name_claim_service::verify_and_cache, relay_key_rotation_service::apply_signed_rotation,
        AccountsService, IdentityService,
    },
};
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameClaimRequestDto {
    pub domain: String,
    pub version: u16,
    pub local_name: String,
    pub relay: String,
    pub peer_id: String,
    pub ed25519_public_key: Vec<u8>,
    pub x25519_public_key: Vec<u8>,
    pub sequence: u64,
    pub issued_at: i64,
    pub nonce: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameClaimDto {
    pub request: NameClaimRequestDto,
    pub user_signature: Vec<u8>,
    pub status: String,
    pub not_before: i64,
    pub not_after: i64,
    pub relay_key_id: String,
    pub relay_signature: Vec<u8>,
}

impl From<NameClaim> for NameClaimDto {
    fn from(claim: NameClaim) -> Self {
        Self {
            request: NameClaimRequestDto {
                domain: claim.request.domain,
                version: claim.request.version,
                local_name: claim.request.local_name,
                relay: claim.request.relay,
                peer_id: claim.request.peer_id,
                ed25519_public_key: claim.request.ed25519_public_key,
                x25519_public_key: claim.request.x25519_public_key,
                sequence: claim.request.sequence,
                issued_at: claim.request.issued_at,
                nonce: claim.request.nonce,
            },
            user_signature: claim.user_signature,
            status: claim.status,
            not_before: claim.not_before,
            not_after: claim.not_after,
            relay_key_id: claim.relay_key_id,
            relay_signature: claim.relay_signature,
        }
    }
}

impl From<NameClaimDto> for NameClaim {
    fn from(claim: NameClaimDto) -> Self {
        Self {
            request: crate::models::NameClaimRequest {
                domain: claim.request.domain,
                version: claim.request.version,
                local_name: claim.request.local_name,
                relay: claim.request.relay,
                peer_id: claim.request.peer_id,
                ed25519_public_key: claim.request.ed25519_public_key,
                x25519_public_key: claim.request.x25519_public_key,
                sequence: claim.request.sequence,
                issued_at: claim.request.issued_at,
                nonce: claim.request.nonce,
            },
            user_signature: claim.user_signature,
            status: claim.status,
            not_before: claim.not_before,
            not_after: claim.not_after,
            relay_key_id: claim.relay_key_id,
            relay_signature: claim.relay_signature,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRelayNameRequest {
    pub name: String,
    pub namespace: String,
}

fn model(w: crate::p2p::protocols::board_sync::NameClaim) -> NameClaim {
    NameClaim {
        request: crate::models::NameClaimRequest {
            domain: w.request.domain,
            version: w.request.version,
            local_name: w.request.local_name,
            relay: w.request.relay,
            peer_id: w.request.peer_id,
            ed25519_public_key: w.request.ed25519_public_key,
            x25519_public_key: w.request.x25519_public_key,
            sequence: w.request.sequence,
            issued_at: w.request.issued_at,
            nonce: w.request.nonce,
        },
        user_signature: w.user_signature,
        status: w.status,
        not_before: w.not_before,
        not_after: w.not_after,
        relay_key_id: w.relay_key_id,
        relay_signature: w.relay_signature,
    }
}

#[tauri::command]
pub async fn register_relay_name(
    request: RegisterRelayNameRequest,
    network: State<'_, NetworkState>,
    db: State<'_, Arc<Database>>,
    accounts: State<'_, Arc<AccountsService>>,
) -> Result<NameClaimDto> {
    let peer = network.get_handle().await?.active_relay().await?;
    let (wire, key) = network
        .get_handle()
        .await?
        .register_relay_name(peer, request.name, request.namespace.clone())
        .await?;
    let public = libp2p::identity::PublicKey::try_decode_protobuf(&key)
        .map_err(|_| AppError::Crypto("Invalid relay authority key".into()))?;
    if libp2p::PeerId::from_public_key(&public) != peer {
        return Err(AppError::Crypto(
            "Relay authority key does not match the connected relay".into(),
        ));
    }
    let ed = public
        .try_into_ed25519()
        .map_err(|_| AppError::Crypto("Relay authority key is not Ed25519".into()))?;
    let claim = model(wire);
    RelayNamesRepository::new(&db).pin_key(
        &request.namespace,
        &claim.relay_key_id,
        &ed.to_bytes(),
        claim.not_before,
        Some(claim.not_after),
    )?;
    verify_and_cache(
        &RelayNamesRepository::new(&db),
        &claim,
        chrono::Utc::now().timestamp(),
    )
    .map_err(|e| AppError::Crypto(e.to_string()))?;
    accounts.update_verified_qualified_name(
        &claim.request.peer_id,
        &format!("@{}@{}", claim.request.local_name, claim.request.relay),
        claim.not_after,
    )?;
    Ok(claim.into())
}
#[tauri::command]
pub fn get_local_name_claim(
    db: State<'_, Arc<Database>>,
    identity: State<'_, Arc<IdentityService>>,
) -> Result<Option<NameClaimDto>> {
    crate::services::name_claim_service::verified_name_claim(
        &RelayNamesRepository::new(&db),
        &identity.get_peer_id()?,
        chrono::Utc::now().timestamp(),
    )
    .map(|claim| claim.map(|(claim, _)| claim.into()))
    .map_err(|error| AppError::Crypto(error.to_string()))
}
#[tauri::command]
pub fn verify_name_claim(claim: NameClaimDto, db: State<'_, Arc<Database>>) -> Result<bool> {
    let claim = NameClaim::from(claim);
    Ok(verify_and_cache(
        &RelayNamesRepository::new(&db),
        &claim,
        chrono::Utc::now().timestamp(),
    )
    .is_ok())
}

#[tauri::command]
pub fn apply_relay_key_rotation(
    signed_rotation: SignedRelayKeyRotation,
    db: State<'_, Arc<Database>>,
) -> Result<bool> {
    apply_signed_rotation(
        &RelayNamesRepository::new(&db),
        &signed_rotation,
        chrono::Utc::now().timestamp(),
    )
    .map_err(|error| AppError::Crypto(error.to_string()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityPublishingState {
    pub mode: String,
}

/// The complete, verified state needed by the frontend identity-entry gate.
///
/// Returning this as one command prevents the UI from making decisions from a
/// publishing-mode read and a separately verified claim read that can fail or
/// complete independently. A claim is only included after all user, relay-key,
/// peer-id, signature, and validity checks have passed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityEntryState {
    pub mode: String,
    pub claim: Option<NameClaimDto>,
}

fn publishing_mode(db: &Database, peer: &str) -> Result<String> {
    db.with_connection(|connection| {
        connection
            .query_row(
                "SELECT mode FROM identity_publishing_state WHERE peer_id=?",
                [peer],
                |row| row.get(0),
            )
            .or_else(|error| {
                if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
                    Ok("required".to_string())
                } else {
                    Err(error)
                }
            })
    })
    .map_err(Into::into)
}

fn validate_publishing_mode(mode: &str, has_verified_claim: bool) -> Result<()> {
    match (mode, has_verified_claim) {
        ("verified", true) | ("unverified", false) => Ok(()),
        ("verified", false) => Err(AppError::Validation(
            "A verified publishing mode requires an active relay name claim".into(),
        )),
        ("unverified", true) => Err(AppError::Validation(
            "A verified identity cannot be downgraded to unverified".into(),
        )),
        _ => Err(AppError::Validation("Invalid publishing mode".into())),
    }
}

#[tauri::command]
pub fn get_identity_entry_state(
    db: State<'_, Arc<Database>>,
    identity: State<'_, Arc<IdentityService>>,
    accounts: State<'_, Arc<AccountsService>>,
) -> Result<IdentityEntryState> {
    let peer = identity.get_peer_id()?;
    let stored_mode = publishing_mode(&db, &peer)?;
    let now = chrono::Utc::now().timestamp();
    let claim = crate::services::name_claim_service::verified_name_claim(
        &RelayNamesRepository::new(&db),
        &peer,
        now,
    )
    .map_err(|error| AppError::Crypto(error.to_string()))?
    .map(|(claim, _)| NameClaimDto::from(claim));
    let mode = if claim.is_some() {
        "verified".to_string()
    } else {
        stored_mode
    };

    if let Some(ref claim) = claim {
        accounts.update_verified_qualified_name(
            &peer,
            &format!("@{}@{}", claim.request.local_name, claim.request.relay),
            claim.not_after,
        )?;
    }

    Ok(IdentityEntryState { mode, claim })
}

#[tauri::command]
pub fn get_identity_publishing_state(
    db: State<'_, Arc<Database>>,
    identity: State<'_, Arc<IdentityService>>,
) -> Result<IdentityPublishingState> {
    let peer = identity.get_peer_id()?;
    let stored_mode = publishing_mode(&db, &peer)?;
    let has_verified_claim = crate::services::name_claim_service::verified_name_claim(
        &RelayNamesRepository::new(&db),
        &peer,
        chrono::Utc::now().timestamp(),
    )
    .map_err(|error| AppError::Crypto(error.to_string()))?
    .is_some();
    let mode = if has_verified_claim {
        "verified".to_string()
    } else {
        stored_mode
    };
    Ok(IdentityPublishingState { mode })
}
#[tauri::command]
pub fn set_identity_publishing_mode(
    mode: String,
    db: State<'_, Arc<Database>>,
    identity: State<'_, Arc<IdentityService>>,
) -> Result<()> {
    let peer = identity.get_peer_id()?;
    let has_verified_claim = crate::services::name_claim_service::verified_name_claim(
        &RelayNamesRepository::new(&db),
        &peer,
        chrono::Utc::now().timestamp(),
    )
    .map_err(|error| AppError::Crypto(error.to_string()))?
    .is_some();
    validate_publishing_mode(&mode, has_verified_claim)?;
    db.with_connection(|c|c.execute("INSERT INTO identity_publishing_state VALUES(?,?,?) ON CONFLICT(peer_id) DO UPDATE SET mode=excluded.mode,updated_at=excluded.updated_at",rusqlite::params![peer,mode,chrono::Utc::now().timestamp()]).map(|_|()))?;
    Ok(())
}

#[tauri::command]
pub async fn drain_private_mention_outbox(
    network: State<'_, NetworkState>,
    db: State<'_, Arc<Database>>,
) -> Result<u32> {
    let relay = network.get_handle().await?.active_relay().await?;
    let queued =
        MentionsRepository::new(&db).queued_outbound(chrono::Utc::now().timestamp(), 25)?;
    let mut delivered = 0;
    for item in queued {
        let mut delay = 200u64;
        for attempt in 0..3 {
            match network
                .get_handle()
                .await?
                .submit_introduction(
                    relay,
                    item.target.clone(),
                    item.mention_id.clone(),
                    item.ephemeral_public_key.clone(),
                    item.ciphertext.clone(),
                    item.expires_at,
                )
                .await
            {
                Ok((id, _)) if id == item.mention_id => {
                    if MentionsRepository::new(&db).mark_delivered(&id)? {
                        delivered += 1
                    }
                    break;
                }
                _ if attempt < 2 => {
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    delay *= 2
                }
                _ => break,
            }
        }
    }
    Ok(delivered)
}

#[cfg(test)]
mod dto_tests {
    use super::*;

    fn claim() -> NameClaim {
        NameClaim {
            request: crate::models::NameClaimRequest {
                domain: "harbor/name-claim-request/1".into(),
                version: 1,
                local_name: "alice".into(),
                relay: "harbor.social".into(),
                peer_id: "12D3KooWTest".into(),
                ed25519_public_key: vec![1; 32],
                x25519_public_key: vec![2; 32],
                sequence: 1,
                issued_at: 100,
                nonce: vec![3; 32],
            },
            user_signature: vec![4; 64],
            status: "active".into(),
            not_before: 100,
            not_after: 200,
            relay_key_id: "key-1".into(),
            relay_signature: vec![5; 64],
        }
    }

    #[test]
    fn tauri_name_claim_dto_uses_camel_case_without_changing_protocol_model() {
        let protocol = claim();
        let dto = NameClaimDto::from(protocol.clone());
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["request"]["localName"], "alice");
        assert_eq!(json["request"]["peerId"], "12D3KooWTest");
        assert!(json["request"].get("local_name").is_none());
        assert_eq!(NameClaim::from(dto), protocol);
    }

    #[test]
    fn identity_entry_mode_defaults_to_required_and_restores_persisted_state() {
        let db = Database::in_memory().unwrap();
        assert_eq!(publishing_mode(&db, "peer-returning").unwrap(), "required");

        db.with_connection(|connection| {
            connection.execute(
                "INSERT INTO identity_publishing_state(peer_id, mode, updated_at) VALUES(?,?,?)",
                rusqlite::params!["peer-returning", "verified", 123],
            )
        })
        .unwrap();

        assert_eq!(publishing_mode(&db, "peer-returning").unwrap(), "verified");
    }

    #[test]
    fn publishing_modes_cannot_spoof_or_downgrade_verified_identity() {
        assert!(validate_publishing_mode("unverified", false).is_ok());
        assert!(validate_publishing_mode("verified", true).is_ok());
        assert!(validate_publishing_mode("verified", false).is_err());
        assert!(validate_publishing_mode("unverified", true).is_err());
        assert!(validate_publishing_mode("compatibility", false).is_err());
    }
}
