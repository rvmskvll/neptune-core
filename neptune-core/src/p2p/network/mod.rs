//! Core networking layer using libp2p
//! 
//! This module provides the libp2p-based networking service that will
//! eventually replace the legacy TCP networking implementation.

mod service;
mod transport;
mod discovery;

pub use service::NetworkService;
pub use transport::TransportService;
pub use discovery::DiscoveryService;

use crate::p2p::P2pResult;

/// Network service errors
#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Transport error: {0}")]
    TransportError(String),
    
    #[error("Discovery error: {0}")]
    DiscoveryError(String),
    
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

/// Network service status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceStatus {
    /// Service is starting
    Starting,
    /// Service is running
    Running,
    /// Service is stopping
    Stopping,
    /// Service has stopped
    Stopped,
    /// Service encountered an error
    Error(String),
}

/// Network connection information
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Peer ID
    pub peer_id: String,
    /// Remote address
    pub remote_addr: String,
    /// Connection direction (inbound/outbound)
    pub direction: ConnectionDirection,
    /// Connection established time
    pub established: std::time::Instant,
}

/// Connection direction
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionDirection {
    /// Incoming connection
    Inbound,
    /// Outgoing connection
    Outbound,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_direction() {
        assert_eq!(ConnectionDirection::Inbound, ConnectionDirection::Inbound);
        assert_eq!(ConnectionDirection::Outbound, ConnectionDirection::Outbound);
        assert_ne!(ConnectionDirection::Inbound, ConnectionDirection::Outbound);
    }

    #[test]
    fn test_service_status() {
        assert_eq!(ServiceStatus::Starting, ServiceStatus::Starting);
        assert_eq!(ServiceStatus::Running, ServiceStatus::Running);
        assert_eq!(ServiceStatus::Stopping, ServiceStatus::Stopping);
        assert_eq!(ServiceStatus::Stopped, ServiceStatus::Stopped);
    }
}
