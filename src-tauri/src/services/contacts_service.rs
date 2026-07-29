//! Contacts service for managing peer relationships

use crate::db::{
    Contact, ContactData, ContactRequestRecord, ContactRequestsRepository, ContactsRepository,
    Database,
};
use crate::error::{AppError, Result};
use crate::services::{
    IdentityService, PermissionGrantMessage, PermissionRevokeMessage, Signable,
    SignablePermissionRevoke,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum ContactAcceptanceFailpoint {
    AfterContact,
    AfterGrant(usize),
    AfterRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContactRevocationAction {
    Blocked,
    Removed,
}

impl ContactRevocationAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Blocked => "blocked",
            Self::Removed => "removed",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactRevocationResult {
    pub event_id: String,
    pub request_id: String,
    pub peer_id: String,
    pub action: ContactRevocationAction,
}

#[derive(Debug, Clone)]
pub struct ContactRevocationOutboxEntry {
    pub event_id: String,
    pub request_id: String,
    pub peer_id: String,
    pub action: ContactRevocationAction,
    pub revocations: Vec<PermissionRevokeMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingContactProfile {
    pub peer_id: String,
    pub revision: u64,
    pub avatar_hash: String,
    pub avatar_mime_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum ContactRevocationFailpoint {
    AfterRelationship,
    AfterPermissions,
    AfterRequests,
    AfterDependentState,
    AfterOutbox,
}

/// Service for managing contacts
pub struct ContactsService {
    db: Arc<Database>,
    identity_service: Arc<IdentityService>,
}

impl ContactsService {
    #[allow(clippy::too_many_arguments)]
    pub fn record_contact_request(
        &self,
        request_id: &str,
        peer_id: &str,
        direction: &str,
        display_name: Option<&str>,
        public_key: Option<&[u8]>,
        x25519_public: Option<&[u8]>,
        avatar_hash: Option<&str>,
        bio: Option<&str>,
        status: &str,
        pending_action: Option<&str>,
        error: Option<&str>,
        at: i64,
    ) -> Result<()> {
        ContactRequestsRepository::new(&self.db)
            .upsert(
                request_id,
                peer_id,
                direction,
                display_name,
                public_key,
                x25519_public,
                avatar_hash,
                bio,
                status,
                pending_action,
                error,
                at,
            )
            .map_err(|error| AppError::DatabaseString(error.to_string()))
    }

    pub fn contact_requests(&self) -> Result<Vec<ContactRequestRecord>> {
        ContactRequestsRepository::new(&self.db)
            .list()
            .map_err(|error| AppError::DatabaseString(error.to_string()))
    }

    pub fn contact_request(&self, request_id: &str) -> Result<Option<ContactRequestRecord>> {
        ContactRequestsRepository::new(&self.db)
            .get(request_id)
            .map_err(|error| AppError::DatabaseString(error.to_string()))
    }

    pub fn contact_request_for_peer(
        &self,
        peer_id: &str,
        direction: &str,
    ) -> Result<Option<ContactRequestRecord>> {
        ContactRequestsRepository::new(&self.db)
            .for_peer(peer_id, direction)
            .map_err(|error| AppError::DatabaseString(error.to_string()))
    }

    pub fn update_contact_request(
        &self,
        request_id: &str,
        status: &str,
        pending_action: Option<&str>,
        error: Option<&str>,
        at: i64,
    ) -> Result<bool> {
        ContactRequestsRepository::new(&self.db)
            .update_status(request_id, status, pending_action, error, at)
            .map_err(|error| AppError::DatabaseString(error.to_string()))
    }

    pub fn promote_contact_request(&self, request_id: &str) -> Result<i64> {
        let request = self
            .contact_request(request_id)?
            .ok_or_else(|| AppError::NotFound("Contact request not found".into()))?;
        let public_key = request.public_key.ok_or_else(|| {
            AppError::Validation("Contact request has no verified signing key".into())
        })?;
        let x25519_public = request.x25519_public.ok_or_else(|| {
            AppError::Validation("Contact request has no verified encryption key".into())
        })?;
        self.add_contact(
            &request.peer_id,
            &public_key,
            &x25519_public,
            request.display_name.as_deref().unwrap_or("Harbor contact"),
            request.avatar_hash.as_deref(),
            request.bio.as_deref(),
        )
    }

    /// Atomically materialize the trusted contact, every signed capability
    /// grant, and the accepted request state. Nothing outside this transaction
    /// may emit ContactAdded or report acceptance.
    pub fn accept_contact_request_atomically(
        &self,
        request_id: &str,
        expected_direction: &str,
        contact: &ContactData,
        grants: &[PermissionGrantMessage],
        at: i64,
    ) -> Result<i64> {
        self.commit_contact_acceptance(
            Some((request_id, expected_direction)),
            contact,
            grants,
            at,
            None,
        )
    }

    /// Atomically add a contact discovered from an invite together with all
    /// locally issued capabilities. Transport/dial state is intentionally not
    /// part of this durable result.
    pub fn add_contact_with_grants_atomically(
        &self,
        contact: &ContactData,
        grants: &[PermissionGrantMessage],
        at: i64,
    ) -> Result<i64> {
        self.commit_contact_acceptance(None, contact, grants, at, None)
    }

    fn commit_contact_acceptance(
        &self,
        request: Option<(&str, &str)>,
        contact: &ContactData,
        grants: &[PermissionGrantMessage],
        at: i64,
        #[cfg_attr(not(test), allow(unused_variables))] failpoint: Option<
            ContactAcceptanceFailpoint,
        >,
    ) -> Result<i64> {
        if let Some(identity) = self.identity_service.get_identity()? {
            if identity.peer_id == contact.peer_id {
                return Err(AppError::Validation("Cannot add self as contact".into()));
            }
        }
        let local_peer_id = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".into()))?
            .peer_id;
        if grants.is_empty() {
            return Err(AppError::Validation(
                "Contact acceptance requires signed capability grants".into(),
            ));
        }
        for grant in grants {
            let direction_is_valid = (grant.issuer_peer_id == local_peer_id
                && grant.subject_peer_id == contact.peer_id)
                || (grant.issuer_peer_id == contact.peer_id
                    && grant.subject_peer_id == local_peer_id);
            if !direction_is_valid {
                return Err(AppError::Unauthorized(
                    "Contact acceptance contains a grant for another relationship".into(),
                ));
            }
            if crate::db::Capability::from_str(&grant.capability).is_none() {
                return Err(AppError::Validation(format!(
                    "Unknown contact capability: {}",
                    grant.capability
                )));
            }
        }
        let issuers: std::collections::HashSet<_> = grants
            .iter()
            .map(|grant| grant.issuer_peer_id.as_str())
            .collect();
        for issuer in issuers {
            for required in ["chat", "wall_read", "call"] {
                if !grants
                    .iter()
                    .any(|grant| grant.issuer_peer_id == issuer && grant.capability == required)
                {
                    return Err(AppError::Validation(format!(
                        "Contact acceptance from {issuer} is missing {required} capability"
                    )));
                }
            }
        }

        self.db.with_connection_mut_result(|connection| {
            let tx = connection
                .transaction()
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;

            if let Some((request_id, expected_direction)) = request {
                let stored: Option<(String, String, String)> = tx
                    .query_row(
                        "SELECT peer_id,direction,status FROM contact_requests WHERE request_id=?",
                        [request_id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .optional()
                    .map_err(|error| AppError::DatabaseString(error.to_string()))?;
                let Some((peer_id, direction, status)) = stored else {
                    return Err(AppError::NotFound("Contact request not found".into()));
                };
                if peer_id != contact.peer_id || direction != expected_direction {
                    return Err(AppError::Validation(
                        "Contact acceptance does not match the durable request".into(),
                    ));
                }
                let allowed = match expected_direction {
                    "incoming" => matches!(status.as_str(), "review" | "failed" | "accepted"),
                    "outgoing" => matches!(
                        status.as_str(),
                        "pending" | "failed" | "accepted" | "review"
                    ),
                    _ => false,
                };
                if !allowed {
                    return Err(AppError::Validation(
                        "Contact request is no longer eligible for acceptance".into(),
                    ));
                }
            }

            tx.execute(
                "INSERT INTO contacts(
                    peer_id,public_key,x25519_public,display_name,avatar_hash,bio,added_at,updated_at
                 ) VALUES(?,?,?,?,?,?,?,?)
                 ON CONFLICT(peer_id) DO UPDATE SET
                    public_key=excluded.public_key,
                    x25519_public=excluded.x25519_public,
                    display_name=excluded.display_name,
                    avatar_hash=excluded.avatar_hash,
                    bio=excluded.bio,
                    is_blocked=0,
                    updated_at=excluded.updated_at",
                params![
                    contact.peer_id,
                    contact.public_key,
                    contact.x25519_public,
                    contact.display_name,
                    contact.avatar_hash,
                    contact.bio,
                    at,
                    at,
                ],
            )
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            tx.execute(
                "DELETE FROM contact_revocation_tombstones WHERE peer_id=?",
                [&contact.peer_id],
            )
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;

            #[cfg(test)]
            if failpoint == Some(ContactAcceptanceFailpoint::AfterContact) {
                return Err(AppError::Internal("injected after contact".into()));
            }

            let local_grants: Vec<_> = grants
                .iter()
                .filter(|grant| grant.issuer_peer_id == local_peer_id)
                .collect();
            if let Some(first) = local_grants.first() {
                let current: i64 = tx
                    .query_row(
                        "SELECT current_value FROM lamport_clocks WHERE author_peer_id=?",
                        [&local_peer_id],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                let mut clocks: Vec<i64> = local_grants
                    .iter()
                    .map(|grant| grant.lamport_clock as i64)
                    .collect();
                clocks.sort_unstable();
                let already_committed = grants.iter().all(|grant| {
                    tx.query_row(
                        "SELECT 1 FROM permission_events WHERE event_id=?",
                        [format!("grant:{}", grant.grant_id)],
                        |_| Ok(()),
                    )
                    .optional()
                    .ok()
                    .flatten()
                    .is_some()
                });
                if !already_committed
                    && (clocks.first().copied() != Some(current + 1)
                        || clocks
                            .windows(2)
                            .any(|window| window[1] != window[0] + 1))
                {
                    return Err(AppError::DatabaseString(format!(
                        "Capability clock changed while accepting contact {}",
                        first.subject_peer_id
                    )));
                }
            }

            for (_index, grant) in grants.iter().enumerate() {
                let scope_json = grant.scope.as_ref().map(|scope| scope.to_string());
                tx.execute(
                    "INSERT INTO permissions_current(
                        grant_id,issuer_peer_id,subject_peer_id,capability,issued_at,expires_at,
                        payload_cbor,signature
                     ) VALUES(?,?,?,?,?,?,?,?)
                     ON CONFLICT(grant_id) DO UPDATE SET
                        expires_at=excluded.expires_at,
                        payload_cbor=excluded.payload_cbor,
                        signature=excluded.signature",
                    params![
                        grant.grant_id,
                        grant.issuer_peer_id,
                        grant.subject_peer_id,
                        grant.capability,
                        grant.issued_at,
                        grant.expires_at,
                        grant.payload_cbor,
                        grant.signature,
                    ],
                )
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
                tx.execute(
                    "INSERT OR IGNORE INTO permission_events(
                        event_id,event_type,entity_id,author_peer_id,issuer_peer_id,subject_peer_id,
                        capability,scope_json,lamport_clock,issued_at,expires_at,payload_cbor,
                        signature,received_at
                     ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                    params![
                        format!("grant:{}", grant.grant_id),
                        "grant",
                        grant.grant_id,
                        grant.issuer_peer_id,
                        grant.issuer_peer_id,
                        grant.subject_peer_id,
                        grant.capability,
                        scope_json,
                        grant.lamport_clock as i64,
                        grant.issued_at,
                        grant.expires_at,
                        grant.payload_cbor,
                        grant.signature,
                        at,
                    ],
                )
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
                tx.execute(
                    "INSERT INTO lamport_clocks(author_peer_id,current_value) VALUES(?,?)
                     ON CONFLICT(author_peer_id) DO UPDATE SET
                       current_value=MAX(current_value,excluded.current_value)",
                    params![grant.issuer_peer_id, grant.lamport_clock as i64],
                )
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;

                #[cfg(test)]
                if failpoint == Some(ContactAcceptanceFailpoint::AfterGrant(_index)) {
                    return Err(AppError::Internal(format!("injected after grant {_index}")));
                }
            }

            if let Some((request_id, _)) = request {
                let changed = tx
                    .execute(
                        "UPDATE contact_requests
                         SET status='accepted',pending_action='accepted',error=NULL,updated_at=?
                         WHERE request_id=?",
                        params![at, request_id],
                    )
                    .map_err(|error| AppError::DatabaseString(error.to_string()))?;
                if changed != 1 {
                    return Err(AppError::NotFound("Contact request not found".into()));
                }
            }

            #[cfg(test)]
            if failpoint == Some(ContactAcceptanceFailpoint::AfterRequest) {
                return Err(AppError::Internal("injected after request".into()));
            }

            let contact_id: i64 = tx
                .query_row(
                    "SELECT id FROM contacts WHERE peer_id=?",
                    [&contact.peer_id],
                    |row| row.get(0),
                )
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            tx.commit()
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            Ok(contact_id)
        })
    }

    #[cfg(test)]
    pub(crate) fn accept_contact_request_with_failpoint(
        &self,
        request_id: &str,
        expected_direction: &str,
        contact: &ContactData,
        grants: &[PermissionGrantMessage],
        at: i64,
        failpoint: ContactAcceptanceFailpoint,
    ) -> Result<i64> {
        self.commit_contact_acceptance(
            Some((request_id, expected_direction)),
            contact,
            grants,
            at,
            Some(failpoint),
        )
    }

    pub fn revoke_contact_requests(&self, peer_id: &str, at: i64) -> Result<usize> {
        ContactRequestsRepository::new(&self.db)
            .revoke_peer(peer_id, at)
            .map_err(|error| AppError::DatabaseString(error.to_string()))
    }

    /// Atomically revoke a relationship and every local capability derived
    /// from it. Network delivery is deliberately post-commit and retries from
    /// `contact_revocation_outbox`; local denial never depends on connectivity.
    pub fn revoke_contact_atomically(
        &self,
        peer_id: &str,
        action: ContactRevocationAction,
        revocations: &[PermissionRevokeMessage],
        at: i64,
    ) -> Result<ContactRevocationResult> {
        self.commit_contact_revocation(peer_id, action, revocations, at, true, None)
    }

    pub fn apply_incoming_contact_revocation_atomically(
        &self,
        peer_id: &str,
        revocations: &[PermissionRevokeMessage],
        at: i64,
    ) -> Result<ContactRevocationResult> {
        self.commit_contact_revocation(
            peer_id,
            ContactRevocationAction::Removed,
            revocations,
            at,
            false,
            None,
        )
    }

    fn commit_contact_revocation(
        &self,
        peer_id: &str,
        action: ContactRevocationAction,
        revocations: &[PermissionRevokeMessage],
        at: i64,
        enqueue_delivery: bool,
        #[cfg_attr(not(test), allow(unused_variables))] failpoint: Option<
            ContactRevocationFailpoint,
        >,
    ) -> Result<ContactRevocationResult> {
        let local_peer_id = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".into()))?
            .peer_id;
        if peer_id == local_peer_id {
            return Err(AppError::Validation("Cannot revoke self".into()));
        }
        for revoke in revocations {
            let expected_issuer = if enqueue_delivery {
                &local_peer_id
            } else {
                peer_id
            };
            if revoke.issuer_peer_id != *expected_issuer {
                return Err(AppError::Unauthorized(
                    "Contact revocation contains an unexpected issuer".into(),
                ));
            }
        }
        let event_id = Uuid::new_v4().to_string();
        let request_id = Uuid::new_v4().to_string();
        let mut payload_cbor = Vec::new();
        ciborium::ser::into_writer(revocations, &mut payload_cbor)
            .map_err(|error| AppError::Serialization(error.to_string()))?;

        self.db.with_connection_mut_result(|connection| {
            let tx = connection
                .transaction()
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            let exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM contacts WHERE peer_id=?)",
                    [peer_id],
                    |row| row.get(0),
                )
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            if !exists && enqueue_delivery {
                return Err(AppError::NotFound("Contact not found".into()));
            }
            match action {
                ContactRevocationAction::Blocked => {
                    tx.execute(
                        "UPDATE contacts SET is_blocked=1,updated_at=? WHERE peer_id=?",
                        params![at, peer_id],
                    )
                    .map_err(|error| AppError::DatabaseString(error.to_string()))?;
                }
                ContactRevocationAction::Removed => {
                    tx.execute("DELETE FROM contacts WHERE peer_id=?", [peer_id])
                        .map_err(|error| AppError::DatabaseString(error.to_string()))?;
                }
            }
            tx.execute(
                "INSERT INTO contact_revocation_tombstones(peer_id,action,revoked_at)
                 VALUES(?,?,?) ON CONFLICT(peer_id) DO UPDATE SET
                   action=excluded.action,revoked_at=excluded.revoked_at",
                params![peer_id, action.as_str(), at],
            )
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;

            #[cfg(test)]
            if failpoint == Some(ContactRevocationFailpoint::AfterRelationship) {
                return Err(AppError::Internal("injected after relationship".into()));
            }

            for revoke in revocations {
                let grant: Option<(String, String, String)> = tx
                    .query_row(
                        "SELECT issuer_peer_id,subject_peer_id,capability
                         FROM permissions_current WHERE grant_id=?",
                        [&revoke.grant_id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .optional()
                    .map_err(|error| AppError::DatabaseString(error.to_string()))?;
                let Some((issuer, subject, capability)) = grant else {
                    if enqueue_delivery {
                        return Err(AppError::NotFound("Revoked grant not found".into()));
                    }
                    // The signed relationship-level action remains authoritative
                    // when this device has an incomplete grant cache.
                    continue;
                };
                let direction_is_valid = if enqueue_delivery {
                    issuer == local_peer_id && subject == peer_id
                } else {
                    issuer == peer_id && subject == local_peer_id
                };
                if !direction_is_valid {
                    if !enqueue_delivery {
                        continue;
                    }
                    return Err(AppError::Unauthorized(
                        "Contact revocation does not match the relationship".into(),
                    ));
                }
                let signable = SignablePermissionRevoke {
                    grant_id: revoke.grant_id.clone(),
                    issuer_peer_id: revoke.issuer_peer_id.clone(),
                    lamport_clock: revoke.lamport_clock,
                    revoked_at: revoke.revoked_at,
                };
                tx.execute(
                    "UPDATE permissions_current SET revoked_at=? WHERE grant_id=?",
                    params![revoke.revoked_at, revoke.grant_id],
                )
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
                tx.execute(
                    "INSERT OR IGNORE INTO permission_events(
                       event_id,event_type,entity_id,author_peer_id,issuer_peer_id,
                       subject_peer_id,capability,scope_json,lamport_clock,issued_at,
                       expires_at,payload_cbor,signature,received_at
                     ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                    params![
                        format!("revoke:{}:{}", revoke.grant_id, revoke.lamport_clock),
                        "revoke",
                        revoke.grant_id,
                        revoke.issuer_peer_id,
                        revoke.issuer_peer_id,
                        subject,
                        capability,
                        Option::<String>::None,
                        revoke.lamport_clock as i64,
                        Option::<i64>::None,
                        Option::<i64>::None,
                        signable.signable_bytes()?,
                        revoke.signature,
                        at,
                    ],
                )
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
                tx.execute(
                    "INSERT INTO lamport_clocks(author_peer_id,current_value) VALUES(?,?)
                     ON CONFLICT(author_peer_id) DO UPDATE SET
                       current_value=MAX(current_value,excluded.current_value)",
                    params![revoke.issuer_peer_id, revoke.lamport_clock as i64],
                )
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            }
            // Inbound grants and any unrecognized future capabilities are also
            // denied locally; only locally issued grants need signed envelopes.
            tx.execute(
                "UPDATE permissions_current SET revoked_at=COALESCE(revoked_at,?1)
                 WHERE (issuer_peer_id=?3 AND subject_peer_id=?2)
                    OR (issuer_peer_id=?2 AND subject_peer_id=?3)",
                params![at, peer_id, local_peer_id],
            )
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;

            #[cfg(test)]
            if failpoint == Some(ContactRevocationFailpoint::AfterPermissions) {
                return Err(AppError::Internal("injected after permissions".into()));
            }

            tx.execute(
                "UPDATE contact_requests SET status='revoked',pending_action='revoked',
                   error=NULL,updated_at=? WHERE peer_id=?",
                params![at, peer_id],
            )
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;

            #[cfg(test)]
            if failpoint == Some(ContactRevocationFailpoint::AfterRequests) {
                return Err(AppError::Internal("injected after requests".into()));
            }

            tx.execute(
                "UPDATE messages SET status='failed' WHERE message_id IN (
                   SELECT message_id FROM direct_message_outbox
                   WHERE peer_id=? AND state IN ('queued','in_flight','sent')
                 )",
                [peer_id],
            )
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            tx.execute(
                "UPDATE direct_message_outbox SET state='canceled',attempt_deadline_at=NULL,
                   last_error='relationship revoked',updated_at=?,terminal_at=?
                 WHERE peer_id=? AND state IN ('queued','in_flight','sent')",
                params![at, at, peer_id],
            )
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            tx.execute("DELETE FROM sync_queue WHERE target_peer_id=?", [peer_id])
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            tx.execute("DELETE FROM sync_cursors WHERE source_peer_id=?", [peer_id])
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            tx.execute(
                "DELETE FROM media_cache_sources WHERE source_peer_id=?",
                [peer_id],
            )
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            tx.execute(
                "UPDATE media_cache_entries SET retain_until=COALESCE(
                   (SELECT MAX(source.retain_until) FROM media_cache_sources source
                    WHERE source.media_hash=media_cache_entries.media_hash), observed_at)",
                [],
            )
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            tx.execute(
                "UPDATE call_history SET status='ended',terminal_reason='contact_revoked',
                   ended_at=COALESCE(ended_at,?),updated_at=?
                 WHERE peer_id=? AND status != 'ended'",
                params![at, at, peer_id],
            )
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;

            #[cfg(test)]
            if failpoint == Some(ContactRevocationFailpoint::AfterDependentState) {
                return Err(AppError::Internal("injected after dependent state".into()));
            }

            if enqueue_delivery {
                tx.execute(
                    "INSERT INTO contact_revocation_outbox(
                       event_id,request_id,peer_id,action,payload_cbor,state,attempt_count,
                       max_attempts,next_attempt_at,created_at,updated_at
                     ) VALUES(?,?,?,?,?,'queued',0,2147483647,?,?,?)",
                    params![
                        event_id,
                        request_id,
                        peer_id,
                        action.as_str(),
                        payload_cbor,
                        at,
                        at,
                        at,
                    ],
                )
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            }

            #[cfg(test)]
            if failpoint == Some(ContactRevocationFailpoint::AfterOutbox) {
                return Err(AppError::Internal("injected after outbox".into()));
            }

            tx.commit()
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            Ok(ContactRevocationResult {
                event_id: event_id.clone(),
                request_id: request_id.clone(),
                peer_id: peer_id.to_string(),
                action,
            })
        })
    }

    #[cfg(test)]
    pub(crate) fn revoke_contact_with_failpoint(
        &self,
        peer_id: &str,
        action: ContactRevocationAction,
        revocations: &[PermissionRevokeMessage],
        at: i64,
        failpoint: ContactRevocationFailpoint,
    ) -> Result<ContactRevocationResult> {
        self.commit_contact_revocation(peer_id, action, revocations, at, true, Some(failpoint))
    }

    pub fn claim_due_contact_revocations(
        &self,
        now: i64,
        lease_seconds: i64,
        limit: u32,
    ) -> Result<Vec<ContactRevocationOutboxEntry>> {
        self.db.with_connection_mut_result(|connection| {
            let tx = connection
                .transaction()
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            tx.execute(
                "UPDATE contact_revocation_outbox SET state='queued',attempt_deadline_at=NULL,
                   next_attempt_at=?,last_error='delivery lease expired',updated_at=?
                 WHERE state='in_flight' AND attempt_deadline_at <= ?",
                params![now, now, now],
            )
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            let rows: Vec<(String, String, String, String, Vec<u8>)> = {
                let mut statement = tx
                    .prepare(
                        "SELECT event_id,request_id,peer_id,action,payload_cbor
                         FROM contact_revocation_outbox
                         WHERE state='queued' AND next_attempt_at <= ?
                           AND attempt_count < max_attempts
                         ORDER BY created_at LIMIT ?",
                    )
                    .map_err(|error| AppError::DatabaseString(error.to_string()))?;
                let mapped = statement
                    .query_map(params![now, limit], |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    })
                    .map_err(|error| AppError::DatabaseString(error.to_string()))?;
                let collected = mapped
                    .collect::<std::result::Result<_, _>>()
                    .map_err(|error| AppError::DatabaseString(error.to_string()))?;
                collected
            };
            let mut entries = Vec::with_capacity(rows.len());
            for (event_id, request_id, peer_id, action, payload) in rows {
                tx.execute(
                    "UPDATE contact_revocation_outbox SET state='in_flight',
                       attempt_count=attempt_count+1,attempt_deadline_at=?,updated_at=?
                     WHERE event_id=? AND state='queued'",
                    params![now + lease_seconds, now, event_id],
                )
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
                let revocations = ciborium::de::from_reader(payload.as_slice())
                    .map_err(|error| AppError::Serialization(error.to_string()))?;
                let action = match action.as_str() {
                    "blocked" => ContactRevocationAction::Blocked,
                    "removed" => ContactRevocationAction::Removed,
                    _ => return Err(AppError::Validation("Invalid revocation action".into())),
                };
                entries.push(ContactRevocationOutboxEntry {
                    event_id,
                    request_id,
                    peer_id,
                    action,
                    revocations,
                });
            }
            tx.commit()
                .map_err(|error| AppError::DatabaseString(error.to_string()))?;
            Ok(entries)
        })
    }

    pub fn mark_contact_revocation_delivered(&self, request_id: &str, now: i64) -> Result<bool> {
        self.db
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE contact_revocation_outbox SET state='delivered',
                   attempt_deadline_at=NULL,last_error=NULL,updated_at=?,terminal_at=?
                 WHERE request_id=? AND state='in_flight'",
                    params![now, now, request_id],
                )
            })
            .map(|changed| changed > 0)
            .map_err(|error| AppError::DatabaseString(error.to_string()))
    }

    pub fn retry_contact_revocation(
        &self,
        request_id: &str,
        error: &str,
        now: i64,
    ) -> Result<bool> {
        self.db
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE contact_revocation_outbox SET
                   state=CASE WHEN attempt_count >= max_attempts THEN 'failed' ELSE 'queued' END,
                   attempt_deadline_at=NULL,last_error=?,updated_at=?,
                   next_attempt_at=? + MIN(300, (1 << MIN(attempt_count, 8))),
                   terminal_at=CASE WHEN attempt_count >= max_attempts THEN ? ELSE NULL END
                 WHERE request_id=? AND state='in_flight'",
                    params![error, now, now, now, request_id],
                )
            })
            .map(|changed| changed > 0)
            .map_err(|error| AppError::DatabaseString(error.to_string()))
    }
    pub fn verified_qualified_name(&self, peer_id: &str) -> Result<Option<String>> {
        let repo = crate::db::repositories::RelayNamesRepository::new(&self.db);
        crate::services::name_claim_service::verified_qualified_name(
            &repo,
            peer_id,
            chrono::Utc::now().timestamp(),
        )
        .map_err(|error| AppError::Crypto(error.to_string()))
    }
    /// Create a new contacts service
    pub fn new(db: Arc<Database>, identity_service: Arc<IdentityService>) -> Self {
        Self {
            db,
            identity_service,
        }
    }

    /// Add a new contact from identity exchange data
    pub fn add_contact(
        &self,
        peer_id: &str,
        public_key: &[u8],
        x25519_public: &[u8],
        display_name: &str,
        avatar_hash: Option<&str>,
        bio: Option<&str>,
    ) -> Result<i64> {
        // Don't add ourselves as a contact
        if let Some(identity) = self.identity_service.get_identity()? {
            if identity.peer_id == peer_id {
                return Err(AppError::Validation(
                    "Cannot add self as contact".to_string(),
                ));
            }
        }

        // Check if already a contact
        if ContactsRepository::is_contact(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
        {
            // Update existing contact info instead
            ContactsRepository::update_contact_info(
                &self.db,
                peer_id,
                display_name,
                avatar_hash,
                bio,
            )
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

            // Return existing contact's ID
            let contact = ContactsRepository::get_by_peer_id(&self.db, peer_id)
                .map_err(|e| AppError::DatabaseString(e.to_string()))?
                .ok_or_else(|| AppError::NotFound("Contact not found".to_string()))?;
            return Ok(contact.id);
        }

        let contact_data = ContactData {
            peer_id: peer_id.to_string(),
            public_key: public_key.to_vec(),
            x25519_public: x25519_public.to_vec(),
            display_name: display_name.to_string(),
            avatar_hash: avatar_hash.map(String::from),
            bio: bio.map(String::from),
        };

        ContactsRepository::add_contact(&self.db, &contact_data)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get a contact by peer ID
    pub fn get_contact(&self, peer_id: &str) -> Result<Option<Contact>> {
        ContactsRepository::get_by_peer_id(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get all contacts
    pub fn get_all_contacts(&self) -> Result<Vec<Contact>> {
        ContactsRepository::get_all(&self.db).map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get all non-blocked contacts
    pub fn get_active_contacts(&self) -> Result<Vec<Contact>> {
        ContactsRepository::get_active(&self.db)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Update contact info (from network)
    pub fn update_contact_info(
        &self,
        peer_id: &str,
        display_name: &str,
        avatar_hash: Option<&str>,
        bio: Option<&str>,
    ) -> Result<bool> {
        ContactsRepository::update_contact_info(&self.db, peer_id, display_name, avatar_hash, bio)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Stage a newer signed contact profile. Avatar-bearing revisions are not
    /// made visible until their content hash has been verified by media storage.
    pub fn stage_profile_update(
        &self,
        peer_id: &str,
        revision: u64,
        display_name: &str,
        avatar_hash: Option<&str>,
        avatar_mime_type: Option<&str>,
        bio: Option<&str>,
    ) -> Result<bool> {
        if revision == 0
            || revision > i64::MAX as u64
            || display_name.is_empty()
            || display_name.chars().count() > 128
            || avatar_hash.is_some_and(|hash| {
                hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            || avatar_mime_type.is_some_and(|mime| !mime.starts_with("image/") || mime.len() > 128)
        {
            return Err(AppError::Validation(
                "Invalid signed profile metadata".into(),
            ));
        }
        let now = chrono::Utc::now().timestamp();
        self.db
            .with_connection(|connection| {
                let transaction = connection.unchecked_transaction()?;
                let active: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM contacts WHERE peer_id = ? AND is_blocked = 0)",
                    [peer_id],
                    |row| row.get(0),
                )?;
                if !active {
                    return Ok(false);
                }
                let accepted: i64 = transaction
                    .query_row(
                        "SELECT revision FROM contact_profile_state WHERE peer_id = ?",
                        [peer_id],
                        |row| row.get(0),
                    )
                    .optional()?
                    .unwrap_or(0);
                let pending: i64 = transaction
                    .query_row(
                        "SELECT revision FROM pending_contact_profiles WHERE peer_id = ?",
                        [peer_id],
                        |row| row.get(0),
                    )
                    .optional()?
                    .unwrap_or(0);
                if revision <= accepted.max(pending) as u64 {
                    return Ok(false);
                }
                if let Some(hash) = avatar_hash {
                    transaction.execute(
                        "INSERT INTO pending_contact_profiles
                         (peer_id, revision, display_name, avatar_hash, avatar_mime_type, bio, received_at)
                         VALUES (?, ?, ?, ?, ?, ?, ?)
                         ON CONFLICT(peer_id) DO UPDATE SET revision=excluded.revision,
                           display_name=excluded.display_name, avatar_hash=excluded.avatar_hash,
                           avatar_mime_type=excluded.avatar_mime_type, bio=excluded.bio,
                           received_at=excluded.received_at",
                        params![peer_id, revision as i64, display_name, hash, avatar_mime_type, bio, now],
                    )?;
                } else {
                    transaction.execute(
                        "UPDATE contacts SET display_name=?, avatar_hash=NULL, bio=?, updated_at=? WHERE peer_id=?",
                        params![display_name, bio, now, peer_id],
                    )?;
                    transaction.execute(
                        "INSERT INTO contact_profile_state(peer_id, revision, avatar_mime_type, updated_at)
                         VALUES (?, ?, NULL, ?)
                         ON CONFLICT(peer_id) DO UPDATE SET revision=excluded.revision,
                           avatar_mime_type=NULL, updated_at=excluded.updated_at",
                        params![peer_id, revision as i64, now],
                    )?;
                    transaction.execute("DELETE FROM pending_contact_profiles WHERE peer_id=?", [peer_id])?;
                }
                transaction.commit()?;
                Ok(true)
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))
    }

    /// Promote only the staged revision that references verified bytes.
    pub fn promote_verified_profile_avatar(
        &self,
        peer_id: &str,
        avatar_hash: &str,
    ) -> Result<bool> {
        self.db
            .with_connection(|connection| {
                let transaction = connection.unchecked_transaction()?;
                let pending = transaction
                    .query_row(
                        "SELECT revision, display_name, avatar_mime_type, bio
                         FROM pending_contact_profiles WHERE peer_id=? AND avatar_hash=?",
                        params![peer_id, avatar_hash],
                        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, Option<String>>(3)?)),
                    )
                    .optional()?;
                let Some((revision, display_name, mime_type, bio)) = pending else {
                    return Ok(false);
                };
                let current: i64 = transaction
                    .query_row(
                        "SELECT revision FROM contact_profile_state WHERE peer_id=?",
                        [peer_id],
                        |row| row.get(0),
                    )
                    .optional()?
                    .unwrap_or(0);
                if revision <= current {
                    transaction.execute("DELETE FROM pending_contact_profiles WHERE peer_id=?", [peer_id])?;
                    transaction.commit()?;
                    return Ok(false);
                }
                let now = chrono::Utc::now().timestamp();
                transaction.execute(
                    "UPDATE contacts SET display_name=?, avatar_hash=?, bio=?, updated_at=? WHERE peer_id=? AND is_blocked=0",
                    params![display_name, avatar_hash, bio, now, peer_id],
                )?;
                transaction.execute(
                    "INSERT INTO contact_profile_state(peer_id, revision, avatar_mime_type, updated_at)
                     VALUES (?, ?, ?, ?)
                     ON CONFLICT(peer_id) DO UPDATE SET revision=excluded.revision,
                       avatar_mime_type=excluded.avatar_mime_type, updated_at=excluded.updated_at",
                    params![peer_id, revision, mime_type, now],
                )?;
                transaction.execute("DELETE FROM pending_contact_profiles WHERE peer_id=?", [peer_id])?;
                transaction.commit()?;
                Ok(true)
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))
    }

    pub fn pending_profile_avatars(&self) -> Result<Vec<PendingContactProfile>> {
        self.db
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT p.peer_id, p.revision, p.avatar_hash, p.avatar_mime_type
                     FROM pending_contact_profiles p
                     JOIN contacts c ON c.peer_id=p.peer_id
                     WHERE c.is_blocked=0 AND p.avatar_hash IS NOT NULL
                     ORDER BY p.received_at DESC LIMIT 512",
                )?;
                let rows = statement.query_map([], |row| {
                    Ok(PendingContactProfile {
                        peer_id: row.get(0)?,
                        revision: row.get::<_, i64>(1)? as u64,
                        avatar_hash: row.get(2)?,
                        avatar_mime_type: row.get(3)?,
                    })
                })?;
                rows.collect()
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))
    }

    /// Update last seen timestamp for a contact
    pub fn update_last_seen(&self, peer_id: &str) -> Result<bool> {
        ContactsRepository::update_last_seen(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Block a contact
    pub fn block_contact(&self, peer_id: &str) -> Result<bool> {
        ContactsRepository::block_contact(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Unblock a contact
    pub fn unblock_contact(&self, peer_id: &str) -> Result<bool> {
        let revoked = self
            .db
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM contact_revocation_tombstones WHERE peer_id=?)",
                    [peer_id],
                    |row| row.get::<_, bool>(0),
                )
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
        if revoked {
            return Err(AppError::Validation(
                "This contact was cryptographically revoked. Send a new contact request to restore access."
                    .into(),
            ));
        }
        ContactsRepository::unblock_contact(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Remove a contact
    pub fn remove_contact(&self, peer_id: &str) -> Result<bool> {
        ContactsRepository::remove_contact(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Check if peer is a contact
    pub fn is_contact(&self, peer_id: &str) -> Result<bool> {
        ContactsRepository::is_contact(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Check if peer is blocked
    pub fn is_blocked(&self, peer_id: &str) -> Result<bool> {
        ContactsRepository::is_blocked(&self.db, peer_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get X25519 public key for a contact (needed for encryption)
    pub fn get_x25519_public(&self, peer_id: &str) -> Result<Option<Vec<u8>>> {
        let contact = self.get_contact(peer_id)?;
        Ok(contact.map(|c| c.x25519_public))
    }

    /// Get Ed25519 public key for a contact (needed for signature verification)
    pub fn get_public_key(&self, peer_id: &str) -> Result<Option<Vec<u8>>> {
        let contact = self.get_contact(peer_id)?;
        Ok(contact.map(|c| c.public_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Capability;
    use crate::models::CreateIdentityRequest;
    use crate::services::PermissionsService;
    use std::sync::Arc;

    fn create_test_services() -> (Arc<Database>, Arc<IdentityService>, ContactsService) {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = ContactsService::new(db.clone(), identity_service.clone());
        (db, identity_service, contacts_service)
    }

    #[test]
    fn test_add_and_get_contact() {
        let (_, _, service) = create_test_services();

        let id = service
            .add_contact(
                "12D3KooWTest",
                &[1, 2, 3, 4],
                &[5, 6, 7, 8],
                "Test User",
                None,
                Some("Hello!"),
            )
            .unwrap();

        assert!(id > 0);

        let contact = service.get_contact("12D3KooWTest").unwrap().unwrap();
        assert_eq!(contact.display_name, "Test User");
        assert_eq!(contact.bio, Some("Hello!".to_string()));
    }

    #[test]
    fn test_block_contact() {
        let (_, _, service) = create_test_services();

        service
            .add_contact(
                "12D3KooWTest",
                &[1, 2, 3, 4],
                &[5, 6, 7, 8],
                "Test User",
                None,
                None,
            )
            .unwrap();

        assert!(!service.is_blocked("12D3KooWTest").unwrap());

        service.block_contact("12D3KooWTest").unwrap();
        assert!(service.is_blocked("12D3KooWTest").unwrap());

        // Blocked contacts shouldn't appear in active list
        let active = service.get_active_contacts().unwrap();
        assert!(active.is_empty());
    }

    #[test]
    fn acceptance_failure_injection_is_atomic_and_restart_converges() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("contact-acceptance.db");
        let remote_peer = "12D3KooWAtomicRemote";
        let request_id = "request-atomic";

        {
            let db = Arc::new(Database::new(path.clone()).unwrap());
            let identity = Arc::new(IdentityService::new(db.clone()));
            identity
                .create_identity(CreateIdentityRequest {
                    display_name: "Local".into(),
                    passphrase: "password123".into(),
                    bio: None,
                    passphrase_hint: None,
                })
                .unwrap();
            let contacts = ContactsService::new(db.clone(), identity.clone());
            let permissions = PermissionsService::new(db.clone(), identity);
            contacts
                .record_contact_request(
                    request_id,
                    remote_peer,
                    "incoming",
                    Some("Remote"),
                    Some(&[7; 32]),
                    Some(&[8; 32]),
                    None,
                    None,
                    "review",
                    None,
                    None,
                    10,
                )
                .unwrap();
            let grants = permissions
                .prepare_permission_grants(
                    remote_peer,
                    &[Capability::Chat, Capability::WallRead, Capability::Call],
                )
                .unwrap();
            let contact = ContactData {
                peer_id: remote_peer.into(),
                public_key: vec![7; 32],
                x25519_public: vec![8; 32],
                display_name: "Remote".into(),
                avatar_hash: None,
                bio: None,
            };

            let failpoints = [
                ContactAcceptanceFailpoint::AfterContact,
                ContactAcceptanceFailpoint::AfterGrant(0),
                ContactAcceptanceFailpoint::AfterGrant(1),
                ContactAcceptanceFailpoint::AfterGrant(2),
                ContactAcceptanceFailpoint::AfterRequest,
            ];
            for failpoint in failpoints {
                assert!(contacts
                    .accept_contact_request_with_failpoint(
                        request_id, "incoming", &contact, &grants, 20, failpoint,
                    )
                    .is_err());
                assert!(!contacts.is_contact(remote_peer).unwrap());
                assert_eq!(
                    contacts
                        .contact_request(request_id)
                        .unwrap()
                        .unwrap()
                        .status,
                    "review"
                );
                let (current, events): (i64, i64) = db
                    .with_connection(|connection| {
                        Ok((
                            connection
                                .query_row(
                                    "SELECT COALESCE(MAX(current_value),0) FROM lamport_clocks",
                                    [],
                                    |row| row.get(0),
                                )
                                .unwrap(),
                            connection.query_row(
                                "SELECT COUNT(*) FROM permission_events",
                                [],
                                |row| row.get(0),
                            )?,
                        ))
                    })
                    .unwrap();
                assert_eq!((current, events), (0, 0));
            }

            contacts
                .accept_contact_request_atomically(request_id, "incoming", &contact, &grants, 30)
                .unwrap();
            assert!(contacts.is_contact(remote_peer).unwrap());
            assert_eq!(
                contacts
                    .contact_request(request_id)
                    .unwrap()
                    .unwrap()
                    .status,
                "accepted"
            );
            for capability in [Capability::Chat, Capability::WallRead, Capability::Call] {
                assert!(permissions
                    .peer_has_capability(remote_peer, capability)
                    .unwrap());
            }

            // An acknowledgement replay is idempotent and cannot duplicate events.
            contacts
                .accept_contact_request_atomically(request_id, "incoming", &contact, &grants, 31)
                .unwrap();
            let events: i64 = db
                .with_connection(|connection| {
                    connection.query_row("SELECT COUNT(*) FROM permission_events", [], |row| {
                        row.get(0)
                    })
                })
                .unwrap();
            assert_eq!(events, 3);
        }

        let reopened_db = Arc::new(Database::new(path).unwrap());
        let reopened_identity = Arc::new(IdentityService::new(reopened_db.clone()));
        let reopened_contacts =
            ContactsService::new(reopened_db.clone(), reopened_identity.clone());
        let reopened_permissions = PermissionsService::new(reopened_db, reopened_identity);
        assert!(reopened_contacts.is_contact(remote_peer).unwrap());
        assert_eq!(
            reopened_contacts
                .contact_request(request_id)
                .unwrap()
                .unwrap()
                .status,
            "accepted"
        );
        for capability in [Capability::Chat, Capability::WallRead, Capability::Call] {
            assert!(reopened_permissions
                .peer_has_capability(remote_peer, capability)
                .unwrap());
        }
    }

    #[test]
    fn revocation_failure_injection_is_atomic_and_restart_denies_stale_grants() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("contact-revocation.db");
        let remote_peer = "12D3KooWRevokedRemote";
        {
            let db = Arc::new(Database::new(path.clone()).unwrap());
            let identity = Arc::new(IdentityService::new(db.clone()));
            identity
                .create_identity(CreateIdentityRequest {
                    display_name: "Local".into(),
                    passphrase: "password123".into(),
                    bio: None,
                    passphrase_hint: None,
                })
                .unwrap();
            let contacts = ContactsService::new(db.clone(), identity.clone());
            let permissions = PermissionsService::new(db.clone(), identity);
            let grants = permissions
                .prepare_permission_grants(
                    remote_peer,
                    &[Capability::Chat, Capability::WallRead, Capability::Call],
                )
                .unwrap();
            contacts
                .add_contact_with_grants_atomically(
                    &ContactData {
                        peer_id: remote_peer.into(),
                        public_key: vec![7; 32],
                        x25519_public: vec![8; 32],
                        display_name: "Remote".into(),
                        avatar_hash: None,
                        bio: None,
                    },
                    &grants,
                    10,
                )
                .unwrap();
            let revocations = permissions
                .prepare_contact_revocations(remote_peer)
                .unwrap();
            assert_eq!(revocations.len(), 3);

            for failpoint in [
                ContactRevocationFailpoint::AfterRelationship,
                ContactRevocationFailpoint::AfterPermissions,
                ContactRevocationFailpoint::AfterRequests,
                ContactRevocationFailpoint::AfterDependentState,
                ContactRevocationFailpoint::AfterOutbox,
            ] {
                assert!(contacts
                    .revoke_contact_with_failpoint(
                        remote_peer,
                        ContactRevocationAction::Blocked,
                        &revocations,
                        20,
                        failpoint,
                    )
                    .is_err());
                assert!(!contacts.is_blocked(remote_peer).unwrap());
                assert_eq!(
                    db.with_connection(|connection| connection.query_row(
                        "SELECT COUNT(*) FROM contact_revocation_tombstones",
                        [],
                        |row| row.get::<_, i64>(0),
                    ))
                    .unwrap(),
                    0
                );
                for capability in [Capability::Chat, Capability::WallRead, Capability::Call] {
                    assert!(permissions
                        .peer_has_capability(remote_peer, capability)
                        .unwrap());
                }
            }

            contacts
                .revoke_contact_atomically(
                    remote_peer,
                    ContactRevocationAction::Blocked,
                    &revocations,
                    30,
                )
                .unwrap();
            assert!(contacts.is_blocked(remote_peer).unwrap());
            let unblock_error = contacts.unblock_contact(remote_peer).unwrap_err();
            assert!(unblock_error
                .to_string()
                .contains("Send a new contact request"));
            assert!(contacts.is_blocked(remote_peer).unwrap());
            for capability in [Capability::Chat, Capability::WallRead, Capability::Call] {
                assert!(!permissions
                    .peer_has_capability(remote_peer, capability)
                    .unwrap());
            }
            let (tombstones, queued, active_grants): (i64, i64, i64) = db
                .with_connection(|connection| {
                    Ok((
                        connection.query_row(
                            "SELECT COUNT(*) FROM contact_revocation_tombstones",
                            [],
                            |row| row.get(0),
                        )?,
                        connection.query_row(
                            "SELECT COUNT(*) FROM contact_revocation_outbox WHERE state='queued'",
                            [],
                            |row| row.get(0),
                        )?,
                        connection.query_row(
                            "SELECT COUNT(*) FROM permissions_current WHERE revoked_at IS NULL",
                            [],
                            |row| row.get(0),
                        )?,
                    ))
                })
                .unwrap();
            assert_eq!((tombstones, queued, active_grants), (1, 1, 0));
            let claimed = contacts.claim_due_contact_revocations(31, 30, 8).unwrap();
            assert_eq!(claimed.len(), 1);
            assert_eq!(claimed[0].revocations.len(), 3);
            contacts
                .retry_contact_revocation(&claimed[0].request_id, "peer offline", 32)
                .unwrap();
        }

        let db = Arc::new(Database::new(path).unwrap());
        let identity = Arc::new(IdentityService::new(db.clone()));
        let contacts = ContactsService::new(db.clone(), identity.clone());
        let permissions = PermissionsService::new(db, identity);
        assert!(contacts.is_blocked(remote_peer).unwrap());
        for capability in [Capability::Chat, Capability::WallRead, Capability::Call] {
            assert!(!permissions
                .peer_has_capability(remote_peer, capability)
                .unwrap());
        }
        let restarted = contacts.claim_due_contact_revocations(100, 30, 8).unwrap();
        assert_eq!(restarted.len(), 1);
        assert_eq!(restarted[0].revocations.len(), 3);
        assert!(contacts
            .mark_contact_revocation_delivered(&restarted[0].request_id, 101)
            .unwrap());
        let delivered: i64 = contacts
            .db
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*) FROM contact_revocation_outbox WHERE state='delivered'",
                    [],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert_eq!(delivered, 1);
    }

    #[test]
    fn incoming_revocation_fails_closed_with_an_incomplete_grant_cache() {
        let (db, identity, contacts) = create_test_services();
        identity
            .create_identity(CreateIdentityRequest {
                display_name: "Local".into(),
                passphrase: "password123".into(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();
        for peer in ["12D3KooWMissingGrant", "12D3KooWEmptyCache"] {
            contacts
                .add_contact(peer, &[7; 32], &[8; 32], "Remote", None, None)
                .unwrap();
        }
        let unknown = PermissionRevokeMessage {
            grant_id: "not-in-this-cache".into(),
            issuer_peer_id: "12D3KooWMissingGrant".into(),
            lamport_clock: 99,
            revoked_at: 20,
            signature: vec![9; 64],
        };
        contacts
            .apply_incoming_contact_revocation_atomically("12D3KooWMissingGrant", &[unknown], 20)
            .unwrap();
        contacts
            .apply_incoming_contact_revocation_atomically("12D3KooWEmptyCache", &[], 21)
            .unwrap();
        assert!(!contacts.is_contact("12D3KooWMissingGrant").unwrap());
        assert!(!contacts.is_contact("12D3KooWEmptyCache").unwrap());
        let tombstones: i64 = db
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*) FROM contact_revocation_tombstones",
                    [],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert_eq!(tombstones, 2);
    }
}
