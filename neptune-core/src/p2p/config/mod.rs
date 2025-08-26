//! Network configuration management for P2P networking
//! 
//! This module provides configuration structures and validation for both
//! legacy and libp2p networking components.

mod settings;
mod validation;

pub use settings::NetworkConfig;
pub use validation::ConfigValidator;

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

/// Network mode configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkMode {
    /// Legacy TCP-based networking only
    LegacyOnly,
    /// libp2p networking only
    Libp2pOnly,
    /// Both legacy and libp2p (compatibility mode)
    Compatibility,
}

impl Default for NetworkMode {
    fn default() -> Self {
        NetworkMode::Compatibility
    }
}

/// Transport configuration for libp2p
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// TCP port to listen on
    pub tcp_port: u16,
    /// Enable Noise encryption
    pub enable_noise: bool,
    /// Enable Yamux multiplexing
    pub enable_yamux: bool,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Keep-alive interval
    pub keep_alive_interval: Duration,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            tcp_port: 9798,
            enable_noise: true,
            enable_yamux: true,
            connection_timeout: Duration::from_secs(30),
            keep_alive_interval: Duration::from_secs(60),
        }
    }
}

/// Discovery configuration for libp2p
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Enable Kademlia DHT
    pub enable_kademlia: bool,
    /// Enable mDNS for local discovery
    pub enable_mdns: bool,
    /// Bootstrap nodes for DHT
    pub bootstrap_nodes: Vec<String>,
    /// Discovery interval
    pub discovery_interval: Duration,
    /// Maximum peers to discover
    pub max_discovered_peers: usize,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            enable_kademlia: true,
            enable_mdns: true,
            bootstrap_nodes: vec![
                "QmNnooDu7bfjPFoTaLxpMpVq4uEbgvnq6ckv2ytLc4fqxG".to_string(),
            ],
            discovery_interval: Duration::from_secs(300),
            max_discovered_peers: 100,
        }
    }
}

/// Protocol configuration for libp2p
#[derive(Debug, Clone)]
pub struct ProtocolConfig {
    /// Enable gossipsub for block/transaction broadcasting
    pub enable_gossipsub: bool,
    /// Enable request-response for block/transaction requests
    pub enable_request_response: bool,
    /// Enable ping protocol for connection health
    pub enable_ping: bool,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Protocol timeout
    pub protocol_timeout: Duration,
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            enable_gossipsub: true,
            enable_request_response: true,
            enable_ping: true,
            max_message_size: 100 * 1024 * 1024, // 100MB
            protocol_timeout: Duration::from_secs(60),
        }
    }
}

/// Legacy compatibility configuration
#[derive(Debug, Clone)]
pub struct LegacyConfig {
    /// Enable legacy TCP networking
    pub enable_legacy: bool,
    /// Legacy TCP port
    pub legacy_port: u16,
    /// Legacy connection timeout
    pub legacy_timeout: Duration,
    /// Maximum legacy connections
    pub max_legacy_connections: usize,
}

impl Default for LegacyConfig {
    fn default() -> Self {
        Self {
            enable_legacy: true,
            legacy_port: 9798,
            legacy_timeout: Duration::from_secs(30),
            max_legacy_connections: 50,
        }
    }
}

/// Feature flags for network functionality
#[derive(Debug, Clone)]
pub struct FeatureFlags {
    /// Enable NAT traversal
    pub nat_traversal: bool,
    /// Enable connection resilience
    pub connection_resilience: bool,
    /// Enable performance monitoring
    pub performance_monitoring: bool,
    /// Enable detailed logging
    pub detailed_logging: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            nat_traversal: true,
            connection_resilience: true,
            performance_monitoring: true,
            detailed_logging: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_mode_default() {
        assert_eq!(NetworkMode::default(), NetworkMode::Compatibility);
    }

    #[test]
    fn test_transport_config_default() {
        let config = TransportConfig::default();
        assert_eq!(config.tcp_port, 9798);
        assert!(config.enable_noise);
        assert!(config.enable_yamux);
    }

    #[test]
    fn test_discovery_config_default() {
        let config = DiscoveryConfig::default();
        assert!(config.enable_kademlia);
        assert!(config.enable_mdns);
        assert!(!config.bootstrap_nodes.is_empty());
    }

    #[test]
    fn test_protocol_config_default() {
        let config = ProtocolConfig::default();
        assert!(config.enable_gossipsub);
        assert!(config.enable_request_response);
        assert!(config.enable_ping);
        assert_eq!(config.max_message_size, 100 * 1024 * 1024);
    }

    #[test]
    fn test_legacy_config_default() {
        let config = LegacyConfig::default();
        assert!(config.enable_legacy);
        assert_eq!(config.legacy_port, 9798);
        assert_eq!(config.max_legacy_connections, 50);
    }

    #[test]
    fn test_feature_flags_default() {
        let flags = FeatureFlags::default();
        assert!(flags.nat_traversal);
        assert!(flags.connection_resilience);
        assert!(flags.performance_monitoring);
        assert!(!flags.detailed_logging);
    }
}
