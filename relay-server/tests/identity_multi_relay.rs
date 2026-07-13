use ed25519_dalek::{Signer, SigningKey};
use harbor_relay_server::{
    abuse::{AbuseGuard, Limits},
    auth::{AuthChallenge, AuthService},
    db::RelayDatabase,
    introduction::{IntroductionEnvelope, IntroductionService},
    name_registration::{register, NameClaimRequest, SignedNameClaimRequest},
};
use libp2p::{identity, PeerId};
use std::sync::{Arc, Barrier};

fn identity(seed: u8) -> (SigningKey, String, Vec<u8>) {
    let key = SigningKey::from_bytes(&[seed; 32]);
    let raw = key.verifying_key().to_bytes();
    let public =
        identity::PublicKey::from(identity::ed25519::PublicKey::try_from_bytes(&raw).unwrap());
    (
        key,
        PeerId::from_public_key(&public).to_string(),
        public.encode_protobuf(),
    )
}
fn signed_name(
    key: &SigningKey,
    peer: &str,
    name: &str,
    relay: &str,
    nonce: u8,
) -> SignedNameClaimRequest {
    let request = NameClaimRequest {
        domain: "harbor/name-claim-request/1".into(),
        version: 1,
        local_name: name.into(),
        relay: relay.into(),
        peer_id: peer.into(),
        ed25519_public_key: key.verifying_key().to_bytes().to_vec(),
        x25519_public_key: x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from(
            [nonce.max(1); 32],
        ))
        .to_bytes()
        .to_vec(),
        sequence: 1,
        issued_at: 100,
        nonce: vec![nonce; 16],
    };
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(&request, &mut bytes).unwrap();
    SignedNameClaimRequest {
        user_signature: key.sign(&bytes).to_bytes().to_vec(),
        request,
    }
}
fn challenge_bytes(c: &AuthChallenge) -> Vec<u8> {
    let mut value = c.clone();
    value.relay_signature.clear();
    let mut out = Vec::new();
    ciborium::ser::into_writer(&value, &mut out).unwrap();
    out
}
fn token(
    auth: &mut AuthService,
    key: &SigningKey,
    peer: &str,
    protobuf: &[u8],
    audience: &str,
) -> String {
    let id: PeerId = peer.parse().unwrap();
    let c = auth.issue_challenge(&id, audience, 100).unwrap();
    auth.complete(
        &c,
        protobuf,
        &key.sign(&challenge_bytes(&c)).to_bytes(),
        100,
    )
    .unwrap()
}
fn limits() -> Limits {
    Limits {
        peer: 20,
        network: 20,
        target: 20,
        action: 20,
        global: 40,
        window_secs: 60,
    }
}

#[test]
fn two_relays_three_identities_collision_restart_and_cross_namespace_intro() {
    let dir = tempfile::tempdir().unwrap();
    let alpha_path = dir.path().join("alpha.db");
    let beta_path = dir.path().join("beta.db");
    let alpha = RelayDatabase::open(alpha_path.to_str().unwrap()).unwrap();
    let beta = RelayDatabase::open(beta_path.to_str().unwrap()).unwrap();
    let relay_a = SigningKey::from_bytes(&[90; 32]);
    let relay_b = SigningKey::from_bytes(&[91; 32]);
    let (alice, alice_peer, alice_pb) = identity(1);
    let (bob, bob_peer, bob_pb) = identity(2);
    let (carol, carol_peer, _) = identity(3);
    let barrier = Arc::new(Barrier::new(3));
    let a1 = alpha.clone();
    let a2 = alpha.clone();
    let b1 = barrier.clone();
    let b2 = barrier.clone();
    let alice_req = signed_name(&alice, &alice_peer, "alice", "alpha.test", 11);
    let carol_req = signed_name(&carol, &carol_peer, "alice", "alpha.test", 12);
    let relay_a1 = relay_a.clone();
    let relay_a2 = relay_a.clone();
    let t1 = std::thread::spawn(move || {
        b1.wait();
        a1.with_connection(|c| register(c, "alpha.test", "k1", &relay_a1, alice_req, 100).is_ok())
    });
    let t2 = std::thread::spawn(move || {
        b2.wait();
        a2.with_connection(|c| register(c, "alpha.test", "k1", &relay_a2, carol_req, 100).is_ok())
    });
    barrier.wait();
    let winners = [t1.join().unwrap(), t2.join().unwrap()];
    assert_eq!(winners.iter().filter(|v| **v).count(), 1);
    // Bob owns an independent namespace.
    assert!(beta
        .with_connection(|c| register(
            c,
            "beta.test",
            "k1",
            &relay_b,
            signed_name(&bob, &bob_peer, "bob", "beta.test", 13),
            100
        ))
        .is_ok());
    drop(alpha);
    let alpha = RelayDatabase::open(alpha_path.to_str().unwrap()).unwrap();
    let winner:String=alpha.with_connection(|c|c.query_row("SELECT peer_id FROM relay_name_claims WHERE relay='alpha.test' AND local_name='alice' AND status='active'",[],|r|r.get(0)).unwrap());
    assert!(winner == alice_peer || winner == carol_peer);
    // Route Bob's opaque cross-namespace introduction to the collision winner.
    let relay_auth = identity::Keypair::generate_ed25519();
    let mut auth = AuthService::new("alpha.test", "k1", relay_auth.clone());
    let submit = token(&mut auth, &bob, &bob_peer, &bob_pb, "introduce");
    let target_key = if winner == alice_peer { &alice } else { &carol };
    let target_pb = identity::PublicKey::from(
        identity::ed25519::PublicKey::try_from_bytes(&target_key.verifying_key().to_bytes())
            .unwrap(),
    )
    .encode_protobuf();
    let read = token(
        &mut auth,
        target_key,
        &winner,
        &target_pb,
        "introductions:read",
    );
    let mut abuse = AbuseGuard::new(limits());
    let work = abuse
        .issue_with_delivery_key(
            "alpha.test",
            &bob_peer,
            "@alice@alpha.test",
            "introduce",
            100,
            "k1",
            &auth.signing_key(),
            vec![7; 32],
        )
        .unwrap();
    let nonce = (0..).find(|n| work.verify(*n, 100)).unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let envelope = IntroductionEnvelope {
        version: 1,
        request_id: id.clone(),
        target: "@alice@alpha.test".into(),
        requester_peer_id: bob_peer.clone(),
        requester_ephemeral_x25519_key: vec![7; 32],
        message_ciphertext: vec![8; 64],
        issued_at: 100,
        expires_at: 200,
        work_challenge: work,
        work_nonce: nonce,
    };
    alpha.with_connection(|c| {
        let mut service = IntroductionService::new(c, &auth, &mut abuse).unwrap();
        let response = service.submit(&submit, "test-net", envelope, 100, false);
        assert_eq!(response.status, "accepted-for-processing");
        let queued = service.take(&read, 101, 10).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].requester_peer_id, bob_peer);
        assert_eq!(queued[0].message_ciphertext, vec![8; 64]);
    });

    // Simulate Relay A becoming unavailable before the target acknowledges the
    // queued envelope. No in-memory service/auth state survives the outage.
    drop(auth);
    drop(abuse);
    drop(alpha);

    let alpha = RelayDatabase::open(alpha_path.to_str().unwrap()).unwrap();
    let mut restarted_auth = AuthService::new("alpha.test", "k1", relay_auth);
    assert!(restarted_auth
        .authorize(&read, "introductions:read", 102)
        .is_err());
    let fresh_read = token(
        &mut restarted_auth,
        target_key,
        &winner,
        &target_pb,
        "introductions:read",
    );
    let mut restarted_abuse = AbuseGuard::new(limits());
    alpha.with_connection(|connection| {
        let service =
            IntroductionService::new(connection, &restarted_auth, &mut restarted_abuse).unwrap();
        let recovered = service.take(&fresh_read, 102, 10).unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].request_id, id);
        assert_eq!(recovered[0].message_ciphertext, vec![8; 64]);
        assert_eq!(
            service
                .acknowledge(&fresh_read, std::slice::from_ref(&id), 102)
                .unwrap(),
            1
        );
        assert!(service.take(&fresh_read, 102, 10).unwrap().is_empty());
    });
    let recovered_winner: String = alpha.with_connection(|connection| {
        connection
            .query_row(
                "SELECT peer_id FROM relay_name_claims
                 WHERE relay='alpha.test' AND local_name='alice' AND status='active'",
                [],
                |row| row.get(0),
            )
            .unwrap()
    });
    assert_eq!(recovered_winner, winner);
    // Both independent namespace assignments survive reopening.
    drop(beta);
    let beta = RelayDatabase::open(beta_path.to_str().unwrap()).unwrap();
    let persisted:String=beta.with_connection(|c|c.query_row("SELECT peer_id FROM relay_name_claims WHERE relay='beta.test' AND local_name='bob' AND status='active'",[],|r|r.get(0)).unwrap());
    assert_eq!(persisted, bob_peer);
    let _ = alice_pb;
}
