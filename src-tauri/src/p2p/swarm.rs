use libp2p::{
    identity::Keypair,
    noise, tcp, yamux, PeerId, Swarm, SwarmBuilder,
};
use tracing::info;

use super::behaviour::ChatBehaviour;
use super::config::NetworkConfig;
use crate::error::{AppError, Result};

/// Build a libp2p swarm with all configured protocols
pub fn build_swarm(
    keypair: Keypair,
    config: &NetworkConfig,
) -> Result<Swarm<ChatBehaviour>> {
    let local_peer_id = PeerId::from(keypair.public());

    info!("Building swarm with peer ID: {}", local_peer_id);

    let swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true),
            noise::Config::new,
            yamux::Config::default,
        )
        .map_err(|e| AppError::Network(format!("TCP transport error: {}", e)))?
        .with_quic()
        .with_relay_client(noise::Config::new, yamux::Config::default)
        .map_err(|e| AppError::Network(format!("Relay client error: {}", e)))?
        .with_behaviour(|keypair, relay_behaviour| {
            Ok(ChatBehaviour::new(
                PeerId::from(keypair.public()),
                keypair.public(),
                relay_behaviour,
            ))
        })
        .map_err(|e| AppError::Network(format!("Behaviour error: {}", e)))?
        .with_swarm_config(|cfg| {
            cfg.with_idle_connection_timeout(config.idle_connection_timeout)
        })
        .build();

    Ok(swarm)
}

/// Convert our application's Ed25519 keypair to a libp2p Keypair
pub fn ed25519_to_libp2p_keypair(ed25519_bytes: &[u8; 32]) -> Result<Keypair> {
    let secret = libp2p::identity::ed25519::SecretKey::try_from_bytes(ed25519_bytes.to_vec())
        .map_err(|e| crate::error::AppError::Crypto(format!("Invalid Ed25519 key: {}", e)))?;
    let keypair = libp2p::identity::ed25519::Keypair::from(secret);
    Ok(Keypair::from(keypair))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_conversion() {
        // Generate a random Ed25519 keypair
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        let signing_key = SigningKey::generate(&mut OsRng);
        let bytes = signing_key.to_bytes();

        let libp2p_keypair = ed25519_to_libp2p_keypair(&bytes).unwrap();
        let peer_id = PeerId::from(libp2p_keypair.public());

        // Peer ID should start with "12D3KooW"
        assert!(peer_id.to_string().starts_with("12D3KooW"));
    }
}
