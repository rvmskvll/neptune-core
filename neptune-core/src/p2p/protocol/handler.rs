//! Protocol handler for Neptune P2P networking
//! 
//! This module handles processing and routing of protocol messages
//! and integrates with the existing Neptune Core application logic.

use super::{ProtocolError, MessageType, ProtocolVersion, ProtocolCapabilities};
use super::messages::{ProtocolMessage, MessagePayload, MessagePriority};
use crate::p2p::P2pResult;
use crate::p2p::bridge::{LegacyBridge, InternalMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Protocol handler for Neptune P2P networking
pub struct ProtocolHandler {
    /// Handler configuration
    config: ProtocolConfig,
    /// Message processing channels
    message_channels: MessageChannels,
    /// Active subscriptions
    subscriptions: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// Message routing table
    routing_table: Arc<RwLock<HashMap<MessageType, Vec<String>>>>,
    /// Handler status
    status: HandlerStatus,
    /// Legacy bridge for compatibility
    legacy_bridge: Option<Arc<LegacyBridge>>,
}

/// Protocol handler configuration
#[derive(Debug, Clone)]
pub struct ProtocolConfig {
    /// Maximum message size
    pub max_message_size: usize,
    /// Message processing timeout
    pub processing_timeout: std::time::Duration,
    /// Enable message signing
    pub enable_signing: bool,
    /// Enable message encryption
    pub enable_encryption: bool,
    /// Message priority levels
    pub priority_levels: u8,
    /// Enable message deduplication
    pub enable_deduplication: bool,
    /// Message cache size
    pub message_cache_size: usize,
}

/// Message processing channels
#[derive(Debug)]
pub struct MessageChannels {
    /// Incoming message channel
    pub incoming_tx: mpsc::Sender<ProtocolMessage>,
    /// Outgoing message channel
    pub outgoing_tx: mpsc::Sender<ProtocolMessage>,
    /// Internal message channel
    pub internal_tx: mpsc::Sender<InternalMessage>,
    /// Error message channel
    pub error_tx: mpsc::Sender<ProtocolError>,
}

/// Handler status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerStatus {
    /// Handler is starting
    Starting,
    /// Handler is running
    Running,
    /// Handler is stopping
    Stopping,
    /// Handler is stopped
    Stopped,
    /// Handler encountered an error
    Error(String),
}

impl ProtocolHandler {
    /// Create a new protocol handler
    pub async fn new(config: ProtocolConfig) -> P2pResult<Self> {
        let (incoming_tx, _) = mpsc::channel(1000);
        let (outgoing_tx, _) = mpsc::channel(1000);
        let (internal_tx, _) = mpsc::channel(1000);
        let (error_tx, _) = mpsc::channel(1000);

        let message_channels = MessageChannels {
            incoming_tx,
            outgoing_tx,
            internal_tx,
            error_tx,
        };

        Ok(Self {
            config,
            message_channels,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            routing_table: Arc::new(RwLock::new(HashMap::new())),
            status: HandlerStatus::Stopped,
            legacy_bridge: None,
        })
    }

    /// Start the protocol handler
    pub async fn start(&mut self) -> P2pResult<()> {
        info!("Starting protocol handler...");
        self.status = HandlerStatus::Starting;

        // Initialize routing table
        self.initialize_routing_table().await?;

        // Start message processing loop
        self.start_message_processing_loop().await?;

        self.status = HandlerStatus::Running;
        info!("Protocol handler started successfully");
        Ok(())
    }

    /// Stop the protocol handler
    pub async fn stop(&mut self) -> P2pResult<()> {
        info!("Stopping protocol handler...");
        self.status = HandlerStatus::Stopping;

        // Clear subscriptions and routing
        {
            let mut subscriptions = self.subscriptions.write().await;
            subscriptions.clear();
        }
        {
            let mut routing_table = self.routing_table.write().await;
            routing_table.clear();
        }

        self.status = HandlerStatus::Stopped;
        info!("Protocol handler stopped");
        Ok(())
    }

    /// Initialize the routing table with default routes
    async fn initialize_routing_table(&mut self) -> P2pResult<()> {
        let mut routing_table = self.routing_table.write().await;

        // Handshake messages - route to all peers
        routing_table.insert(MessageType::Handshake, vec!["*".to_string()]);

        // Block messages - route to mining peers and validators
        routing_table.insert(MessageType::Block, vec!["mining".to_string(), "validation".to_string()]);

        // Transaction messages - route to all peers for propagation
        routing_table.insert(MessageType::Transaction, vec!["*".to_string()]);

        // Peer list messages - route to discovery service
        routing_table.insert(MessageType::PeerList, vec!["discovery".to_string()]);

        // Sync challenge messages - route to validation peers
        routing_table.insert(MessageType::SyncChallenge, vec!["validation".to_string()]);

        // Ping/Pong messages - route to connection peers
        routing_table.insert(MessageType::Ping, vec!["connection".to_string()]);
        routing_table.insert(MessageType::Pong, vec!["connection".to_string()]);

        // Error messages - route to source peer
        routing_table.insert(MessageType::Error, vec!["source".to_string()]);

        Ok(())
    }

    /// Start the message processing loop
    async fn start_message_processing_loop(&mut self) -> P2pResult<()> {
        let config = self.config.clone();
        let subscriptions = self.subscriptions.clone();
        let routing_table = self.routing_table.clone();
        let legacy_bridge = self.legacy_bridge.clone();

        tokio::spawn(async move {
            loop {
                // Process incoming messages
                // This would integrate with the actual message queue
                debug!("Message processing tick - would process messages");
                
                // Sleep for a short interval
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        });

        Ok(())
    }

    /// Process an incoming protocol message
    pub async fn process_message(&mut self, message: ProtocolMessage) -> P2pResult<()> {
        // Validate message
        message.validate()?;

        // Check message size
        if message.size() as usize > self.config.max_message_size {
            return Err(ProtocolError::MessageTooLarge(
                format!("Message size {} exceeds limit {}", message.size(), self.config.max_message_size)
            ).into());
        }

        // Process message based on type
        match message.message_type() {
            MessageType::Handshake => self.handle_handshake(message).await,
            MessageType::Block => self.handle_block(message).await,
            MessageType::Transaction => self.handle_transaction(message).await,
            MessageType::PeerList => self.handle_peer_list(message).await,
            MessageType::SyncChallenge => self.handle_sync_challenge(message).await,
            MessageType::Ping => self.handle_ping(message).await,
            MessageType::Pong => self.handle_pong(message).await,
            MessageType::Error => self.handle_error(message).await,
            MessageType::Custom => self.handle_custom(message).await,
            MessageType::Unknown => {
                warn!("Received unknown message type: {:?}", message);
                Ok(())
            }
        }
    }

    /// Handle handshake messages
    async fn handle_handshake(&mut self, message: ProtocolMessage) -> P2pResult<()> {
        debug!("Processing handshake message from {}", message.source_peer_id());

        if let MessagePayload::Handshake(handshake) = &message.payload {
            // Validate handshake data
            self.validate_handshake(handshake).await?;

            // Send handshake response
            let response = self.create_handshake_response(&message).await?;
            self.send_message(response).await?;

            // Update peer information
            self.update_peer_info(&message).await?;

            info!("Handshake completed with peer {}", message.source_peer_id());
        }

        Ok(())
    }

    /// Handle block messages
    async fn handle_block(&mut self, message: ProtocolMessage) -> P2pResult<()> {
        debug!("Processing block message from {}", message.source_peer_id());

        if let MessagePayload::Block(block) = &message.payload {
            // Validate block data
            self.validate_block(block).await?;

            // Process block (would integrate with mining loop)
            self.process_block(block).await?;

            // Broadcast to other peers if needed
            if message.is_broadcast() {
                self.broadcast_block(block).await?;
            }

            info!("Block {} processed from peer {}", block.block_hash, message.source_peer_id());
        }

        Ok(())
    }

    /// Handle transaction messages
    async fn handle_transaction(&mut self, message: ProtocolMessage) -> P2pResult<()> {
        debug!("Processing transaction message from {}", message.source_peer_id());

        if let MessagePayload::Transaction(transaction) = &message.payload {
            // Validate transaction data
            self.validate_transaction(transaction).await?;

            // Process transaction (would integrate with transaction pool)
            self.process_transaction(transaction).await?;

            // Broadcast to other peers
            self.broadcast_transaction(transaction).await?;

            info!("Transaction {} processed from peer {}", transaction.transaction_hash, message.source_peer_id());
        }

        Ok(())
    }

    /// Handle peer list messages
    async fn handle_peer_list(&mut self, message: ProtocolMessage) -> P2pResult<()> {
        debug!("Processing peer list message from {}", message.source_peer_id());

        if let MessagePayload::PeerList(peer_list) = &message.payload {
            if peer_list.is_request {
                // Send peer list response
                let response = self.create_peer_list_response(&message).await?;
                self.send_message(response).await?;
            } else {
                // Process received peer list
                self.process_peer_list(peer_list).await?;
            }
        }

        Ok(())
    }

    /// Handle sync challenge messages
    async fn handle_sync_challenge(&mut self, message: ProtocolMessage) -> P2pResult<()> {
        debug!("Processing sync challenge from {}", message.source_peer_id());

        if let MessagePayload::SyncChallenge(challenge) = &message.payload {
            // Process sync challenge (would integrate with blockchain sync)
            let response = self.process_sync_challenge(challenge).await?;
            
            // Send challenge response
            let response_message = self.create_sync_challenge_response(&message, response).await?;
            self.send_message(response_message).await?;
        }

        Ok(())
    }

    /// Handle ping messages
    async fn handle_ping(&mut self, message: ProtocolMessage) -> P2pResult<()> {
        debug!("Processing ping from {}", message.source_peer_id());

        // Send pong response
        let pong = self.create_pong_response(&message).await?;
        self.send_message(pong).await?;

        Ok(())
    }

    /// Handle pong messages
    async fn handle_pong(&mut self, message: ProtocolMessage) -> P2pResult<()> {
        debug!("Processing pong from {}", message.source_peer_id());

        // Update peer latency information
        self.update_peer_latency(&message).await?;

        Ok(())
    }

    /// Handle error messages
    async fn handle_error(&mut self, message: ProtocolMessage) -> P2pResult<()> {
        if let MessagePayload::Error(error) = &message.payload {
            warn!("Error from peer {}: {} (code: {})", 
                  message.source_peer_id(), 
                  error.error_message, 
                  error.error_code);
            
            // Handle error based on code
            self.handle_peer_error(&message, error).await?;
        }

        Ok(())
    }

    /// Handle custom messages
    async fn handle_custom(&mut self, message: ProtocolMessage) -> P2pResult<()> {
        if let MessagePayload::Custom(custom) = &message.payload {
            debug!("Custom message from {}: {}", message.source_peer_id(), custom.custom_type);
            
            // Process custom message
            self.process_custom_message(custom).await?;
        }

        Ok(())
    }

    /// Validate handshake data
    async fn validate_handshake(&self, handshake: &super::messages::HandshakeMessage) -> P2pResult<()> {
        // Check node version compatibility
        if handshake.node_version.is_empty() {
            return Err(ProtocolError::InvalidHandshake(
                "Node version cannot be empty".to_string()
            ).into());
        }

        // Check network ID compatibility
        if handshake.network_id != "mainnet" && handshake.network_id != "testnet" {
            return Err(ProtocolError::InvalidHandshake(
                format!("Unsupported network ID: {}", handshake.network_id)
            ).into());
        }

        // Check genesis hash compatibility
        if handshake.genesis_hash.is_empty() {
            return Err(ProtocolError::InvalidHandshake(
                "Genesis hash cannot be empty".to_string()
            ).into());
        }

        Ok(())
    }

    /// Validate block data
    async fn validate_block(&self, block: &super::messages::BlockMessage) -> P2pResult<()> {
        // Check block hash
        if block.block_hash.is_empty() {
            return Err(ProtocolError::InvalidBlock(
                "Block hash cannot be empty".to_string()
            ).into());
        }

        // Check block data
        if block.block_data.is_empty() {
            return Err(ProtocolError::InvalidBlock(
                "Block data cannot be empty".to_string()
            ).into());
        }

        // Check block size
        if block.block_size != block.block_data.len() as u32 {
            return Err(ProtocolError::InvalidBlock(
                "Block size mismatch".to_string()
            ).into());
        }

        Ok(())
    }

    /// Validate transaction data
    async fn validate_transaction(&self, transaction: &super::messages::TransactionMessage) -> P2pResult<()> {
        // Check transaction hash
        if transaction.transaction_hash.is_empty() {
            return Err(ProtocolError::InvalidTransaction(
                "Transaction hash cannot be empty".to_string()
            ).into());
        }

        // Check transaction data
        if transaction.transaction_data.is_empty() {
            return Err(ProtocolError::InvalidTransaction(
                "Transaction data cannot be empty".to_string()
            ).into());
        }

        // Check transaction size
        if transaction.transaction_size != transaction.transaction_data.len() as u32 {
            return Err(ProtocolError::InvalidTransaction(
                "Transaction size mismatch".to_string()
            ).into());
        }

        Ok(())
    }

    /// Create handshake response
    async fn create_handshake_response(&self, original_message: &ProtocolMessage) -> P2pResult<ProtocolMessage> {
        // This would create a proper handshake response
        // For now, return a simple acknowledgment
        Ok(ProtocolMessage::new(
            MessageType::Handshake,
            ProtocolVersion::V1,
            "neptune-core".to_string(),
            original_message.source_peer_id().to_string(),
            MessagePayload::Handshake(super::messages::HandshakeMessage {
                node_version: "0.3.0".to_string(),
                supported_versions: vec![ProtocolVersion::V1],
                capabilities: ProtocolCapabilities::default(),
                network_id: "mainnet".to_string(),
                genesis_hash: "genesis_hash".to_string(),
                block_height: 1000,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            }),
        ))
    }

    /// Create peer list response
    async fn create_peer_list_response(&self, original_message: &ProtocolMessage) -> P2pResult<ProtocolMessage> {
        // This would create a peer list response
        // For now, return empty peer list
        Ok(ProtocolMessage::new(
            MessageType::PeerList,
            ProtocolVersion::V1,
            "neptune-core".to_string(),
            original_message.source_peer_id().to_string(),
            MessagePayload::PeerList(super::messages::PeerListMessage {
                peers: vec![],
                is_request: false,
                max_peers: 10,
            }),
        ))
    }

    /// Create sync challenge response
    async fn create_sync_challenge_response(&self, original_message: &ProtocolMessage, response: Vec<u8>) -> P2pResult<ProtocolMessage> {
        // This would create a sync challenge response
        // For now, return simple response
        Ok(ProtocolMessage::new(
            MessageType::SyncChallenge,
            ProtocolVersion::V1,
            "neptune-core".to_string(),
            original_message.source_peer_id().to_string(),
            MessagePayload::SyncChallenge(super::messages::SyncChallengeMessage {
                challenge_data: response,
                difficulty: 1,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                expected_response: "response_hash".to_string(),
            }),
        ))
    }

    /// Create pong response
    async fn create_pong_response(&self, ping_message: &ProtocolMessage) -> P2pResult<ProtocolMessage> {
        if let MessagePayload::PingPong(ping) = &ping_message.payload {
            Ok(ProtocolMessage::new(
                MessageType::Pong,
                ProtocolVersion::V1,
                "neptune-core".to_string(),
                ping_message.source_peer_id().to_string(),
                MessagePayload::PingPong(super::messages::PingPongMessage {
                    ping_type: super::messages::PingType::Pong,
                    sequence: ping.sequence,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    payload: None,
                }),
            ))
        } else {
            Err(ProtocolError::InvalidMessage(
                "Expected ping message".to_string()
            ).into())
        }
    }

    /// Send a message
    async fn send_message(&self, message: ProtocolMessage) -> P2pResult<()> {
        // Send to outgoing channel
        self.message_channels.outgoing_tx.send(message).await
            .map_err(|e| ProtocolError::SendError(
                format!("Failed to send message: {}", e)
            ))?;

        Ok(())
    }

    /// Update peer information
    async fn update_peer_info(&self, message: &ProtocolMessage) -> P2pResult<()> {
        // This would update peer information in the peer registry
        debug!("Updating peer info for {}", message.source_peer_id());
        Ok(())
    }

    /// Process block
    async fn process_block(&self, block: &super::messages::BlockMessage) -> P2pResult<()> {
        // This would integrate with the mining loop and blockchain
        debug!("Processing block {} at height {}", block.block_hash, block.block_height);
        Ok(())
    }

    /// Broadcast block
    async fn broadcast_block(&self, block: &super::messages::BlockMessage) -> P2pResult<()> {
        // This would broadcast the block to other peers
        debug!("Broadcasting block {} to peers", block.block_hash);
        Ok(())
    }

    /// Process transaction
    async fn process_transaction(&self, transaction: &super::messages::TransactionMessage) -> P2pResult<()> {
        // This would integrate with the transaction pool
        debug!("Processing transaction {}", transaction.transaction_hash);
        Ok(())
    }

    /// Broadcast transaction
    async fn broadcast_transaction(&self, transaction: &super::messages::TransactionMessage) -> P2pResult<()> {
        // This would broadcast the transaction to other peers
        debug!("Broadcasting transaction {} to peers", transaction.transaction_hash);
        Ok(())
    }

    /// Process peer list
    async fn process_peer_list(&self, peer_list: &super::messages::PeerListMessage) -> P2pResult<()> {
        // This would update the peer registry
        debug!("Processing peer list with {} peers", peer_list.peers.len());
        Ok(())
    }

    /// Process sync challenge
    async fn process_sync_challenge(&self, challenge: &super::messages::SyncChallengeMessage) -> P2pResult<Vec<u8>> {
        // This would process the sync challenge
        debug!("Processing sync challenge with difficulty {}", challenge.difficulty);
        Ok(vec![1, 2, 3, 4]) // Simple response
    }

    /// Update peer latency
    async fn update_peer_latency(&self, message: &ProtocolMessage) -> P2pResult<()> {
        // This would update peer latency information
        debug!("Updating latency for peer {}", message.source_peer_id());
        Ok(())
    }

    /// Handle peer error
    async fn handle_peer_error(&self, message: &ProtocolMessage, error: &super::messages::ErrorMessage) -> P2pResult<()> {
        // This would handle peer errors based on error code
        debug!("Handling error from peer {}: {}", message.source_peer_id(), error.error_message);
        Ok(())
    }

    /// Process custom message
    async fn process_custom_message(&self, custom: &super::messages::CustomMessage) -> P2pResult<()> {
        // This would process custom messages
        debug!("Processing custom message: {}", custom.custom_type);
        Ok(())
    }

    /// Set legacy bridge
    pub fn set_legacy_bridge(&mut self, bridge: Arc<LegacyBridge>) {
        self.legacy_bridge = Some(bridge);
    }

    /// Get handler status
    pub fn status(&self) -> HandlerStatus {
        self.status.clone()
    }

    /// Get configuration
    pub fn config(&self) -> &ProtocolConfig {
        &self.config
    }

    /// Subscribe to message type
    pub async fn subscribe(&mut self, message_type: MessageType, subscriber: String) -> P2pResult<()> {
        let mut subscriptions = self.subscriptions.write().await;
        let key = format!("{:?}", message_type);
        
        if let Some(subscribers) = subscriptions.get_mut(&key) {
            if !subscribers.contains(&subscriber) {
                subscribers.push(subscriber);
            }
        } else {
            subscriptions.insert(key, vec![subscriber]);
        }

        Ok(())
    }

    /// Unsubscribe from message type
    pub async fn unsubscribe(&mut self, message_type: MessageType, subscriber: String) -> P2pResult<()> {
        let mut subscriptions = self.subscriptions.write().await;
        let key = format!("{:?}", message_type);
        
        if let Some(subscribers) = subscriptions.get_mut(&key) {
            subscribers.retain(|s| s != &subscriber);
        }

        Ok(())
    }
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            max_message_size: 1024 * 1024, // 1MB
            processing_timeout: std::time::Duration::from_secs(30),
            enable_signing: true,
            enable_encryption: true,
            priority_levels: 4,
            enable_deduplication: true,
            message_cache_size: 1000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::protocol::{MessageType, ProtocolVersion, ProtocolCapabilities};
    use crate::p2p::protocol::messages::{MessagePayload, PingPongMessage, PingType};

    #[tokio::test]
    async fn test_protocol_handler_creation() {
        let config = ProtocolConfig::default();
        let handler = ProtocolHandler::new(config).await;
        assert!(handler.is_ok());
    }

    #[tokio::test]
    async fn test_protocol_handler_lifecycle() {
        let config = ProtocolConfig::default();
        let mut handler = ProtocolHandler::new(config).await.unwrap();
        
        // Handler should not be running initially
        assert_eq!(handler.status(), HandlerStatus::Stopped);
        
        // Start handler
        assert!(handler.start().await.is_ok());
        assert_eq!(handler.status(), HandlerStatus::Running);
        
        // Stop handler
        assert!(handler.stop().await.is_ok());
        assert_eq!(handler.status(), HandlerStatus::Stopped);
    }

    #[test]
    fn test_protocol_config_default() {
        let config = ProtocolConfig::default();
        assert_eq!(config.max_message_size, 1024 * 1024);
        assert_eq!(config.processing_timeout, std::time::Duration::from_secs(30));
        assert!(config.enable_signing);
        assert!(config.enable_encryption);
        assert_eq!(config.priority_levels, 4);
        assert!(config.enable_deduplication);
        assert_eq!(config.message_cache_size, 1000);
    }

    #[tokio::test]
    async fn test_message_processing() {
        let config = ProtocolConfig::default();
        let mut handler = ProtocolHandler::new(config).await.unwrap();
        
        let message = ProtocolMessage::new(
            MessageType::Ping,
            ProtocolVersion::V1,
            "peer1".to_string(),
            "peer2".to_string(),
            MessagePayload::PingPong(PingPongMessage {
                ping_type: PingType::Ping,
                sequence: 1,
                timestamp: 1234567890,
                payload: None,
            }),
        );

        // Process message
        assert!(handler.process_message(message).await.is_ok());
    }

    #[tokio::test]
    async fn test_subscription_management() {
        let config = ProtocolConfig::default();
        let mut handler = ProtocolHandler::new(config).await.unwrap();
        
        // Subscribe to message type
        assert!(handler.subscribe(MessageType::Block, "subscriber1".to_string()).await.is_ok());
        
        // Unsubscribe from message type
        assert!(handler.unsubscribe(MessageType::Block, "subscriber1".to_string()).await.is_ok());
    }
}
