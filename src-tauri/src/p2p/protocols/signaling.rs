//! Signed voice-call signaling wire protocol.
//!
//! The actual cryptographic signatures are produced and verified by
//! `CallingService` using the `SignableSignaling*` helpers.  This module only
//! defines the libp2p request/response envelope and CBOR encoding boundary for
//! `/harbor/signaling/1.0.0`.

use serde::{Deserialize, Serialize};

/// A signaling request sent over the `/harbor/signaling/1.0.0` protocol.
///
/// `sender_peer_id` and `recipient_peer_id` are transport routing metadata used
/// to reject wrong-peer and retargeted envelopes before the frontend sees them.
/// The nested payload contains the signed call data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalingEnvelope {
    pub sender_peer_id: String,
    pub recipient_peer_id: String,
    pub payload: SignalingPayload,
}

impl SignalingEnvelope {
    /// Return the call ID for responses, logging, and duplicate diagnostics.
    pub fn call_id(&self) -> &str {
        self.payload.call_id()
    }

    /// Return the Unix timestamp embedded in the signed payload.
    pub fn timestamp(&self) -> i64 {
        self.payload.timestamp()
    }
}

/// Signed call signaling payload variants.
///
/// Declines and busy responses intentionally reuse the signed hangup payload
/// because the existing canonical helper signs `{ call_id, sender_peer_id,
/// reason, timestamp }` and the reason is part of the signature.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum SignalingPayload {
    Offer(SignalingOffer),
    Answer(SignalingAnswer),
    Ice(SignalingIce),
    Hangup(SignalingHangup),
    Decline(SignalingHangup),
    Busy(SignalingHangup),
    GroupMembership(GroupMembershipSignal),
}

impl SignalingPayload {
    pub fn call_id(&self) -> &str {
        match self {
            SignalingPayload::Offer(payload) => &payload.call_id,
            SignalingPayload::Answer(payload) => &payload.call_id,
            SignalingPayload::Ice(payload) => &payload.call_id,
            SignalingPayload::Hangup(payload)
            | SignalingPayload::Decline(payload)
            | SignalingPayload::Busy(payload) => &payload.call_id,
            SignalingPayload::GroupMembership(payload) => &payload.room_id,
        }
    }

    pub fn timestamp(&self) -> i64 {
        match self {
            SignalingPayload::Offer(payload) => payload.timestamp,
            SignalingPayload::Answer(payload) => payload.timestamp,
            SignalingPayload::Ice(payload) => payload.timestamp,
            SignalingPayload::Hangup(payload)
            | SignalingPayload::Decline(payload)
            | SignalingPayload::Busy(payload) => payload.timestamp,
            SignalingPayload::GroupMembership(payload) => payload.timestamp,
        }
    }
}

/// Membership action carried by a signed group-room update.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GroupMembershipAction {
    Invite,
    Join,
    Leave,
    Roster,
    Terminate,
}

/// Signed group-room membership update. SDP and ICE continue to use the
/// existing pairwise payloads, bound to this room through deterministic legs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GroupMembershipSignal {
    pub room_id: String,
    pub creator_peer_id: String,
    pub sender_peer_id: String,
    pub action: GroupMembershipAction,
    pub topology: String,
    pub roster_version: u64,
    pub participants: Vec<String>,
    pub media_mode: String,
    pub nonce: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Signed call offer payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalingOffer {
    pub call_id: String,
    pub caller_peer_id: String,
    pub callee_peer_id: String,
    pub sdp: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Signed call answer payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalingAnswer {
    pub call_id: String,
    pub caller_peer_id: String,
    pub callee_peer_id: String,
    pub sdp: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Signed ICE candidate payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalingIce {
    pub call_id: String,
    pub sender_peer_id: String,
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<u32>,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Signed hangup/decline/busy payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalingHangup {
    pub call_id: String,
    pub sender_peer_id: String,
    pub reason: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Response to a signaling request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalingResponse {
    pub accepted: bool,
    pub call_id: Option<String>,
    pub error: Option<String>,
}

impl SignalingResponse {
    pub fn accepted(call_id: impl Into<String>) -> Self {
        Self {
            accepted: true,
            call_id: Some(call_id.into()),
            error: None,
        }
    }

    pub fn rejected(call_id: Option<String>, error: impl Into<String>) -> Self {
        Self {
            accepted: false,
            call_id,
            error: Some(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_offer_envelope() -> SignalingEnvelope {
        SignalingEnvelope {
            sender_peer_id: "12D3KooWCaller".to_string(),
            recipient_peer_id: "12D3KooWCallee".to_string(),
            payload: SignalingPayload::Offer(SignalingOffer {
                call_id: "call-1".to_string(),
                caller_peer_id: "12D3KooWCaller".to_string(),
                callee_peer_id: "12D3KooWCallee".to_string(),
                sdp: "v=0\r\ns=Harbor\r\n".to_string(),
                timestamp: 1_700_000_000,
                signature: vec![7; 64],
            }),
        }
    }

    #[test]
    fn signaling_envelope_cbor_roundtrip_preserves_signed_payload() {
        let envelope = sample_offer_envelope();
        let mut bytes = Vec::new();
        ciborium::into_writer(&envelope, &mut bytes).unwrap();
        let decoded: SignalingEnvelope = ciborium::from_reader(bytes.as_slice()).unwrap();

        assert_eq!(decoded, envelope);
        assert_eq!(decoded.call_id(), "call-1");
        assert_eq!(decoded.timestamp(), 1_700_000_000);
    }

    #[test]
    fn signaling_response_cbor_roundtrip_carries_rejections() {
        let response = SignalingResponse::rejected(
            Some("call-1".to_string()),
            "Permission denied: no call grant",
        );
        let mut bytes = Vec::new();
        ciborium::into_writer(&response, &mut bytes).unwrap();
        let decoded: SignalingResponse = ciborium::from_reader(bytes.as_slice()).unwrap();

        assert_eq!(decoded, response);
        assert!(!decoded.accepted);
        assert_eq!(decoded.call_id.as_deref(), Some("call-1"));
        assert!(decoded.error.unwrap().contains("Permission denied"));
    }

    #[test]
    fn group_membership_cbor_roundtrip_preserves_roster_contract() {
        let signal = GroupMembershipSignal {
            room_id: "room-1".into(),
            creator_peer_id: "peer-a".into(),
            sender_peer_id: "peer-a".into(),
            action: GroupMembershipAction::Invite,
            topology: "relay_assisted_mesh_v1".into(),
            roster_version: 1,
            participants: vec!["peer-a".into(), "peer-b".into()],
            media_mode: "video".into(),
            nonce: "nonce-1".into(),
            timestamp: 1_700_000_000,
            signature: vec![3; 64],
        };
        let envelope = SignalingEnvelope {
            sender_peer_id: "peer-a".into(),
            recipient_peer_id: "peer-b".into(),
            payload: SignalingPayload::GroupMembership(signal.clone()),
        };
        let mut bytes = Vec::new();
        ciborium::into_writer(&envelope, &mut bytes).unwrap();
        let decoded: SignalingEnvelope = ciborium::from_reader(bytes.as_slice()).unwrap();
        assert_eq!(decoded, envelope);
        assert_eq!(decoded.call_id(), "room-1");
        assert_eq!(decoded.timestamp(), 1_700_000_000);
    }

    #[cfg(test)]
    mod libp2p_roundtrip {
        use super::*;
        use crate::p2p::behaviour::{ChatBehaviour, ChatBehaviourEvent};
        use crate::p2p::config::NetworkConfig;
        use crate::p2p::swarm::build_swarm;
        use futures::StreamExt;
        use libp2p::request_response;
        use libp2p::swarm::SwarmEvent;
        use libp2p::{Multiaddr, PeerId, Swarm};
        use std::time::Duration;

        fn payload_kind(payload: &SignalingPayload) -> &'static str {
            match payload {
                SignalingPayload::Offer(_) => "offer",
                SignalingPayload::Answer(_) => "answer",
                SignalingPayload::Ice(_) => "ice",
                SignalingPayload::Hangup(_) => "hangup",
                SignalingPayload::Decline(_) => "decline",
                SignalingPayload::Busy(_) => "busy",
                SignalingPayload::GroupMembership(_) => "group_membership",
            }
        }

        fn envelope(kind: &str, sender: PeerId, recipient: PeerId) -> SignalingEnvelope {
            let sender_peer_id = sender.to_string();
            let recipient_peer_id = recipient.to_string();
            let call_id = "call-libp2p-1".to_string();
            let timestamp = 1_700_000_000;
            let signature = vec![1; 64];
            let payload = match kind {
                "offer" => SignalingPayload::Offer(SignalingOffer {
                    call_id,
                    caller_peer_id: sender_peer_id.clone(),
                    callee_peer_id: recipient_peer_id.clone(),
                    sdp: "v=0\r\ns=Harbor\r\n".to_string(),
                    timestamp,
                    signature,
                }),
                "answer" => SignalingPayload::Answer(SignalingAnswer {
                    call_id,
                    caller_peer_id: recipient_peer_id.clone(),
                    callee_peer_id: sender_peer_id.clone(),
                    sdp: "v=0\r\ns=Harbor\r\n".to_string(),
                    timestamp,
                    signature,
                }),
                "ice" => SignalingPayload::Ice(SignalingIce {
                    call_id,
                    sender_peer_id: sender_peer_id.clone(),
                    candidate: "candidate:0 1 UDP".to_string(),
                    sdp_mid: Some("audio".to_string()),
                    sdp_mline_index: Some(0),
                    timestamp,
                    signature,
                }),
                "hangup" => SignalingPayload::Hangup(SignalingHangup {
                    call_id,
                    sender_peer_id: sender_peer_id.clone(),
                    reason: "normal".to_string(),
                    timestamp,
                    signature,
                }),
                _ => panic!("unsupported test payload kind"),
            };

            SignalingEnvelope {
                sender_peer_id,
                recipient_peer_id,
                payload,
            }
        }

        async fn wait_listen_addr(swarm: &mut Swarm<ChatBehaviour>, peer: PeerId) -> Multiaddr {
            let deadline = tokio::time::sleep(Duration::from_secs(5));
            tokio::pin!(deadline);

            loop {
                tokio::select! {
                    event = swarm.select_next_some() => {
                        if let SwarmEvent::NewListenAddr { address, .. } = event {
                            return address.with(libp2p::multiaddr::Protocol::P2p(peer));
                        }
                    }
                    _ = &mut deadline => panic!("timed out waiting for listener address"),
                }
            }
        }

        async fn wait_connected(
            swarm_a: &mut Swarm<ChatBehaviour>,
            peer_a: PeerId,
            swarm_b: &mut Swarm<ChatBehaviour>,
            peer_b: PeerId,
        ) {
            let mut a_connected = false;
            let mut b_connected = false;
            let deadline = tokio::time::sleep(Duration::from_secs(5));
            tokio::pin!(deadline);

            while !a_connected || !b_connected {
                tokio::select! {
                    event = swarm_a.select_next_some() => {
                        if let SwarmEvent::ConnectionEstablished { peer_id, .. } = event {
                            if peer_id == peer_b {
                                a_connected = true;
                            }
                        }
                    }
                    event = swarm_b.select_next_some() => {
                        if let SwarmEvent::ConnectionEstablished { peer_id, .. } = event {
                            if peer_id == peer_a {
                                b_connected = true;
                            }
                        }
                    }
                    _ = &mut deadline => panic!("timed out waiting for signaling peers to connect"),
                }
            }
        }

        async fn exchange(
            sender: &mut Swarm<ChatBehaviour>,
            receiver: &mut Swarm<ChatBehaviour>,
            target_peer: PeerId,
            request: SignalingEnvelope,
            expected_kind: &'static str,
        ) {
            let request_id = sender
                .behaviour_mut()
                .signaling
                .send_request(&target_peer, request);
            let deadline = tokio::time::sleep(Duration::from_secs(5));
            tokio::pin!(deadline);

            loop {
                tokio::select! {
                    event = sender.select_next_some() => {
                        if let SwarmEvent::Behaviour(ChatBehaviourEvent::Signaling(
                            request_response::Event::Message {
                                message: request_response::Message::Response { request_id: id, response },
                                ..
                            },
                        )) = event
                        {
                            if id == request_id {
                                assert!(response.accepted, "signaling response was rejected: {:?}", response.error);
                                assert_eq!(response.call_id.as_deref(), Some("call-libp2p-1"));
                                return;
                            }
                        }
                    }
                    event = receiver.select_next_some() => {
                        if let SwarmEvent::Behaviour(ChatBehaviourEvent::Signaling(
                            request_response::Event::Message {
                                message: request_response::Message::Request { request, channel, .. },
                                ..
                            },
                        )) = event
                        {
                            assert_eq!(payload_kind(&request.payload), expected_kind);
                            receiver
                                .behaviour_mut()
                                .signaling
                                .send_response(channel, SignalingResponse::accepted(request.call_id().to_string()))
                                .expect("signaling response should send");
                        }
                    }
                    _ = &mut deadline => panic!("timed out waiting for signaling {} exchange", expected_kind),
                }
            }
        }

        #[tokio::test]
        async fn offer_answer_ice_hangup_cross_libp2p_signaling_protocol() {
            let mut swarm_a = build_swarm(
                libp2p::identity::Keypair::generate_ed25519(),
                &NetworkConfig::default(),
            )
            .unwrap();
            let mut swarm_b = build_swarm(
                libp2p::identity::Keypair::generate_ed25519(),
                &NetworkConfig::default(),
            )
            .unwrap();
            let peer_a = *swarm_a.local_peer_id();
            let peer_b = *swarm_b.local_peer_id();

            swarm_b
                .listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap())
                .unwrap();
            let address_b = wait_listen_addr(&mut swarm_b, peer_b).await;
            swarm_a.dial(address_b).unwrap();
            wait_connected(&mut swarm_a, peer_a, &mut swarm_b, peer_b).await;

            exchange(
                &mut swarm_a,
                &mut swarm_b,
                peer_b,
                envelope("offer", peer_a, peer_b),
                "offer",
            )
            .await;
            exchange(
                &mut swarm_b,
                &mut swarm_a,
                peer_a,
                envelope("answer", peer_b, peer_a),
                "answer",
            )
            .await;
            exchange(
                &mut swarm_a,
                &mut swarm_b,
                peer_b,
                envelope("ice", peer_a, peer_b),
                "ice",
            )
            .await;
            exchange(
                &mut swarm_b,
                &mut swarm_a,
                peer_a,
                envelope("hangup", peer_b, peer_a),
                "hangup",
            )
            .await;
        }
    }
}
