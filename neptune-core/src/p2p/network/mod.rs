//! Core networking layer using libp2p
//!
//! This module provides the libp2p-based networking service that will
//! eventually replace the legacy TCP networking implementation.

mod discovery;
mod service;
mod transport;

pub use discovery::DiscoveryService;
pub use service::NetworkService;
pub use transport::TransportService;

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

/// Network health status and recommendations
#[derive(Debug, Clone)]
pub struct NetworkHealthStatus {
    /// Current number of connected peers
    pub peer_count: usize,
    /// Whether DHT is enabled
    pub dht_enabled: bool,
    /// Whether mDNS is enabled
    pub mdns_enabled: bool,
    /// Whether Rendezvous is enabled
    pub rendezvous_enabled: bool,
    /// Warning messages about network issues
    pub warnings: Vec<String>,
    /// Recommendations for improving network health
    pub recommendations: Vec<String>,
}

impl NetworkHealthStatus {
    /// Check if network is healthy
    pub fn is_healthy(&self) -> bool {
        self.peer_count > 0 && self.warnings.is_empty()
    }

    /// Get overall health score (0-100)
    pub fn health_score(&self) -> u8 {
        let mut score = 100u8;

        // Deduct points for warnings
        score = score.saturating_sub((self.warnings.len() * 10) as u8);

        // Deduct points for low peer count
        if self.peer_count == 0 {
            score = score.saturating_sub(50);
        } else if self.peer_count < 3 {
            score = score.saturating_sub(20);
        }

        // Deduct points for disabled discovery methods
        if !self.dht_enabled {
            score = score.saturating_sub(20);
        }
        if !self.mdns_enabled {
            score = score.saturating_sub(10);
        }
        if !self.rendezvous_enabled {
            score = score.saturating_sub(10);
        }

        score
    }

    /// Get summary of network status
    pub fn summary(&self) -> String {
        if self.is_healthy() {
            format!("Network healthy with {} peers", self.peer_count)
        } else {
            format!(
                "Network issues detected: {} peers, {} warnings",
                self.peer_count,
                self.warnings.len()
            )
        }
    }
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
