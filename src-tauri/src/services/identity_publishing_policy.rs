use crate::{
    db::{repositories::RelayNamesRepository, Database},
    error::{AppError, Result},
    services::IdentityService,
};

pub struct IdentityPublishingPolicy;
impl IdentityPublishingPolicy {
    pub fn evaluate(mode: Option<&str>, has_active_claim: bool) -> Result<()> {
        match mode {
            Some("unverified") if !has_active_claim => Ok(()),
            Some("unverified") => Err(AppError::Validation(
                "A verified identity cannot publish as unverified".into(),
            )),
            Some("verified") if has_active_claim => Ok(()),
            Some("verified") => Err(AppError::Validation(
                "Verified publishing requires an active relay name claim".into(),
            )),
            _ => Err(AppError::Validation(
                "Claim a verified Harbor name or explicitly publish as an unverified identity"
                    .into(),
            )),
        }
    }
    pub fn enforce(db: &Database, identity: &IdentityService) -> Result<()> {
        let peer = identity.get_peer_id()?;
        let mode: Option<String> = db
            .with_connection(|c| {
                c.query_row(
                    "SELECT mode FROM identity_publishing_state WHERE peer_id=?",
                    [&peer],
                    |r| r.get(0),
                )
                .optional()
            })
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        let active = crate::services::name_claim_service::verified_name_claim(
            &RelayNamesRepository::new(db),
            &peer,
            chrono::Utc::now().timestamp(),
        )
        .map_err(|error| AppError::Crypto(error.to_string()))?
        .is_some();
        Self::evaluate(mode.as_deref(), active)
    }
}
use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_bypass_requires_explicit_state() {
        assert!(IdentityPublishingPolicy::evaluate(None, false).is_err());
        assert!(IdentityPublishingPolicy::evaluate(Some("required"), true).is_err());
        assert!(IdentityPublishingPolicy::evaluate(Some("verified"), false).is_err());
        assert!(IdentityPublishingPolicy::evaluate(Some("verified"), true).is_ok());
        assert!(IdentityPublishingPolicy::evaluate(Some("unverified"), false).is_ok());
        assert!(IdentityPublishingPolicy::evaluate(Some("unverified"), true).is_err());
    }
}
