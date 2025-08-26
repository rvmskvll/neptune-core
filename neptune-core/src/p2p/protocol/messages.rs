//! Protocol messages for Neptune P2P networking
//! 
//! This module defines the protocol message types and structures
//! used for communication between Neptune nodes.

use super::{ProtocolError, MessageType, ProtocolVersion, ProtocolCapabilities};
use crate::p2p::P2pResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Protocol message for Neptune P2P networking
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolMessage {
    /// Message header
    pub header: MessageHeader,
    /// Message payload
    pub payload: MessagePayload,
    /// Message signature (if signed)
    pub signature: Option<Vec<u8>>,
    /// Timestamp when message was created
    pub timestamp: u64,
}

/// Message header containing metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageHeader {
    /// Message type identifier
    pub message_type: MessageType,
    /// Protocol version
    pub protocol_version: ProtocolVersion,
    /// Message ID for deduplication
    pub message_id: String,
    /// Source peer ID
    pub source_peer_id: String,
    /// Target peer ID (empty for broadcast)
    pub target_peer_id: String,
    /// Message priority
    pub priority: MessagePriority,
    /// Time-to-live for routing
    pub ttl: u8,
    /// Message size in bytes
    pub size: u32,
}

/// Message payload containing the actual data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessagePayload {
    /// Handshake message for peer establishment
    Handshake(HandshakeMessage),
    /// Block message for blockchain synchronization
    Block(BlockMessage),
    /// Transaction message for transaction propagation
    Transaction(TransactionMessage),
    /// Peer list request/response
    PeerList(PeerListMessage),
    /// Sync challenge for peer validation
    SyncChallenge(SyncChallengeMessage),
    /// Ping/Pong for connection health
    PingPong(PingPongMessage),
    /// Error message for error reporting
    Error(ErrorMessage),
    /// Custom message for extensibility
    Custom(CustomMessage),
}

/// Message priority levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    /// Low priority - best effort delivery
    Low = 0,
    /// Normal priority - standard delivery
    Normal = 1,
    /// High priority - expedited delivery
    High = 2,
    /// Critical priority - immediate delivery
    Critical = 3,
}

/// Handshake message for peer establishment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandshakeMessage {
    /// Node version
    pub node_version: String,
    /// Supported protocol versions
    pub supported_versions: Vec<ProtocolVersion>,
    /// Node capabilities
    pub capabilities: ProtocolCapabilities,
    /// Network identifier
    pub network_id: String,
    /// Genesis block hash
    pub genesis_hash: String,
    /// Current block height
    pub block_height: u64,
    /// Timestamp of handshake
    pub timestamp: u64,
}

/// Block message for blockchain synchronization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockMessage {
    /// Block hash
    pub block_hash: String,
    /// Block height
    pub block_height: u64,
    /// Previous block hash
    pub previous_hash: String,
    /// Block timestamp
    pub timestamp: u64,
    /// Block data (serialized)
    pub block_data: Vec<u8>,
    /// Block size in bytes
    pub block_size: u32,
}

/// Transaction message for transaction propagation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionMessage {
    /// Transaction hash
    pub transaction_hash: String,
    /// Transaction data (serialized)
    pub transaction_data: Vec<u8>,
    /// Transaction size in bytes
    pub transaction_size: u32,
    /// Transaction fee
    pub fee: u64,
    /// Transaction timestamp
    pub timestamp: u64,
}

/// Peer list message for peer discovery
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerListMessage {
    /// List of peer addresses
    pub peers: Vec<PeerInfo>,
    /// Request or response flag
    pub is_request: bool,
    /// Maximum number of peers to return
    pub max_peers: u32,
}

/// Peer information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerInfo {
    /// Peer ID
    pub peer_id: String,
    /// Peer address
    pub address: String,
    /// Peer port
    pub port: u16,
    /// Peer version
    pub version: String,
    /// Last seen timestamp
    pub last_seen: u64,
    /// Peer score/reputation
    pub score: i32,
}

/// Sync challenge message for peer validation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncChallengeMessage {
    /// Challenge data
    pub challenge_data: Vec<u8>,
    /// Challenge difficulty
    pub difficulty: u32,
    /// Challenge timestamp
    pub timestamp: u64,
    /// Expected response hash
    pub expected_response: String,
}

/// Ping/Pong message for connection health
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PingPongMessage {
    /// Message type (ping or pong)
    pub ping_type: PingType,
    /// Sequence number for matching
    pub sequence: u64,
    /// Timestamp for RTT calculation
    pub timestamp: u64,
    /// Optional payload data
    pub payload: Option<Vec<u8>>,
}

/// Ping message type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PingType {
    /// Ping request
    Ping,
    /// Pong response
    Pong,
}

/// Error message for error reporting
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorMessage {
    /// Error code
    pub error_code: u32,
    /// Error message
    pub error_message: String,
    /// Error details
    pub error_details: Option<String>,
    /// Timestamp of error
    pub timestamp: u64,
}

/// Custom message for extensibility
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomMessage {
    /// Custom message type
    pub custom_type: String,
    /// Custom data
    pub custom_data: Vec<u8>,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

impl ProtocolMessage {
    /// Create a new protocol message
    pub fn new(
        message_type: MessageType,
        protocol_version: ProtocolVersion,
        source_peer_id: String,
        target_peer_id: String,
        payload: MessagePayload,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let message_id = Self::generate_message_id(&message_type, &source_peer_id, timestamp);
        let size = Self::calculate_message_size(&payload);

        let header = MessageHeader {
            message_type,
            protocol_version,
            message_id,
            source_peer_id,
            target_peer_id,
            priority: MessagePriority::Normal,
            ttl: 64, // Default TTL
            size,
        };

        Self {
            header,
            payload,
            signature: None,
            timestamp,
        }
    }

    /// Create a handshake message
    pub fn handshake(
        protocol_version: ProtocolVersion,
        source_peer_id: String,
        node_version: String,
        supported_versions: Vec<ProtocolVersion>,
        capabilities: ProtocolCapabilities,
        network_id: String,
        genesis_hash: String,
        block_height: u64,
    ) -> Self {
        let payload = MessagePayload::Handshake(HandshakeMessage {
            node_version,
            supported_versions,
            capabilities,
            network_id,
            genesis_hash,
            block_height,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });

        Self::new(
            MessageType::Handshake,
            protocol_version,
            source_peer_id,
            String::new(), // Broadcast
            payload,
        )
    }

    /// Create a block message
    pub fn block(
        protocol_version: ProtocolVersion,
        source_peer_id: String,
        target_peer_id: String,
        block_hash: String,
        block_height: u64,
        previous_hash: String,
        block_data: Vec<u8>,
    ) -> Self {
        let payload = MessagePayload::Block(BlockMessage {
            block_hash,
            block_height,
            previous_hash,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            block_data,
            block_size: 0, // Will be calculated
        });

        let mut message = Self::new(
            MessageType::Block,
            protocol_version,
            source_peer_id,
            target_peer_id,
            payload,
        );

        // Update size after creation
        message.header.size = Self::calculate_message_size(&message.payload);
        message
    }

    /// Create a transaction message
    pub fn transaction(
        protocol_version: ProtocolVersion,
        source_peer_id: String,
        target_peer_id: String,
        transaction_hash: String,
        transaction_data: Vec<u8>,
        fee: u64,
    ) -> Self {
        let payload = MessagePayload::Transaction(TransactionMessage {
            transaction_hash,
            transaction_data: transaction_data.clone(),
            transaction_size: transaction_data.len() as u32,
            fee,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });

        Self::new(
            MessageType::Transaction,
            protocol_version,
            source_peer_id,
            target_peer_id,
            payload,
        )
    }

    /// Create a ping message
    pub fn ping(
        protocol_version: ProtocolVersion,
        source_peer_id: String,
        target_peer_id: String,
        sequence: u64,
    ) -> Self {
        let payload = MessagePayload::PingPong(PingPongMessage {
            ping_type: PingType::Ping,
            sequence,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            payload: None,
        });

        Self::new(
            MessageType::Ping,
            protocol_version,
            source_peer_id,
            target_peer_id,
            payload,
        )
    }

    /// Create a pong message in response to a ping
    pub fn pong(
        protocol_version: ProtocolVersion,
        source_peer_id: String,
        target_peer_id: String,
        sequence: u64,
    ) -> Self {
        let payload = MessagePayload::PingPong(PingPongMessage {
            ping_type: PingType::Pong,
            sequence,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            payload: None,
        });

        Self::new(
            MessageType::Pong,
            protocol_version,
            source_peer_id,
            target_peer_id,
            payload,
        )
    }

    /// Create an error message
    pub fn error(
        protocol_version: ProtocolVersion,
        source_peer_id: String,
        target_peer_id: String,
        error_code: u32,
        error_message: String,
        error_details: Option<String>,
    ) -> Self {
        let payload = MessagePayload::Error(ErrorMessage {
            error_code,
            error_message,
            error_details,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });

        Self::new(
            MessageType::Error,
            protocol_version,
            source_peer_id,
            target_peer_id,
            payload,
        )
    }

    /// Get message type
    pub fn message_type(&self) -> &MessageType {
        &self.header.message_type
    }

    /// Get protocol version
    pub fn protocol_version(&self) -> &ProtocolVersion {
        &self.header.protocol_version
    }

    /// Get message ID
    pub fn message_id(&self) -> &str {
        &self.header.message_id
    }

    /// Get source peer ID
    pub fn source_peer_id(&self) -> &str {
        &self.header.source_peer_id
    }

    /// Get target peer ID
    pub fn target_peer_id(&self) -> &str {
        &self.header.target_peer_id
    }

    /// Check if message is broadcast
    pub fn is_broadcast(&self) -> bool {
        self.header.target_peer_id.is_empty()
    }

    /// Check if message is for specific peer
    pub fn is_for_peer(&self, peer_id: &str) -> bool {
        self.is_broadcast() || self.header.target_peer_id == peer_id
    }

    /// Get message priority
    pub fn priority(&self) -> &MessagePriority {
        &self.header.priority
    }

    /// Set message priority
    pub fn set_priority(&mut self, priority: MessagePriority) {
        self.header.priority = priority;
    }

    /// Get message TTL
    pub fn ttl(&self) -> u8 {
        self.header.ttl
    }

    /// Set message TTL
    pub fn set_ttl(&mut self, ttl: u8) {
        self.header.ttl = ttl;
    }

    /// Decrement TTL
    pub fn decrement_ttl(&mut self) -> bool {
        if self.header.ttl > 0 {
            self.header.ttl -= 1;
            true
        } else {
            false
        }
    }

    /// Check if message has expired
    pub fn has_expired(&self) -> bool {
        self.header.ttl == 0
    }

    /// Get message size
    pub fn size(&self) -> u32 {
        self.header.size
    }

    /// Get message timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Check if message is signed
    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    /// Get message signature
    pub fn signature(&self) -> Option<&[u8]> {
        self.signature.as_deref()
    }

    /// Set message signature
    pub fn set_signature(&mut self, signature: Vec<u8>) {
        self.signature = Some(signature);
    }

    /// Validate message
    pub fn validate(&self) -> P2pResult<()> {
        // Check timestamp
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        if self.timestamp > current_time + 300 { // 5 minutes in future
            return Err(ProtocolError::InvalidTimestamp(
                "Message timestamp is too far in the future".to_string()
            ).into());
        }

        // Check TTL
        if self.header.ttl > 255 {
            return Err(ProtocolError::InvalidTtl(
                "Message TTL exceeds maximum value".to_string()
            ).into());
        }

        // Check message size
        let calculated_size = Self::calculate_message_size(&self.payload);
        if self.header.size != calculated_size {
            return Err(ProtocolError::InvalidSize(
                format!("Message size mismatch: expected {}, got {}", calculated_size, self.header.size)
            ).into());
        }

        // Check message ID format
        if self.header.message_id.is_empty() {
            return Err(ProtocolError::InvalidMessageId(
                "Message ID cannot be empty".to_string()
            ).into());
        }

        Ok(())
    }

    /// Generate a unique message ID
    fn generate_message_id(
        message_type: &MessageType,
        source_peer_id: &str,
        timestamp: u64,
    ) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        message_type.hash(&mut hasher);
        source_peer_id.hash(&mut hasher);
        timestamp.hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }

    /// Calculate message size
    fn calculate_message_size(payload: &MessagePayload) -> u32 {
        // This is a simplified calculation
        // In practice, you'd want to serialize the payload to get exact size
        match payload {
            MessagePayload::Handshake(_) => 256,
            MessagePayload::Block(block) => block.block_data.len() as u32 + 128,
            MessagePayload::Transaction(tx) => tx.transaction_data.len() as u32 + 64,
            MessagePayload::PeerList(_) => 512,
            MessagePayload::SyncChallenge(_) => 128,
            MessagePayload::PingPong(_) => 64,
            MessagePayload::Error(_) => 128,
            MessagePayload::Custom(custom) => custom.custom_data.len() as u32 + 64,
        }
    }
}

impl Default for MessagePriority {
    fn default() -> Self {
        Self::Normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::protocol::{MessageType, ProtocolVersion, ProtocolCapabilities};

    #[test]
    fn test_protocol_message_creation() {
        let message = ProtocolMessage::new(
            MessageType::Handshake,
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

        assert_eq!(message.message_type(), &MessageType::Handshake);
        assert_eq!(message.protocol_version(), &ProtocolVersion::V1);
        assert_eq!(message.source_peer_id(), "peer1");
        assert_eq!(message.target_peer_id(), "peer2");
        assert!(!message.is_broadcast());
    }

    #[test]
    fn test_handshake_message() {
        let capabilities = ProtocolCapabilities::default();
        let message = ProtocolMessage::handshake(
            ProtocolVersion::V1,
            "peer1".to_string(),
            "1.0.0".to_string(),
            vec![ProtocolVersion::V1],
            capabilities,
            "mainnet".to_string(),
            "genesis_hash".to_string(),
            1000,
        );

        assert_eq!(message.message_type(), &MessageType::Handshake);
        assert!(message.is_broadcast());
        
        if let MessagePayload::Handshake(handshake) = &message.payload {
            assert_eq!(handshake.node_version, "1.0.0");
            assert_eq!(handshake.network_id, "mainnet");
            assert_eq!(handshake.block_height, 1000);
        } else {
            panic!("Expected handshake payload");
        }
    }

    #[test]
    fn test_block_message() {
        let message = ProtocolMessage::block(
            ProtocolVersion::V1,
            "peer1".to_string(),
            "peer2".to_string(),
            "block_hash".to_string(),
            1001,
            "prev_hash".to_string(),
            vec![1, 2, 3, 4],
        );

        assert_eq!(message.message_type(), &MessageType::Block);
        assert_eq!(message.target_peer_id(), "peer2");
        
        if let MessagePayload::Block(block) = &message.payload {
            assert_eq!(block.block_hash, "block_hash");
            assert_eq!(block.block_height, 1001);
            assert_eq!(block.previous_hash, "prev_hash");
            assert_eq!(block.block_data, vec![1, 2, 3, 4]);
        } else {
            panic!("Expected block payload");
        }
    }

    #[test]
    fn test_ping_pong_messages() {
        let ping = ProtocolMessage::ping(
            ProtocolVersion::V1,
            "peer1".to_string(),
            "peer2".to_string(),
            42,
        );

        let pong = ProtocolMessage::pong(
            ProtocolVersion::V1,
            "peer2".to_string(),
            "peer1".to_string(),
            42,
        );

        assert_eq!(ping.message_type(), &MessageType::Ping);
        assert_eq!(pong.message_type(), &MessageType::Pong);
        
        if let MessagePayload::PingPong(ping_payload) = &ping.payload {
            assert_eq!(ping_payload.ping_type, PingType::Ping);
            assert_eq!(ping_payload.sequence, 42);
        } else {
            panic!("Expected ping payload");
        }
        
        if let MessagePayload::PingPong(pong_payload) = &pong.payload {
            assert_eq!(pong_payload.ping_type, PingType::Pong);
            assert_eq!(pong_payload.sequence, 42);
        } else {
            panic!("Expected pong payload");
        }
    }

    #[test]
    fn test_message_validation() {
        let mut message = ProtocolMessage::new(
            MessageType::Handshake,
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

        // Valid message should pass validation
        assert!(message.validate().is_ok());

        // Invalid TTL should fail validation
        message.header.ttl = 300; // Exceeds max
        assert!(message.validate().is_err());

        // Reset TTL
        message.header.ttl = 64;
        assert!(message.validate().is_ok());
    }

    #[test]
    fn test_message_priority() {
        let mut message = ProtocolMessage::new(
            MessageType::Handshake,
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

        assert_eq!(message.priority(), &MessagePriority::Normal);
        
        message.set_priority(MessagePriority::High);
        assert_eq!(message.priority(), &MessagePriority::High);
    }

    #[test]
    fn test_message_ttl() {
        let mut message = ProtocolMessage::new(
            MessageType::Handshake,
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

        assert_eq!(message.ttl(), 64);
        assert!(!message.has_expired());
        
        message.set_ttl(2);
        assert_eq!(message.ttl(), 2);
        
        assert!(message.decrement_ttl());
        assert_eq!(message.ttl(), 1);
        
        assert!(message.decrement_ttl());
        assert_eq!(message.ttl(), 0);
        assert!(message.has_expired());
        
        assert!(!message.decrement_ttl());
    }

    #[test]
    fn test_message_signature() {
        let mut message = ProtocolMessage::new(
            MessageType::Handshake,
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

        assert!(!message.is_signed());
        assert_eq!(message.signature(), None);
        
        let signature = vec![1, 2, 3, 4];
        message.set_signature(signature.clone());
        
        assert!(message.is_signed());
        assert_eq!(message.signature(), Some(signature.as_slice()));
    }

    #[test]
    fn test_peer_targeting() {
        let message = ProtocolMessage::new(
            MessageType::Handshake,
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

        assert!(!message.is_broadcast());
        assert!(message.is_for_peer("peer2"));
        assert!(!message.is_for_peer("peer3"));
        
        let broadcast_message = ProtocolMessage::new(
            MessageType::Handshake,
            ProtocolVersion::V1,
            "peer1".to_string(),
            String::new(), // Empty target = broadcast
            MessagePayload::PingPong(PingPongMessage {
                ping_type: PingType::Ping,
                sequence: 1,
                timestamp: 1234567890,
                payload: None,
            }),
        );
        
        assert!(broadcast_message.is_broadcast());
        assert!(broadcast_message.is_for_peer("any_peer"));
    }
}
