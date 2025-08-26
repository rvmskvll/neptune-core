//! Legacy TCP network service for backward compatibility
//!
//! This module provides a bridge between the existing legacy TCP-based P2P
//! implementation and the new libp2p networking layer.

use crate::p2p::bridge::adapter::MessageAdapter;
use crate::p2p::bridge::{BridgeStatus, InternalMessage, MigrationPhase};
use crate::p2p::config::NetworkConfig;
use crate::p2p::{NetworkMetrics, NetworkStatus, P2pResult};
use futures::StreamExt;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio_serde::{Framed, SymmetricalBincode};
use tokio_util::codec::LengthDelimitedCodec;
use tracing::{debug, error, info, warn};

/// Legacy Neptune Core peer message types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LegacyPeerMessage {
    /// Handshake message
    Handshake {
        version: String,
        instance_id: String,
        timestamp: u64,
    },
    /// Block message
    Block {
        block_data: Vec<u8>,
        block_hash: String,
    },
    /// Transaction message
    Transaction { tx_data: Vec<u8>, tx_hash: String },
    /// Peer list request
    PeerListRequest,
    /// Peer list response
    PeerListResponse { peers: Vec<SocketAddr> },
    /// Sync challenge
    SyncChallenge { challenge: Vec<u8> },
    /// Sync response
    SyncResponse { response: Vec<u8> },
    /// Ping message
    Ping { timestamp: u64 },
    /// Pong response
    Pong { timestamp: u64 },
    /// Error message
    Error {
        error_code: u32,
        error_message: String,
    },
}

/// Legacy connection information
#[derive(Debug, Clone)]
pub struct LegacyConnectionInfo {
    /// Remote socket address
    pub remote_addr: SocketAddr,
    /// Connection established timestamp
    pub established: std::time::Instant,
    /// Last activity timestamp
    pub last_activity: std::time::Instant,
    /// Connection status
    pub status: LegacyConnectionStatus,
    /// Peer version
    pub version: Option<String>,
    /// Instance ID
    pub instance_id: Option<String>,
}

/// Legacy connection status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyConnectionStatus {
    /// Connection established, waiting for handshake
    Connecting,
    /// Handshake completed, connection active
    Connected,
    /// Connection closed
    Closed,
    /// Connection error
    Error(String),
}

/// Legacy network service that handles TCP connections
pub struct LegacyNetworkService {
    /// Network configuration
    config: NetworkConfig,
    /// TCP listener for incoming connections
    listener: Option<TcpListener>,
    /// Active legacy connections
    connections: Arc<RwLock<HashMap<SocketAddr, LegacyConnectionInfo>>>,
    /// Message adapter for protocol conversion
    message_adapter: MessageAdapter,
    /// Channel for sending messages to the bridge
    bridge_tx: mpsc::Sender<InternalMessage>,
    /// Service running flag
    running: bool,
    /// Bridge status reference
    bridge_status: Arc<RwLock<BridgeStatus>>,
}

impl LegacyNetworkService {
    /// Create a new legacy network service
    pub fn new(
        config: NetworkConfig,
        bridge_tx: mpsc::Sender<InternalMessage>,
        bridge_status: Arc<RwLock<BridgeStatus>>,
    ) -> P2pResult<Self> {
        let message_adapter = MessageAdapter::new();

        Ok(Self {
            config,
            listener: None,
            connections: Arc::new(RwLock::new(HashMap::new())),
            message_adapter,
            bridge_tx,
            running: false,
            bridge_status,
        })
    }

    /// Start the legacy network service
    pub async fn start(&mut self) -> P2pResult<()> {
        if self.running {
            return Err(crate::p2p::P2pError::Bridge(
                crate::p2p::bridge::BridgeError::ConfigurationError(
                    "Legacy service already running".to_string(),
                ),
            ));
        }

        // Bind TCP listener
        let listen_addr = format!("0.0.0.0:{}", self.config.transport.tcp_port);
        let listener = TcpListener::bind(&listen_addr).await.map_err(|e| {
            crate::p2p::P2pError::Bridge(crate::p2p::bridge::BridgeError::ConnectionError(format!(
                "Failed to bind TCP listener: {}",
                e
            )))
        })?;

        info!("Legacy TCP listener bound to {}", listen_addr);
        self.listener = Some(listener);
        self.running = true;

        // Start accepting connections
        self.accept_connections().await?;

        Ok(())
    }

    /// Stop the legacy network service
    pub async fn stop(&mut self) -> P2pResult<()> {
        if !self.running {
            return Ok(());
        }

        self.running = false;

        // Close all connections
        let mut connections = self.connections.write().await;
        for (addr, conn_info) in connections.iter_mut() {
            conn_info.status = LegacyConnectionStatus::Closed;
            info!("Closed legacy connection to {}", addr);
        }
        connections.clear();

        // Close listener
        if let Some(listener) = self.listener.take() {
            drop(listener);
        }

        info!("Legacy network service stopped");
        Ok(())
    }

    /// Accept incoming TCP connections
    async fn accept_connections(&mut self) -> P2pResult<()> {
        let listener = self.listener.as_mut().ok_or_else(|| {
            crate::p2p::P2pError::Bridge(crate::p2p::bridge::BridgeError::ConfigurationError(
                "TCP listener not initialized".to_string(),
            ))
        })?;

        let connections = self.connections.clone();
        let bridge_tx = self.bridge_tx.clone();
        let message_adapter = self.message_adapter.clone();

        // Spawn connection acceptor
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((socket, addr)) => {
                        debug!("Legacy TCP connection from {}", addr);

                        // Add connection to tracking
                        let conn_info = LegacyConnectionInfo {
                            remote_addr: addr,
                            established: std::time::Instant::now(),
                            last_activity: std::time::Instant::now(),
                            status: LegacyConnectionStatus::Connecting,
                            version: None,
                            instance_id: None,
                        };

                        {
                            let mut connections = connections.write().await;
                            connections.insert(addr, conn_info);
                        }

                        // Spawn connection handler
                        let connections_clone = connections.clone();
                        let bridge_tx_clone = bridge_tx.clone();
                        let message_adapter_clone = message_adapter.clone();

                        tokio::spawn(async move {
                            Self::handle_legacy_connection(
                                socket,
                                addr,
                                connections_clone,
                                bridge_tx_clone,
                                message_adapter_clone,
                            )
                            .await;
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept legacy TCP connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle individual legacy TCP connection
    async fn handle_legacy_connection(
        socket: TcpStream,
        addr: SocketAddr,
        connections: Arc<RwLock<HashMap<SocketAddr, LegacyConnectionInfo>>>,
        bridge_tx: mpsc::Sender<InternalMessage>,
        message_adapter: MessageAdapter,
    ) {
        let codec = LengthDelimitedCodec::new();
        let framed = Framed::new(socket, SymmetricalBincode::<LegacyPeerMessage>::new(codec));

        let (mut tx, mut rx) = framed.split();

        // Send handshake
        let handshake = LegacyPeerMessage::Handshake {
            version: "0.3.0".to_string(),
            instance_id: format!("neptune-legacy-{}", uuid::Uuid::new_v4()),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        if let Err(e) = tx.send(handshake).await {
            error!("Failed to send handshake to {}: {}", addr, e);
            Self::close_connection(addr, connections).await;
            return;
        }

        // Update connection status
        {
            let mut connections = connections.write().await;
            if let Some(conn_info) = connections.get_mut(&addr) {
                conn_info.status = LegacyConnectionStatus::Connected;
            }
        }

        // Handle incoming messages
        while let Some(result) = rx.next().await {
            match result {
                Ok(message) => {
                    // Update last activity
                    {
                        let mut connections = connections.write().await;
                        if let Some(conn_info) = connections.get_mut(&addr) {
                            conn_info.last_activity = std::time::Instant::now();

                            // Extract version and instance ID from handshake
                            if let LegacyPeerMessage::Handshake {
                                version,
                                instance_id,
                                ..
                            } = &message
                            {
                                conn_info.version = Some(version.clone());
                                conn_info.instance_id = Some(instance_id.clone());
                            }
                        }
                    }

                    // Convert legacy message to internal format
                    let internal_message = message_adapter.legacy_to_internal(message);

                    // Send to bridge
                    if let Err(e) = bridge_tx.send(internal_message).await {
                        error!("Failed to send message to bridge: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!("Error reading from legacy connection {}: {}", addr, e);
                    break;
                }
            }
        }

        // Close connection
        Self::close_connection(addr, connections).await;
    }

    /// Close a legacy connection
    async fn close_connection(
        addr: SocketAddr,
        connections: Arc<RwLock<HashMap<SocketAddr, LegacyConnectionInfo>>>,
    ) {
        let mut connections = connections.write().await;
        if let Some(conn_info) = connections.get_mut(&addr) {
            conn_info.status = LegacyConnectionStatus::Closed;
        }
        connections.remove(&addr);
        info!("Legacy connection to {} closed", addr);
    }

    /// Get connection count
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Get connection information
    pub async fn get_connections(&self) -> Vec<LegacyConnectionInfo> {
        self.connections.read().await.values().cloned().collect()
    }

    /// Check if service is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Send message to legacy peer
    pub async fn send_message(
        &self,
        addr: SocketAddr,
        message: LegacyPeerMessage,
    ) -> P2pResult<()> {
        // TODO: Implement sending messages to specific legacy peers
        // This requires maintaining a map of active connections and their writers
        debug!("Sending message to legacy peer {}: {:?}", addr, message);
        Ok(())
    }
}

impl Drop for LegacyNetworkService {
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
    async fn test_legacy_service_creation() {
        let config = NetworkConfig::default();
        let (bridge_tx, _) = mpsc::channel(100);
        let bridge_status = Arc::new(RwLock::new(BridgeStatus {
            migration_phase: MigrationPhase::DualProtocol,
            legacy_connections: 0,
            libp2p_connections: 0,
            migration_progress: 0.0,
        }));

        let service = LegacyNetworkService::new(config, bridge_tx, bridge_status);
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_legacy_service_lifecycle() {
        let mut config = NetworkConfig::default();
        config.transport.tcp_port = 0; // Let OS assign port for testing

        let (bridge_tx, _) = mpsc::channel(100);
        let bridge_status = Arc::new(RwLock::new(BridgeStatus {
            migration_phase: MigrationPhase::DualProtocol,
            legacy_connections: 0,
            libp2p_connections: 0,
            migration_progress: 0.0,
        }));

        let mut service = LegacyNetworkService::new(config, bridge_tx, bridge_status).unwrap();

        // Service should not be running initially
        assert!(!service.is_running());

        // Start service
        assert!(service.start().await.is_ok());
        assert!(service.is_running());

        // Stop service
        assert!(service.stop().await.is_ok());
        assert!(!service.is_running());
    }

    #[test]
    fn test_legacy_config_access() {
        let config = NetworkConfig::default();
        let (bridge_tx, _) = mpsc::channel(100);
        let bridge_status = Arc::new(RwLock::new(BridgeStatus {
            migration_phase: MigrationPhase::DualProtocol,
            legacy_connections: 0,
            libp2p_connections: 0,
            migration_progress: 0.0,
        }));

        // This would need to be async in real usage
        // For now, just test the config access
        assert_eq!(config.transport.tcp_port, 9798);
        assert!(config.transport.enable_tcp);
    }
}
