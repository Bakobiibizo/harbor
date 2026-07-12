use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::relay_identity::{MAX_LOCAL_NAME_BYTES, MAX_RELAY_HOST_BYTES};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RelayNameError {
    #[error("relay name must use canonical @name@relay form")]
    InvalidForm,
    #[error("local name is invalid")]
    InvalidLocalName,
    #[error("relay hostname is invalid")]
    InvalidRelay,
    #[error("relay name is not canonical")]
    NonCanonical,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LocalName(String);

impl LocalName {
    pub fn parse(value: &str) -> Result<Self, RelayNameError> {
        if value.len() < 3
            || value.len() > MAX_LOCAL_NAME_BYTES
            || value.starts_with('-')
            || value.ends_with('-')
            || value.contains("--")
            || !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(RelayNameError::InvalidLocalName);
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for LocalName {
    type Error = RelayNameError;
    fn try_from(v: String) -> Result<Self, Self::Error> {
        Self::parse(&v)
    }
}
impl From<LocalName> for String {
    fn from(v: LocalName) -> Self {
        v.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelayHostname(String);

impl RelayHostname {
    pub fn parse(value: &str) -> Result<Self, RelayNameError> {
        if value.is_empty()
            || value.len() > MAX_RELAY_HOST_BYTES
            || !value.is_ascii()
            || value.contains([':', '/', '@'])
            || value.ends_with('.')
            || value != value.to_ascii_lowercase()
        {
            return Err(RelayNameError::InvalidRelay);
        }
        let labels: Vec<_> = value.split('.').collect();
        if labels.len() < 2
            || labels.iter().any(|l| {
                l.is_empty()
                    || l.len() > 63
                    || l.starts_with('-')
                    || l.ends_with('-')
                    || !l
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            })
        {
            return Err(RelayNameError::InvalidRelay);
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for RelayHostname {
    type Error = RelayNameError;
    fn try_from(v: String) -> Result<Self, Self::Error> {
        Self::parse(&v)
    }
}
impl From<RelayHostname> for String {
    fn from(v: RelayHostname) -> Self {
        v.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct QualifiedRelayName {
    local: LocalName,
    relay: RelayHostname,
}

impl QualifiedRelayName {
    pub fn new(local: LocalName, relay: RelayHostname) -> Self {
        Self { local, relay }
    }
    pub fn local(&self) -> &LocalName {
        &self.local
    }
    pub fn relay(&self) -> &RelayHostname {
        &self.relay
    }
}
impl FromStr for QualifiedRelayName {
    type Err = RelayNameError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let rest = value.strip_prefix('@').ok_or(RelayNameError::InvalidForm)?;
        let (local, relay) = rest.split_once('@').ok_or(RelayNameError::InvalidForm)?;
        if relay.contains('@') {
            return Err(RelayNameError::InvalidForm);
        }
        let parsed = Self::new(LocalName::parse(local)?, RelayHostname::parse(relay)?);
        if parsed.to_string() != value {
            return Err(RelayNameError::NonCanonical);
        }
        Ok(parsed)
    }
}
impl fmt::Display for QualifiedRelayName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}@{}", self.local.as_str(), self.relay.as_str())
    }
}
impl TryFrom<String> for QualifiedRelayName {
    type Error = RelayNameError;
    fn try_from(v: String) -> Result<Self, Self::Error> {
        v.parse()
    }
}
impl From<QualifiedRelayName> for String {
    fn from(v: QualifiedRelayName) -> Self {
        v.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_canonical_names() {
        for n in ["@alice@relay.example", "@a-1@xn--bcher-kva.example"] {
            assert_eq!(n.parse::<QualifiedRelayName>().unwrap().to_string(), n);
        }
    }
    #[test]
    fn rejects_ambiguous_or_noncanonical_names() {
        for n in [
            "alice@relay.example",
            "@@relay.example",
            "@Alice@relay.example",
            "@alice@Relay.example",
            "@alice@relay.example:443",
            "@alice@https://relay.example",
            "@аlice@relay.example",
            "@-alice@relay.example",
            "@ab@relay.example",
            "@ali--ce@relay.example",
            "@alice@localhost",
            "@alice@relay.example/",
        ] {
            assert!(n.parse::<QualifiedRelayName>().is_err(), "{n}");
        }
    }
}
