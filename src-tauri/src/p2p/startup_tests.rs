use super::{NetworkConfig, NetworkService};
use crate::db::Database;
use crate::error::AppError;
use crate::services::IdentityService;
use std::net::TcpListener;
use std::sync::Arc;

fn test_service(config: NetworkConfig) -> NetworkService {
    let identity = Arc::new(IdentityService::new(Arc::new(
        Database::in_memory().unwrap(),
    )));
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    NetworkService::new(config, identity, keypair).unwrap().0
}

#[tokio::test]
async fn listener_failure_is_reported_and_the_same_port_can_start_cleanly_afterward() {
    let blocker = TcpListener::bind(("0.0.0.0", 0)).unwrap();
    let tcp_port = blocker.local_addr().unwrap().port();
    let config = NetworkConfig {
        tcp_port,
        quic_port: 0,
        enable_mdns: false,
        ..NetworkConfig::default()
    };

    let mut failed_service = test_service(config.clone());
    let error = failed_service.start_listening().await.unwrap_err();
    assert!(matches!(error, AppError::Network(_)));
    drop(failed_service);
    drop(blocker);

    let mut clean_service = test_service(config);
    clean_service.start_listening().await.unwrap();
}
