use serde::{Deserialize, Serialize};

/// Request for identity information from a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityRequest {
    /// The peer ID of the requester
    pub requester_peer_id: String,
    /// Timestamp of the request
    pub timestamp: i64,
    /// Signature over (requester_peer_id, timestamp) using requester's Ed25519 key
    pub signature: Vec<u8>,
}

/// Response containing identity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityResponse {
    /// The peer ID (derived from public key)
    pub peer_id: String,
    /// Ed25519 public key
    pub public_key: Vec<u8>,
    /// X25519 public key for key agreement
    pub x25519_public: Vec<u8>,
    /// Display name
    pub display_name: String,
    /// Avatar hash (SHA-256 of avatar image)
    pub avatar_hash: Option<String>,
    /// Bio/description
    pub bio: Option<String>,
    /// Timestamp
    pub timestamp: i64,
    /// Signature over all fields above
    pub signature: Vec<u8>,
}

/// Codec for identity exchange protocol
#[derive(Debug, Clone, Default)]
pub struct IdentityCodec;

impl IdentityCodec {
    /// Encode an identity request to CBOR bytes
    pub fn encode_request(request: &IdentityRequest) -> Result<Vec<u8>, ciborium::ser::Error<std::io::Error>> {
        let mut bytes = Vec::new();
        ciborium::into_writer(request, &mut bytes)?;
        Ok(bytes)
    }

    /// Decode an identity request from CBOR bytes
    pub fn decode_request(bytes: &[u8]) -> Result<IdentityRequest, ciborium::de::Error<std::io::Error>> {
        ciborium::from_reader(bytes)
    }

    /// Encode an identity response to CBOR bytes
    pub fn encode_response(response: &IdentityResponse) -> Result<Vec<u8>, ciborium::ser::Error<std::io::Error>> {
        let mut bytes = Vec::new();
        ciborium::into_writer(response, &mut bytes)?;
        Ok(bytes)
    }

    /// Decode an identity response from CBOR bytes
    pub fn decode_response(bytes: &[u8]) -> Result<IdentityResponse, ciborium::de::Error<std::io::Error>> {
        ciborium::from_reader(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_request_roundtrip() {
        let request = IdentityRequest {
            requester_peer_id: "12D3KooWTestPeerId".to_string(),
            timestamp: 1234567890,
            signature: vec![1, 2, 3, 4],
        };

        let encoded = IdentityCodec::encode_request(&request).unwrap();
        let decoded = IdentityCodec::decode_request(&encoded).unwrap();

        assert_eq!(decoded.requester_peer_id, request.requester_peer_id);
        assert_eq!(decoded.timestamp, request.timestamp);
        assert_eq!(decoded.signature, request.signature);
    }

    #[test]
    fn test_identity_response_roundtrip() {
        let response = IdentityResponse {
            peer_id: "12D3KooWTestPeerId".to_string(),
            public_key: vec![1, 2, 3],
            x25519_public: vec![4, 5, 6],
            display_name: "Test User".to_string(),
            avatar_hash: Some("abc123".to_string()),
            bio: Some("A test bio".to_string()),
            timestamp: 1234567890,
            signature: vec![7, 8, 9],
        };

        let encoded = IdentityCodec::encode_response(&response).unwrap();
        let decoded = IdentityCodec::decode_response(&encoded).unwrap();

        assert_eq!(decoded.peer_id, response.peer_id);
        assert_eq!(decoded.display_name, response.display_name);
        assert_eq!(decoded.bio, response.bio);
    }
}
