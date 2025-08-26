//! Integration module for connecting the new P2P system to the main Neptune Core application
//!
//! This module provides a unified interface that bridges the existing legacy networking
//! with our new libp2p-based P2P system, enabling seamless migration and operation.

use crate::p2p::bridge::{BridgeStatus, LegacyBridge, MigrationPhase};
use crate::p2p::config::NetworkConfig;
use crate::p2p::network::NetworkService;
use crate::p2p::protocol::codec::MessageCodec;
use crate::p2p::protocol::handler::ProtocolHandler;
use crate::p2p::{NetworkMetrics, NetworkStatus, P2pResult};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Integration service that manages both legacy and new P2P networking
pub struct P2pIntegrationService {
    /// Network configuration
    config: NetworkConfig,
    /// Legacy compatibility bridge
    legacy_bridge: Option<LegacyBridge>,
    /// New libp2p network service
    libp2p_service: Option<NetworkService>,
    /// Protocol handler for application layer integration
    protocol_handler: ProtocolHandler,
    /// Message codec for protocol translation
    message_codec: MessageCodec,
    /// Integration status
    status: Arc<RwLock<IntegrationStatus>>,
    /// Channel for receiving application messages
    app_rx: mpsc::Receiver<ApplicationMessage>,
    /// Channel for sending network events to application
    network_tx: mpsc::Sender<NetworkEvent>,
    /// Service running flag
    running: bool,
}

/// Integration status information
#[derive(Debug, Clone)]
pub struct IntegrationStatus {
    /// Current migration phase
    pub migration_phase: MigrationPhase,
    /// Legacy network status
    pub legacy_status: String,
    /// libp2p network status
    pub libp2p_status: String,
    /// Protocol handler status
    pub protocol_status: String,
    /// Overall integration health
    pub health_score: u8,
    /// Active connections count
    pub total_connections: usize,
    /// Legacy connections count
    pub legacy_connections: usize,
    /// libp2p connections count
    pub libp2p_connections: usize,
}

/// Application layer messages that need network routing
#[derive(Debug, Clone)]
pub enum ApplicationMessage {
    /// Block broadcast request
    BroadcastBlock {
        block_data: Vec<u8>,
        block_hash: String,
    },
    /// Transaction broadcast request
    BroadcastTransaction { tx_data: Vec<u8>, tx_hash: String },
    /// Peer list request
    GetPeerList { max_peers: u32 },
    /// Sync request
    SyncRequest { from_height: u64, to_height: u64 },
    /// Network status request
    GetNetworkStatus,
    /// Force peer discovery
    ForcePeerDiscovery,
    /// Add manual peer
    AddManualPeer { address: String },
}

/// Network events sent to the application layer
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// New block received from network
    BlockReceived {
        block_data: Vec<u8>,
        block_hash: String,
        source: String,
    },
    /// New transaction received from network
    TransactionReceived {
        tx_data: Vec<u8>,
        tx_hash: String,
        source: String,
    },
    /// Peer list update
    PeerListUpdate { peers: Vec<String>, source: String },
    /// Sync response received
    SyncResponse {
        blocks: Vec<Vec<u8>>,
        source: String,
    },
    /// Network status update
    NetworkStatusUpdate { status: IntegrationStatus },
    /// Peer connection event
    PeerConnected {
        peer_id: String,
        address: String,
        protocol: String,
    },
    /// Peer disconnection event
    PeerDisconnected { peer_id: String, reason: String },
    /// Network error event
    NetworkError {
        error: String,
        severity: ErrorSeverity,
    },
}

/// Error severity levels
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Low severity - informational
    Low,
    /// Medium severity - warning
    Medium,
    /// High severity - error
    High,
    /// Critical severity - fatal
    Critical,
}

impl P2pIntegrationService {
    /// Create a new P2P integration service
    pub async fn new(
        config: NetworkConfig,
        app_rx: mpsc::Receiver<ApplicationMessage>,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> P2pResult<Self> {
        // Validate configuration
        config.validate()?;

        let status = Arc::new(RwLock::new(IntegrationStatus {
            migration_phase: MigrationPhase::DualProtocol,
            legacy_status: "Initializing".to_string(),
            libp2p_status: "Initializing".to_string(),
            protocol_status: "Initializing".to_string(),
            health_score: 0,
            total_connections: 0,
            legacy_connections: 0,
            libp2p_connections: 0,
        }));

        let protocol_handler = ProtocolHandler::new(config.protocol.clone())?;
        let message_codec = MessageCodec::new(config.protocol.clone())?;

        Ok(Self {
            config,
            legacy_bridge: None,
            libp2p_service: None,
            protocol_handler,
            message_codec,
            status,
            app_rx,
            network_tx,
            running: false,
        })
    }

    /// Start the P2P integration service
    pub async fn start(&mut self) -> P2pResult<()> {
        if self.running {
            return Err(crate::p2p::P2pError::Bridge(
                crate::p2p::bridge::BridgeError::ConfigurationError(
                    "Integration service already running".to_string(),
                ),
            ));
        }

        info!("Starting P2P integration service...");

        // Initialize legacy bridge if enabled
        if self.config.legacy_enabled() {
            info!("Initializing legacy compatibility bridge...");
            self.initialize_legacy_bridge().await?;
        }

        // Initialize libp2p service if enabled
        if self.config.libp2p_enabled() {
            info!("Initializing libp2p network service...");
            self.initialize_libp2p_service().await?;
        }

        // Start protocol handler
        self.protocol_handler.start().await?;

        // Start main integration loop
        self.start_integration_loop().await?;

        self.running = true;
        info!("P2P integration service started successfully");
        Ok(())
    }

    /// Stop the P2P integration service
    pub async fn stop(&mut self) -> P2pResult<()> {
        if !self.running {
            return Ok(());
        }

        info!("Stopping P2P integration service...");
        self.running = false;

        // Stop protocol handler
        self.protocol_handler.stop().await?;

        // Stop libp2p service
        if let Some(mut service) = self.libp2p_service.take() {
            service.stop().await?;
        }

        // Stop legacy bridge
        if let Some(mut bridge) = self.legacy_bridge.take() {
            bridge.stop().await?;
        }

        info!("P2P integration service stopped");
        Ok(())
    }

    /// Initialize the legacy compatibility bridge
    async fn initialize_legacy_bridge(&mut self) -> P2pResult<()> {
        let (bridge_tx, bridge_rx) = mpsc::channel(100);
        let (network_tx, network_rx) = mpsc::channel(100);

        let bridge_status = Arc::new(RwLock::new(BridgeStatus {
            migration_phase: MigrationPhase::DualProtocol,
            legacy_connections: 0,
            libp2p_connections: 0,
            migration_progress: 0.0,
        }));

        let mut legacy_bridge =
            LegacyBridge::new(self.config.clone(), bridge_tx, bridge_status).await?;

        legacy_bridge.start().await?;
        self.legacy_bridge = Some(legacy_bridge);

        // Spawn legacy bridge event handler
        let protocol_handler = self.protocol_handler.clone();
        let message_codec = self.message_codec.clone();
        let network_tx = self.network_tx.clone();

        tokio::spawn(async move {
            Self::handle_legacy_bridge_events(
                bridge_rx,
                network_rx,
                protocol_handler,
                message_codec,
                network_tx,
            )
            .await;
        });

        Ok(())
    }

    /// Initialize the libp2p network service
    async fn initialize_libp2p_service(&mut self) -> P2pResult<()> {
        let mut libp2p_service = NetworkService::new(self.config.clone()).await?;
        libp2p_service.start().await?;
        self.libp2p_service = Some(libp2p_service);
        Ok(())
    }

    /// Start the main integration loop
    async fn start_integration_loop(&mut self) -> P2pResult<()> {
        let mut app_rx = self.app_rx.clone();
        let network_tx = self.network_tx.clone();
        let mut status = self.status.clone();

        tokio::spawn(async move {
            loop {
                // Handle application messages
                while let Ok(message) = app_rx.try_recv() {
                    if let Err(e) = Self::handle_application_message(message, &network_tx).await {
                        error!("Failed to handle application message: {}", e);
                    }
                }

                // Update integration status
                if let Err(e) = Self::update_integration_status(&mut status, &network_tx).await {
                    error!("Failed to update integration status: {}", e);
                }

                // Sleep to prevent busy waiting
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });

        Ok(())
    }

    /// Handle application layer messages
    async fn handle_application_message(
        message: ApplicationMessage,
        network_tx: &mpsc::Sender<NetworkEvent>,
    ) -> P2pResult<()> {
        match message {
            ApplicationMessage::BroadcastBlock {
                block_data,
                block_hash,
            } => {
                debug!("Broadcasting block: {}", block_hash);
                // TODO: Route to appropriate network (legacy or libp2p)
                let _ = network_tx
                    .send(NetworkEvent::BlockReceived {
                        block_data: block_data.clone(),
                        block_hash: block_hash.clone(),
                        source: "application".to_string(),
                    })
                    .await;
            }
            ApplicationMessage::BroadcastTransaction { tx_data, tx_hash } => {
                debug!("Broadcasting transaction: {}", tx_hash);
                // TODO: Route to appropriate network
                let _ = network_tx
                    .send(NetworkEvent::TransactionReceived {
                        tx_data: tx_data.clone(),
                        tx_hash: tx_hash.clone(),
                        source: "application".to_string(),
                    })
                    .await;
            }
            ApplicationMessage::GetPeerList { max_peers } => {
                debug!("Getting peer list (max: {})", max_peers);
                // TODO: Get from both legacy and libp2p networks
            }
            ApplicationMessage::SyncRequest {
                from_height,
                to_height,
            } => {
                debug!("Sync request: {} to {}", from_height, to_height);
                // TODO: Route to appropriate network
            }
            ApplicationMessage::GetNetworkStatus => {
                debug!("Getting network status");
                // TODO: Return current integration status
            }
            ApplicationMessage::ForcePeerDiscovery => {
                debug!("Forcing peer discovery");
                // TODO: Trigger discovery in both networks
            }
            ApplicationMessage::AddManualPeer { address } => {
                debug!("Adding manual peer: {}", address);
                // TODO: Add to both networks if possible
            }
        }

        Ok(())
    }

    /// Update integration status
    async fn update_integration_status(
        status: &mut Arc<RwLock<IntegrationStatus>>,
        network_tx: &mpsc::Sender<NetworkEvent>,
    ) -> P2pResult<()> {
        let mut status_guard = status.write().await;

        // Update connection counts (placeholder for now)
        status_guard.total_connections =
            status_guard.legacy_connections + status_guard.libp2p_connections;

        // Calculate health score
        let mut health_score = 100u8;
        if status_guard.total_connections == 0 {
            health_score = health_score.saturating_sub(50);
        } else if status_guard.total_connections < 3 {
            health_score = health_score.saturating_sub(20);
        }

        status_guard.health_score = health_score;

        // Send status update to application
        let _ = network_tx
            .send(NetworkEvent::NetworkStatusUpdate {
                status: status_guard.clone(),
            })
            .await;

        Ok(())
    }

    /// Handle legacy bridge events
    async fn handle_legacy_bridge_events(
        mut bridge_rx: mpsc::Receiver<crate::p2p::bridge::InternalMessage>,
        mut network_rx: mpsc::Receiver<NetworkEvent>,
        protocol_handler: ProtocolHandler,
        message_codec: MessageCodec,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) {
        loop {
            tokio::select! {
                // Handle bridge messages
                Some(bridge_msg) = bridge_rx.recv() => {
                    debug!("Received bridge message: {:?}", bridge_msg);
                    // TODO: Process bridge messages and route appropriately
                }

                // Handle network events
                Some(network_event) = network_rx.recv() => {
                    debug!("Received network event: {:?}", network_event);
                    // TODO: Process network events and route to application
                }

                // Handle timeout
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // Continue loop
                }
            }
        }
    }

    /// Get current integration status
    pub async fn get_status(&self) -> IntegrationStatus {
        self.status.read().await.clone()
    }

    /// Check if service is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get configuration
    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }
}

impl Drop for P2pIntegrationService {
    fn drop(&mut self) {
        if self.running {
            // Try to stop gracefully
            let _ = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(self.stop())
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::config::NetworkConfig;

    #[tokio::test]
    async fn test_integration_service_creation() {
        let config = NetworkConfig::test_config();
        let (app_tx, app_rx) = mpsc::channel(100);
        let (network_tx, _) = mpsc::channel(100);

        let service = P2pIntegrationService::new(config, app_rx, network_tx).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_integration_service_lifecycle() {
        let config = NetworkConfig::test_config();
        let (app_tx, app_rx) = mpsc::channel(100);
        let (network_tx, _) = mpsc::channel(100);

        let mut service = P2pIntegrationService::new(config, app_rx, network_tx)
            .await
            .unwrap();

        // Service should not be running initially
        assert!(!service.is_running());

        // Start service
        let result = service.start().await;
        assert!(result.is_ok());
        assert!(service.is_running());

        // Stop service
        let result = service.stop().await;
        assert!(result.is_ok());
        assert!(!service.is_running());
    }
}
