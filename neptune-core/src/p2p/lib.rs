//! P2P networking layer for Neptune Core
//! 
//! This module provides a modular networking implementation that supports both
//! legacy TCP-based protocols and modern libp2p protocols through a compatibility bridge.
//! 
//! ## Architecture
//! 
//! - **`network/`** - Core libp2p networking service
//! - **`protocol/`** - Application protocol message handling
//! - **`bridge/`** - Legacy protocol compatibility layer
//! - **`config/`** - Network configuration management
//! 
//! ## Usage
//! 
//! ```rust
//! use neptune_core::p2p::{NetworkService, NetworkConfig};
//! 
//! let config = NetworkConfig::default();
//! let network = NetworkService::new(config).await?;
//! network.start().await?;
//! ```

pub mod bridge;
pub mod config;
pub mod network;
pub mod protocol;

// Re-export main types for easy access
pub use bridge::LegacyBridge;
pub use config::NetworkConfig;
pub use network::NetworkService;
pub use protocol::ProtocolHandler;

use std::error::Error;
use std::fmt;

/// Result type for P2P operations
pub type P2pResult<T> = Result<T, P2pError>;

/// Error type for P2P operations
#[derive(Debug, thiserror::Error)]
pub enum P2pError {
    #[error("Network service error: {0}")]
    Network(#[from] network::NetworkError),
    
    #[error("Protocol error: {0}")]
    Protocol(#[from] protocol::ProtocolError),
    
    #[error("Bridge error: {0}")]
    Bridge(#[from] bridge::BridgeError),
    
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),
    
    #[error("Legacy compatibility error: {0}")]
    Legacy(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl P2pError {
    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(self, 
            P2pError::Network(_) | 
            P2pError::Protocol(_) |
            P2pError::Config(_)
        )
    }
    
    /// Check if this error indicates a legacy protocol issue
    pub fn is_legacy_related(&self) -> bool {
        matches!(self, P2pError::Legacy(_) | P2pError::Bridge(_))
    }
}

/// Network service status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkStatus {
    /// Service is starting up
    Starting,
    /// Service is running normally
    Running,
    /// Service is in compatibility mode (legacy + libp2p)
    CompatibilityMode,
    /// Service is shutting down
    ShuttingDown,
    /// Service has encountered an error
    Error(String),
}

/// Network metrics for monitoring
#[derive(Debug, Clone)]
pub struct NetworkMetrics {
    /// Number of active libp2p connections
    pub libp2p_connections: usize,
    /// Number of active legacy connections
    pub legacy_connections: usize,
    /// Total messages processed
    pub messages_processed: u64,
    /// Network latency in milliseconds
    pub average_latency_ms: f64,
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        Self {
            libp2p_connections: 0,
            legacy_connections: 0,
            messages_processed: 0,
            average_latency_ms: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p2p_error_recoverable() {
        let network_error = P2pError::Network(network::NetworkError::ConnectionFailed);
        assert!(network_error.is_recoverable());
        
        let internal_error = P2pError::Internal("test".to_string());
        assert!(!internal_error.is_recoverable());
    }

    #[test]
    fn test_p2p_error_legacy_related() {
        let legacy_error = P2pError::Legacy("test".to_string());
        assert!(legacy_error.is_legacy_related());
        
        let config_error = P2pError::Config(config::ConfigError::InvalidPort);
        assert!(!config_error.is_legacy_related());
    }

    #[test]
    fn test_network_metrics_default() {
        let metrics = NetworkMetrics::default();
        assert_eq!(metrics.libp2p_connections, 0);
        assert_eq!(metrics.legacy_connections, 0);
        assert_eq!(metrics.messages_processed, 0);
        assert_eq!(metrics.average_latency_ms, 0.0);
    }
}
