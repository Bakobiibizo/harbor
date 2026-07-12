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
        IdentityService,
    },
};
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

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
) -> Result<NameClaim> {
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
    Ok(claim)
}
#[tauri::command]
pub fn get_local_name_claim(
    db: State<'_, Arc<Database>>,
    identity: State<'_, Arc<IdentityService>>,
) -> Result<Option<NameClaim>> {
    let Some(bytes) = RelayNamesRepository::new(&db)
        .active_for_peer(&identity.get_peer_id()?, chrono::Utc::now().timestamp())?
    else {
        return Ok(None);
    };
    ciborium::de::from_reader(bytes.as_slice())
        .map(Some)
        .map_err(|e| AppError::Serialization(e.to_string()))
}
#[tauri::command]
pub fn verify_name_claim(claim: NameClaim, db: State<'_, Arc<Database>>) -> Result<bool> {
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
pub struct IdentityMigrationState {
    pub mode: String,
}
#[tauri::command]
pub fn get_identity_migration_state(
    db: State<'_, Arc<Database>>,
    identity: State<'_, Arc<IdentityService>>,
) -> Result<IdentityMigrationState> {
    let peer = identity.get_peer_id()?;
    let mode = db.with_connection(|c| {
        c.query_row(
            "SELECT mode FROM identity_migration_state WHERE peer_id=?",
            [&peer],
            |r| r.get(0),
        )
        .or_else(|e| {
            if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
                Ok("required".to_string())
            } else {
                Err(e)
            }
        })
    })?;
    Ok(IdentityMigrationState { mode })
}
#[tauri::command]
pub fn set_identity_migration_mode(
    mode: String,
    db: State<'_, Arc<Database>>,
    identity: State<'_, Arc<IdentityService>>,
) -> Result<()> {
    if mode != "compatibility" && mode != "verified" {
        return Err(AppError::Validation("Invalid migration mode".into()));
    }
    let peer = identity.get_peer_id()?;
    db.with_connection(|c|c.execute("INSERT INTO identity_migration_state VALUES(?,?,?) ON CONFLICT(peer_id) DO UPDATE SET mode=excluded.mode,updated_at=excluded.updated_at",rusqlite::params![peer,mode,chrono::Utc::now().timestamp()]).map(|_|()))?;
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
