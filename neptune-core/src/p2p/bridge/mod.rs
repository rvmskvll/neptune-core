//! Legacy protocol compatibility bridge for P2P networking
//! 
//! This module provides a bridge between the existing legacy TCP-based P2P
//! implementation and the new libp2p networking layer, ensuring backward
//! compatibility during the migration period.

mod adapter;
mod legacy;
mod migration;

pub use adapter::MessageAdapter;
pub use legacy::LegacyNetworkService;
pub use migration::MigrationManager;

use crate::p2p::{P2pResult, NetworkStatus, NetworkMetrics};
use crate::p2p::config::NetworkConfig;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Bridge error types
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Legacy protocol error: {0}")]
    LegacyProtocol(String),
    
    #[error("Message conversion error: {0}")]
    MessageConversion(String),
    
    #[error("Protocol mismatch: {0}")]
    ProtocolMismatch(String),
    
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    #[error("Migration error: {0}")]
    MigrationError(String),
}

/// Bridge status information
#[derive(Debug, Clone)]
pub struct BridgeStatus {
    /// Current migration phase
    pub migration_phase: MigrationPhase,
    /// Number of legacy connections
    pub legacy_connections: usize,
    /// Number of libp2p connections
    pub libp2p_connections: usize,
    /// Migration progress percentage
    pub migration_progress: f64,
}

/// Migration phases for the bridge
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationPhase {
    /// Initial phase - both protocols running
    DualProtocol,
    /// Migration phase - gradually moving peers
    Migration,
    /// Final phase - mostly libp2p with legacy fallback
    Libp2pPrimary,
    /// Complete - legacy disabled
    Complete,
}

/// Main compatibility bridge that manages both legacy and libp2p networking
pub struct LegacyBridge {
    /// Network configuration
    config: NetworkConfig,
    /// Legacy network service
    legacy_service: Option<LegacyNetworkService>,
    /// Migration manager
    migration_manager: MigrationManager,
    /// Bridge status
    status: Arc<RwLock<BridgeStatus>>,
    /// Message adapter for protocol conversion
    message_adapter: MessageAdapter,
}

impl LegacyBridge {
    /// Create a new compatibility bridge
    pub async fn new(config: NetworkConfig) -> P2pResult<Self> {
        // Validate configuration
        config.validate()?;
        
        let status = Arc::new(RwLock::new(BridgeStatus {
            migration_phase: MigrationPhase::DualProtocol,
            legacy_connections: 0,
            libp2p_connections: 0,
            migration_progress: 0.0,
        }));
        
        let migration_manager = MigrationManager::new(config.clone(), status.clone());
        let message_adapter = MessageAdapter::new();
        
        let mut bridge = Self {
            config,
            legacy_service: None,
            migration_manager,
            status,
            message_adapter,
        };
        
        // Initialize legacy service if enabled
        if bridge.config.legacy_enabled() {
            bridge.legacy_service = Some(LegacyNetworkService::new(
                bridge.config.legacy.clone(),
                bridge.status.clone(),
            ).await?);
        }
        
        Ok(bridge)
    }
    
    /// Start the bridge services
    pub async fn start(&mut self) -> P2pResult<()> {
        // Start legacy service if enabled
        if let Some(ref mut legacy_service) = self.legacy_service {
            legacy_service.start().await?;
        }
        
        // Start migration manager
        self.migration_manager.start().await?;
        
        // Update status
        let mut status = self.status.write().await;
        status.migration_phase = MigrationPhase::DualProtocol;
        status.migration_progress = 0.0;
        
        Ok(())
    }
    
    /// Stop the bridge services
    pub async fn stop(&mut self) -> P2pResult<()> {
        // Stop legacy service
        if let Some(ref mut legacy_service) = self.legacy_service {
            legacy_service.stop().await?;
        }
        
        // Stop migration manager
        self.migration_manager.stop().await?;
        
        // Update status
        let mut status = self.status.write().await;
        status.migration_phase = MigrationPhase::Complete;
        status.migration_progress = 100.0;
        
        Ok(())
    }
    
    /// Get current bridge status
    pub async fn status(&self) -> BridgeStatus {
        self.status.read().await.clone()
    }
    
    /// Get network metrics
    pub async fn metrics(&self) -> NetworkMetrics {
        let status = self.status.read().await;
        
        NetworkMetrics {
            libp2p_connections: status.libp2p_connections,
            legacy_connections: status.legacy_connections,
            messages_processed: 0, // TODO: Implement message counting
            average_latency_ms: 0.0, // TODO: Implement latency measurement
        }
    }
    
    /// Check if legacy networking is active
    pub fn legacy_active(&self) -> bool {
        self.legacy_service.is_some()
    }
    
    /// Check if bridge is in migration mode
    pub async fn is_migrating(&self) -> bool {
        let status = self.status.read().await;
        matches!(status.migration_phase, MigrationPhase::Migration)
    }
    
    /// Get migration progress
    pub async fn migration_progress(&self) -> f64 {
        let status = self.status.read().await;
        status.migration_progress
    }
    
    /// Force migration to next phase
    pub async fn force_migration(&mut self) -> P2pResult<()> {
        self.migration_manager.advance_phase().await?;
        Ok(())
    }
    
    /// Handle incoming message from either protocol
    pub async fn handle_message(
        &self,
        source: SocketAddr,
        message: Vec<u8>,
        protocol: MessageProtocol,
    ) -> P2pResult<Vec<u8>> {
        match protocol {
            MessageProtocol::Legacy => {
                // Convert legacy message to internal format
                let internal_msg = self.message_adapter.legacy_to_internal(message)?;
                
                // Process message and get response
                let response = self.process_internal_message(source, internal_msg).await?;
                
                // Convert response back to legacy format
                self.message_adapter.internal_to_legacy(response)
            }
            MessageProtocol::Libp2p => {
                // Convert libp2p message to internal format
                let internal_msg = self.message_adapter.libp2p_to_internal(message)?;
                
                // Process message and get response
                let response = self.process_internal_message(source, internal_msg).await?;
                
                // Convert response to libp2p format
                self.message_adapter.internal_to_libp2p(response)
            }
        }
    }
    
    /// Process internal message format
    async fn process_internal_message(
        &self,
        source: SocketAddr,
        message: InternalMessage,
    ) -> P2pResult<InternalMessage> {
        // TODO: Implement message processing logic
        // This will integrate with the existing Neptune message handling
        todo!("Implement internal message processing")
    }
}

/// Message protocol types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageProtocol {
    /// Legacy TCP-based protocol
    Legacy,
    /// libp2p protocol
    Libp2p,
}

/// Internal message format for bridge processing
#[derive(Debug, Clone)]
pub enum InternalMessage {
    /// Handshake message
    Handshake(HandshakeData),
    /// Block-related message
    Block(BlockMessage),
    /// Transaction-related message
    Transaction(TransactionMessage),
    /// Peer management message
    Peer(PeerMessage),
    /// Network control message
    Control(ControlMessage),
}

/// Handshake data for internal processing
#[derive(Debug, Clone)]
pub struct HandshakeData {
    pub version: String,
    pub network: String,
    pub instance_id: u128,
}

/// Block-related messages
#[derive(Debug, Clone)]
pub enum BlockMessage {
    Request(BlockRequest),
    Response(BlockResponse),
    Notification(BlockNotification),
}

/// Transaction-related messages
#[derive(Debug, Clone)]
pub enum TransactionMessage {
    Send(TransactionData),
    Request(TransactionRequest),
    Notification(TransactionNotification),
}

/// Peer management messages
#[derive(Debug, Clone)]
pub enum PeerMessage {
    ListRequest,
    ListResponse(Vec<SocketAddr>),
    Connect(SocketAddr),
    Disconnect(SocketAddr),
}

/// Network control messages
#[derive(Debug, Clone)]
pub enum ControlMessage {
    Status,
    Metrics,
    Shutdown,
}

/// Block request data
#[derive(Debug, Clone)]
pub struct BlockRequest {
    pub height: Option<u64>,
    pub hash: Option<String>,
}

/// Block response data
#[derive(Debug, Clone)]
pub struct BlockResponse {
    pub blocks: Vec<Vec<u8>>,
}

/// Block notification data
#[derive(Debug, Clone)]
pub struct BlockNotification {
    pub height: u64,
    pub hash: String,
}

/// Transaction data
#[derive(Debug, Clone)]
pub struct TransactionData {
    pub data: Vec<u8>,
}

/// Transaction request data
#[derive(Debug, Clone)]
pub struct TransactionRequest {
    pub id: String,
}

/// Transaction notification data
#[derive(Debug, Clone)]
pub struct TransactionNotification {
    pub id: String,
    pub timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::config::NetworkConfig;

    #[tokio::test]
    async fn test_bridge_creation() {
        let config = NetworkConfig::test_config();
        let bridge = LegacyBridge::new(config).await;
        assert!(bridge.is_ok());
    }

    #[tokio::test]
    async fn test_bridge_status() {
        let config = NetworkConfig::test_config();
        let bridge = LegacyBridge::new(config).await.unwrap();
        
        let status = bridge.status().await;
        assert_eq!(status.migration_phase, MigrationPhase::DualProtocol);
        assert_eq!(status.migration_progress, 0.0);
    }

    #[tokio::test]
    async fn test_bridge_legacy_active() {
        let config = NetworkConfig::test_config();
        let bridge = LegacyBridge::new(config).await.unwrap();
        
        assert!(bridge.legacy_active());
    }

    #[test]
    fn test_message_protocol() {
        assert_eq!(MessageProtocol::Legacy, MessageProtocol::Legacy);
        assert_eq!(MessageProtocol::Libp2p, MessageProtocol::Libp2p);
        assert_ne!(MessageProtocol::Legacy, MessageProtocol::Libp2p);
    }
}
