//! Application protocol layer for P2P networking
//! 
//! This module handles the application-specific protocol messages and
//! provides a unified interface for both legacy and libp2p protocols.

mod messages;
mod handler;
mod codec;

pub use messages::ProtocolMessage;
pub use handler::ProtocolHandler;
pub use codec::MessageCodec;

use crate::p2p::P2pResult;

/// Protocol errors
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("Message encoding error: {0}")]
    EncodingError(String),
    
    #[error("Message decoding error: {0}")]
    DecodingError(String),
    
    #[error("Invalid message format: {0}")]
    InvalidFormat(String),
    
    #[error("Protocol version mismatch: {0}")]
    VersionMismatch(String),
    
    #[error("Unsupported message type: {0}")]
    UnsupportedMessage(String),
}

/// Protocol message types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageType {
    /// Handshake messages
    Handshake,
    /// Block-related messages
    Block,
    /// Transaction-related messages
    Transaction,
    /// Peer management messages
    Peer,
    /// Network control messages
    Control,
    /// Unknown message type
    Unknown,
}

/// Protocol version information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolVersion {
    /// Major version number
    pub major: u8,
    /// Minor version number
    pub minor: u8,
    /// Patch version number
    pub patch: u8,
}

impl ProtocolVersion {
    /// Create a new protocol version
    pub fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self { major, minor, patch }
    }

    /// Check if this version is compatible with another
    pub fn is_compatible(&self, other: &ProtocolVersion) -> bool {
        // Major version must match exactly
        if self.major != other.major {
            return false;
        }
        
        // Minor version must be >= other minor
        if self.minor < other.minor {
            return false;
        }
        
        // Patch version differences are acceptable
        true
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self::new(1, 0, 0)
    }
}

/// Protocol capabilities
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolCapabilities {
    /// Supported message types
    pub supported_messages: Vec<MessageType>,
    /// Maximum message size
    pub max_message_size: usize,
    /// Supported protocol versions
    pub supported_versions: Vec<ProtocolVersion>,
}

impl Default for ProtocolCapabilities {
    fn default() -> Self {
        Self {
            supported_messages: vec![
                MessageType::Handshake,
                MessageType::Block,
                MessageType::Transaction,
                MessageType::Peer,
                MessageType::Control,
            ],
            max_message_size: 100 * 1024 * 1024, // 100MB
            supported_versions: vec![ProtocolVersion::default()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version_creation() {
        let version = ProtocolVersion::new(1, 2, 3);
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 3);
    }

    #[test]
    fn test_protocol_version_compatibility() {
        let v1_0_0 = ProtocolVersion::new(1, 0, 0);
        let v1_1_0 = ProtocolVersion::new(1, 1, 0);
        let v1_0_1 = ProtocolVersion::new(1, 0, 1);
        let v2_0_0 = ProtocolVersion::new(2, 0, 0);

        // Same major version, compatible minor versions
        assert!(v1_1_0.is_compatible(&v1_0_0));
        assert!(v1_0_1.is_compatible(&v1_0_0));
        
        // Different major versions are incompatible
        assert!(!v2_0_0.is_compatible(&v1_0_0));
        
        // Lower minor version is incompatible with higher
        assert!(!v1_0_0.is_compatible(&v1_1_0));
    }

    #[test]
    fn test_protocol_version_string() {
        let version = ProtocolVersion::new(1, 2, 3);
        assert_eq!(version.to_string(), "1.2.3");
    }

    #[test]
    fn test_protocol_version_default() {
        let version = ProtocolVersion::default();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_protocol_capabilities_default() {
        let capabilities = ProtocolCapabilities::default();
        assert!(!capabilities.supported_messages.is_empty());
        assert_eq!(capabilities.max_message_size, 100 * 1024 * 1024);
        assert!(!capabilities.supported_versions.is_empty());
    }

    #[test]
    fn test_message_type() {
        assert_eq!(MessageType::Handshake, MessageType::Handshake);
        assert_eq!(MessageType::Block, MessageType::Block);
        assert_eq!(MessageType::Transaction, MessageType::Transaction);
        assert_eq!(MessageType::Peer, MessageType::Peer);
        assert_eq!(MessageType::Control, MessageType::Control);
        assert_eq!(MessageType::Unknown, MessageType::Unknown);
    }
}
