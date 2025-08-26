//! Message adapter for protocol conversion between legacy and libp2p
//! 
//! This module provides conversion between different message formats:
//! - Legacy Neptune Core TCP messages
//! - libp2p protocol messages
//! - Internal bridge messages

use super::legacy::LegacyPeerMessage;
use crate::p2p::protocol::messages::{ProtocolMessage, MessagePayload, MessageHeader, MessageType};
use crate::p2p::bridge::{InternalMessage, BridgeError};
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Message adapter for converting between different protocol formats
#[derive(Debug, Clone)]
pub struct MessageAdapter {
    /// Current protocol version
    protocol_version: String,
    /// Maximum message size
    max_message_size: usize,
}

impl MessageAdapter {
    /// Create a new message adapter
    pub fn new() -> Self {
        Self {
            protocol_version: "0.3.0".to_string(),
            max_message_size: 1024 * 1024, // 1MB
        }
    }

    /// Convert legacy message to internal format
    pub fn legacy_to_internal(&self, message: LegacyPeerMessage) -> InternalMessage {
        match message {
            LegacyPeerMessage::Handshake { version, instance_id, timestamp } => {
                InternalMessage::Handshake {
                    version,
                    instance_id,
                    timestamp,
                    source: "legacy".to_string(),
                }
            }
            LegacyPeerMessage::Block { block_data, block_hash } => {
                InternalMessage::Block {
                    block_data,
                    block_hash,
                    source: "legacy".to_string(),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                }
            }
            LegacyPeerMessage::Transaction { tx_data, tx_hash } => {
                InternalMessage::Transaction {
                    tx_data,
                    tx_hash,
                    source: "legacy".to_string(),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                }
            }
            LegacyPeerMessage::PeerListRequest => {
                InternalMessage::PeerListRequest {
                    source: "legacy".to_string(),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                }
            }
            LegacyPeerMessage::PeerListResponse { peers } => {
                InternalMessage::PeerListResponse {
                    peers: peers.into_iter().map(|addr| addr.to_string()).collect(),
                    source: "legacy".to_string(),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                }
            }
            LegacyPeerMessage::SyncChallenge { challenge } => {
                InternalMessage::SyncChallenge {
                    challenge,
                    source: "legacy".to_string(),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                }
            }
            LegacyPeerMessage::SyncResponse { response } => {
                InternalMessage::SyncResponse {
                    response,
                    source: "legacy".to_string(),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                }
            }
            LegacyPeerMessage::Ping { timestamp } => {
                InternalMessage::Ping {
                    timestamp,
                    source: "legacy".to_string(),
                }
            }
            LegacyPeerMessage::Pong { timestamp } => {
                InternalMessage::Pong {
                    timestamp,
                    source: "legacy".to_string(),
                }
            }
            LegacyPeerMessage::Error { error_code, error_message } => {
                InternalMessage::Error {
                    error_code,
                    error_message,
                    source: "legacy".to_string(),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                }
            }
        }
    }

    /// Convert internal message to legacy format
    pub fn internal_to_legacy(&self, message: &InternalMessage) -> Result<LegacyPeerMessage, BridgeError> {
        match message {
            InternalMessage::Handshake { version, instance_id, timestamp, .. } => {
                Ok(LegacyPeerMessage::Handshake {
                    version: version.clone(),
                    instance_id: instance_id.clone(),
                    timestamp: *timestamp,
                })
            }
            InternalMessage::Block { block_data, block_hash, .. } => {
                Ok(LegacyPeerMessage::Block {
                    block_data: block_data.clone(),
                    block_hash: block_hash.clone(),
                })
            }
            InternalMessage::Transaction { tx_data, tx_hash, .. } => {
                Ok(LegacyPeerMessage::Transaction {
                    tx_data: tx_data.clone(),
                    tx_hash: tx_hash.clone(),
                })
            }
            InternalMessage::PeerListRequest { .. } => {
                Ok(LegacyPeerMessage::PeerListRequest)
            }
            InternalMessage::PeerListResponse { peers, .. } => {
                let socket_addrs: Result<Vec<SocketAddr>, _> = peers
                    .iter()
                    .map(|addr| addr.parse())
                    .collect();
                
                Ok(LegacyPeerMessage::PeerListResponse {
                    peers: socket_addrs.map_err(|_| BridgeError::MessageConversion(
                        "Failed to parse peer addresses".to_string()
                    ))?,
                })
            }
            InternalMessage::SyncChallenge { challenge, .. } => {
                Ok(LegacyPeerMessage::SyncChallenge {
                    challenge: challenge.clone(),
                })
            }
            InternalMessage::SyncResponse { response, .. } => {
                Ok(LegacyPeerMessage::SyncResponse {
                    response: response.clone(),
                })
            }
            InternalMessage::Ping { timestamp, .. } => {
                Ok(LegacyPeerMessage::Ping {
                    timestamp: *timestamp,
                })
            }
            InternalMessage::Pong { timestamp, .. } => {
                Ok(LegacyPeerMessage::Pong {
                    timestamp: *timestamp,
                })
            }
            InternalMessage::Error { error_code, error_message, .. } => {
                Ok(LegacyPeerMessage::Error {
                    error_code: *error_code,
                    error_message: error_message.clone(),
                })
            }
            _ => {
                Err(BridgeError::MessageConversion(
                    format!("Unsupported internal message type: {:?}", message)
                ))
            }
        }
    }

    /// Convert libp2p message to internal format
    pub fn libp2p_to_internal(&self, message: ProtocolMessage) -> Result<InternalMessage, BridgeError> {
        // Validate message size
        if !self.validate_message_size(&message) {
            return Err(BridgeError::MessageConversion(
                "Message size exceeds limit".to_string()
            ));
        }

        let payload = message.payload;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        match payload {
            MessagePayload::Handshake(handshake) => {
                Ok(InternalMessage::Handshake {
                    version: handshake.version,
                    instance_id: handshake.instance_id,
                    timestamp: handshake.timestamp,
                    source: "libp2p".to_string(),
                })
            }
            MessagePayload::Block(block) => {
                Ok(InternalMessage::Block {
                    block_data: block.block_data,
                    block_hash: block.block_hash,
                    source: "libp2p".to_string(),
                    timestamp,
                })
            }
            MessagePayload::Transaction(tx) => {
                Ok(InternalMessage::Transaction {
                    tx_data: tx.tx_data,
                    tx_hash: tx.tx_hash,
                    source: "libp2p".to_string(),
                    timestamp,
                })
            }
            MessagePayload::PeerList(peer_list) => {
                Ok(InternalMessage::PeerListResponse {
                    peers: peer_list.peers.into_iter().map(|p| p.address).collect(),
                    source: "libp2p".to_string(),
                    timestamp,
                })
            }
            MessagePayload::SyncChallenge(sync) => {
                Ok(InternalMessage::SyncChallenge {
                    challenge: sync.challenge,
                    source: "libp2p".to_string(),
                    timestamp,
                })
            }
            MessagePayload::PingPong(ping) => {
                match ping.ping_type {
                    crate::p2p::protocol::messages::PingType::Ping => {
                        Ok(InternalMessage::Ping {
                            timestamp: ping.timestamp,
                            source: "libp2p".to_string(),
                        })
                    }
                    crate::p2p::protocol::messages::PingType::Pong => {
                        Ok(InternalMessage::Pong {
                            timestamp: ping.timestamp,
                            source: "libp2p".to_string(),
                        })
                    }
                }
            }
            MessagePayload::Error(err) => {
                Ok(InternalMessage::Error {
                    error_code: err.error_code,
                    error_message: err.error_message,
                    source: "libp2p".to_string(),
                    timestamp,
                })
            }
            MessagePayload::Custom(custom) => {
                // Handle custom messages based on type
                match custom.message_type.as_str() {
                    "peer_list_request" => {
                        Ok(InternalMessage::PeerListRequest {
                            source: "libp2p".to_string(),
                            timestamp,
                        })
                    }
                    "sync_response" => {
                        Ok(InternalMessage::SyncResponse {
                            response: custom.data,
                            source: "libp2p".to_string(),
                            timestamp,
                        })
                    }
                    _ => {
                        Err(BridgeError::MessageConversion(
                            format!("Unknown custom message type: {}", custom.message_type)
                        ))
                    }
                }
            }
        }
    }

    /// Convert internal message to libp2p format
    pub fn internal_to_libp2p(&self, message: &InternalMessage) -> Result<ProtocolMessage, BridgeError> {
        let header = MessageHeader {
            message_type: self.get_message_type(message),
            protocol_version: self.protocol_version.clone(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            priority: crate::p2p::protocol::messages::MessagePriority::Normal,
            ttl: 300, // 5 minutes
        };

        let payload = match message {
            InternalMessage::Handshake { version, instance_id, timestamp, .. } => {
                MessagePayload::Handshake(crate::p2p::protocol::messages::HandshakeMessage {
                    version: version.clone(),
                    instance_id: instance_id.clone(),
                    timestamp: *timestamp,
                })
            }
            InternalMessage::Block { block_data, block_hash, .. } => {
                MessagePayload::Block(crate::p2p::protocol::messages::BlockMessage {
                    block_data: block_data.clone(),
                    block_hash: block_hash.clone(),
                })
            }
            InternalMessage::Transaction { tx_data, tx_hash, .. } => {
                MessagePayload::Transaction(crate::p2p::protocol::messages::TransactionMessage {
                    tx_data: tx_data.clone(),
                    tx_hash: tx_hash.clone(),
                })
            }
            InternalMessage::PeerListRequest { .. } => {
                MessagePayload::Custom(crate::p2p::protocol::messages::CustomMessage {
                    message_type: "peer_list_request".to_string(),
                    data: Vec::new(),
                })
            }
            InternalMessage::PeerListResponse { peers, .. } => {
                let peer_infos: Vec<crate::p2p::protocol::messages::PeerInfo> = peers
                    .iter()
                    .map(|addr| crate::p2p::protocol::messages::PeerInfo {
                        address: addr.clone(),
                        version: Some(self.protocol_version.clone()),
                        last_seen: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    })
                    .collect();

                MessagePayload::PeerList(crate::p2p::protocol::messages::PeerListMessage {
                    peers: peer_infos,
                })
            }
            InternalMessage::SyncChallenge { challenge, .. } => {
                MessagePayload::SyncChallenge(crate::p2p::protocol::messages::SyncChallengeMessage {
                    challenge: challenge.clone(),
                })
            }
            InternalMessage::SyncResponse { response, .. } => {
                MessagePayload::Custom(crate::p2p::protocol::messages::CustomMessage {
                    message_type: "sync_response".to_string(),
                    data: response.clone(),
                })
            }
            InternalMessage::Ping { timestamp, .. } => {
                MessagePayload::PingPong(crate::p2p::protocol::messages::PingPongMessage {
                    ping_type: crate::p2p::protocol::messages::PingType::Ping,
                    timestamp: *timestamp,
                })
            }
            InternalMessage::Pong { timestamp, .. } => {
                MessagePayload::PingPong(crate::p2p::protocol::messages::PingPongMessage {
                    ping_type: crate::p2p::protocol::messages::PingType::Pong,
                    timestamp: *timestamp,
                })
            }
            InternalMessage::Error { error_code, error_message, .. } => {
                MessagePayload::Error(crate::p2p::protocol::messages::ErrorMessage {
                    error_code: *error_code,
                    error_message: error_message.clone(),
                })
            }
        };

        Ok(ProtocolMessage { header, payload })
    }

    /// Validate message size
    pub fn validate_message_size(&self, message: &ProtocolMessage) -> bool {
        // Calculate approximate message size
        let size = self.get_message_size(message);
        size <= self.max_message_size
    }

    /// Get message size in bytes
    pub fn get_message_size(&self, message: &ProtocolMessage) -> usize {
        // This is a simplified size calculation
        // In production, you'd want more accurate sizing
        let header_size = 64; // Approximate header size
        let payload_size = match &message.payload {
            MessagePayload::Handshake(h) => h.version.len() + h.instance_id.len() + 8,
            MessagePayload::Block(b) => b.block_data.len() + b.block_hash.len(),
            MessagePayload::Transaction(t) => t.tx_data.len() + t.tx_hash.len(),
            MessagePayload::PeerList(p) => p.peers.iter().map(|peer| peer.address.len() + 16).sum(),
            MessagePayload::SyncChallenge(s) => s.challenge.len(),
            MessagePayload::PingPong(_) => 8,
            MessagePayload::Error(e) => e.error_message.len() + 4,
            MessagePayload::Custom(c) => c.message_type.len() + c.data.len(),
        };
        
        header_size + payload_size
    }

    /// Get message type for internal message
    fn get_message_type(&self, message: &InternalMessage) -> MessageType {
        match message {
            InternalMessage::Handshake { .. } => MessageType::Handshake,
            InternalMessage::Block { .. } => MessageType::Block,
            InternalMessage::Transaction { .. } => MessageType::Transaction,
            InternalMessage::PeerListRequest { .. } => MessageType::PeerList,
            InternalMessage::PeerListResponse { .. } => MessageType::PeerList,
            InternalMessage::SyncChallenge { .. } => MessageType::SyncChallenge,
            InternalMessage::SyncResponse { .. } => MessageType::Custom,
            InternalMessage::Ping { .. } => MessageType::PingPong,
            InternalMessage::Pong { .. } => MessageType::PingPong,
            InternalMessage::Error { .. } => MessageType::Error,
        }
    }

    /// Get compression stats
    pub fn get_compression_stats(&self) -> crate::p2p::protocol::codec::CompressionStats {
        crate::p2p::protocol::codec::CompressionStats {
            original_size: 0,
            compressed_size: 0,
            compression_ratio: 0.0,
            algorithm: "none".to_string(),
        }
    }
}

impl Default for MessageAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::bridge::InternalMessage;

    #[test]
    fn test_message_adapter_creation() {
        let adapter = MessageAdapter::new();
        assert_eq!(adapter.protocol_version, "0.3.0");
        assert_eq!(adapter.max_message_size, 1024 * 1024);
    }

    #[test]
    fn test_legacy_to_internal_handshake() {
        let adapter = MessageAdapter::new();
        let legacy_msg = LegacyPeerMessage::Handshake {
            version: "0.3.0".to_string(),
            instance_id: "test-123".to_string(),
            timestamp: 1234567890,
        };

        let internal_msg = adapter.legacy_to_internal(legacy_msg);
        
        if let InternalMessage::Handshake { version, instance_id, timestamp, source } = internal_msg {
            assert_eq!(version, "0.3.0");
            assert_eq!(instance_id, "test-123");
            assert_eq!(timestamp, 1234567890);
            assert_eq!(source, "legacy");
        } else {
            panic!("Expected Handshake message");
        }
    }

    #[test]
    fn test_legacy_to_internal_block() {
        let adapter = MessageAdapter::new();
        let legacy_msg = LegacyPeerMessage::Block {
            block_data: vec![1, 2, 3, 4],
            block_hash: "abc123".to_string(),
        };

        let internal_msg = adapter.legacy_to_internal(legacy_msg);
        
        if let InternalMessage::Block { block_data, block_hash, source, .. } = internal_msg {
            assert_eq!(block_data, vec![1, 2, 3, 4]);
            assert_eq!(block_hash, "abc123");
            assert_eq!(source, "legacy");
        } else {
            panic!("Expected Block message");
        }
    }

    #[test]
    fn test_message_size_validation() {
        let adapter = MessageAdapter::new();
        
        // Test with small message
        let small_message = ProtocolMessage {
            header: MessageHeader {
                message_type: MessageType::PingPong,
                protocol_version: "0.3.0".to_string(),
                timestamp: 1234567890,
                priority: crate::p2p::protocol::messages::MessagePriority::Normal,
                ttl: 300,
            },
            payload: MessagePayload::PingPong(crate::p2p::protocol::messages::PingPongMessage {
                ping_type: crate::p2p::protocol::messages::PingType::Ping,
                timestamp: 1234567890,
            }),
        };
        
        assert!(adapter.validate_message_size(&small_message));
    }
}
