//! Main network configuration structure and builder

use super::*;
use crate::p2p::P2pResult;

/// Main network configuration that combines all networking components
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Network operating mode
    pub mode: NetworkMode,
    /// Transport configuration for libp2p
    pub transport: TransportConfig,
    /// Discovery configuration for libp2p
    pub discovery: DiscoveryConfig,
    /// Protocol configuration for libp2p
    pub protocol: ProtocolConfig,
    /// Legacy compatibility configuration
    pub legacy: LegacyConfig,
    /// Feature flags
    pub features: FeatureFlags,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            mode: NetworkMode::default(),
            transport: TransportConfig::default(),
            discovery: DiscoveryConfig::default(),
            protocol: ProtocolConfig::default(),
            legacy: LegacyConfig::default(),
            features: FeatureFlags::default(),
        }
    }
}

impl NetworkConfig {
    /// Create a new configuration with custom settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set network mode
    pub fn with_mode(mut self, mode: NetworkMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set transport configuration
    pub fn with_transport(mut self, transport: TransportConfig) -> Self {
        self.transport = transport;
        self
    }

    /// Set discovery configuration
    pub fn with_discovery(mut self, discovery: DiscoveryConfig) -> Self {
        self.discovery = discovery;
        self
    }

    /// Set protocol configuration
    pub fn with_protocol(mut self, protocol: ProtocolConfig) -> Self {
        self.protocol = protocol;
        self
    }

    /// Set legacy configuration
    pub fn with_legacy(mut self, legacy: LegacyConfig) -> Self {
        self.legacy = legacy;
        self
    }

    /// Set feature flags
    pub fn with_features(mut self, features: FeatureFlags) -> Self {
        self.features = features;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> P2pResult<()> {
        // Validate transport configuration
        if self.transport.tcp_port == 0 {
            return Err(crate::p2p::P2pError::Config(
                crate::p2p::config::ConfigError::InvalidPort,
            ));
        }

        // Validate discovery configuration
        if self.discovery.max_discovered_peers == 0 {
            return Err(crate::p2p::P2pError::Config(
                crate::p2p::config::ConfigError::InvalidPeerLimit,
            ));
        }

        // Validate protocol configuration
        if self.protocol.max_message_size == 0 {
            return Err(crate::p2p::P2pError::Config(
                crate::p2p::config::ConfigError::InvalidMessageSize,
            ));
        }

        // Validate legacy configuration
        if self.legacy.enable_legacy && self.legacy.max_legacy_connections == 0 {
            return Err(crate::p2p::P2pError::Config(
                crate::p2p::config::ConfigError::InvalidPeerLimit,
            ));
        }

        // Validate mode-specific requirements
        match self.mode {
            NetworkMode::LegacyOnly => {
                if !self.legacy.enable_legacy {
                    return Err(crate::p2p::P2pError::Config(
                        crate::p2p::config::ConfigError::ModeMismatch(
                            "LegacyOnly mode requires legacy networking to be enabled".to_string(),
                        ),
                    ));
                }
            }
            NetworkMode::Libp2pOnly => {
                if !self.transport.enable_noise || !self.transport.enable_yamux {
                    return Err(crate::p2p::P2pError::Config(
                        crate::p2p::config::ConfigError::ModeMismatch(
                            "Libp2pOnly mode requires full libp2p features".to_string(),
                        ),
                    ));
                }
            }
            NetworkMode::Compatibility => {
                // Both modes should be enabled for compatibility
                if !self.legacy.enable_legacy {
                    return Err(crate::p2p::P2pError::Config(
                        crate::p2p::config::ConfigError::ModeMismatch(
                            "Compatibility mode requires legacy networking to be enabled"
                                .to_string(),
                        ),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Check if legacy networking is enabled
    pub fn legacy_enabled(&self) -> bool {
        self.legacy.enable_legacy
            && (self.mode == NetworkMode::LegacyOnly || self.mode == NetworkMode::Compatibility)
    }

    /// Check if libp2p networking is enabled
    pub fn libp2p_enabled(&self) -> bool {
        self.mode == NetworkMode::Libp2pOnly || self.mode == NetworkMode::Compatibility
    }

    /// Get the primary listening port based on mode
    pub fn primary_port(&self) -> u16 {
        match self.mode {
            NetworkMode::LegacyOnly => self.legacy.legacy_port,
            NetworkMode::Libp2pOnly => self.transport.tcp_port,
            NetworkMode::Compatibility => self.transport.tcp_port,
        }
    }

    /// Create a minimal configuration for testing
    pub fn test_config() -> Self {
        Self {
            mode: NetworkMode::Compatibility,
            transport: TransportConfig {
                tcp_port: 0,         // Let OS assign port
                enable_noise: false, // Disable for testing
                enable_yamux: false, // Disable for testing
                connection_timeout: Duration::from_secs(5),
                keep_alive_interval: Duration::from_secs(10),
            },
            discovery: DiscoveryConfig {
                enable_kademlia: false,   // Disable for testing
                enable_mdns: false,       // Disable for testing
                enable_rendezvous: false, // Disable for testing
                bootstrap_nodes: vec![],
                neptune_bootstrap_nodes: vec![],
                discovery_interval: Duration::from_secs(60),
                max_discovered_peers: 10,
                discovery_timeout: Duration::from_secs(60),
            },
            protocol: ProtocolConfig {
                enable_gossipsub: false, // Disable for testing
                enable_request_response: true,
                enable_ping: false,            // Disable for testing
                max_message_size: 1024 * 1024, // 1MB for testing
                protocol_timeout: Duration::from_secs(10),
            },
            legacy: LegacyConfig {
                enable_legacy: true,
                legacy_port: 0, // Let OS assign port
                legacy_timeout: Duration::from_secs(5),
                max_legacy_connections: 5,
            },
            features: FeatureFlags {
                nat_traversal: false,          // Disable for testing
                connection_resilience: false,  // Disable for testing
                performance_monitoring: false, // Disable for testing
                detailed_logging: true,        // Enable for testing
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_config_default() {
        let config = NetworkConfig::default();
        assert_eq!(config.mode, NetworkMode::Compatibility);
        assert!(config.legacy_enabled());
        assert!(config.libp2p_enabled());
    }

    #[test]
    fn test_network_config_builder() {
        let config = NetworkConfig::new()
            .with_mode(NetworkMode::LegacyOnly)
            .with_features(FeatureFlags {
                detailed_logging: true,
                ..Default::default()
            });

        assert_eq!(config.mode, NetworkMode::LegacyOnly);
        assert!(config.features.detailed_logging);
    }

    #[test]
    fn test_network_config_validation() {
        let mut config = NetworkConfig::default();

        // Valid configuration should pass
        assert!(config.validate().is_ok());

        // Invalid port should fail
        config.transport.tcp_port = 0;
        assert!(config.validate().is_err());

        // Reset and test invalid peer limit
        config = NetworkConfig::default();
        config.discovery.max_discovered_peers = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_network_config_mode_validation() {
        // LegacyOnly mode with disabled legacy should fail
        let config = NetworkConfig::new()
            .with_mode(NetworkMode::LegacyOnly)
            .with_legacy(LegacyConfig {
                enable_legacy: false,
                ..Default::default()
            });
        assert!(config.validate().is_err());

        // Libp2pOnly mode with disabled features should fail
        let config = NetworkConfig::new()
            .with_mode(NetworkMode::Libp2pOnly)
            .with_transport(TransportConfig {
                enable_noise: false,
                enable_yamux: false,
                ..Default::default()
            });
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_network_config_test_config() {
        let config = NetworkConfig::test_config();
        assert_eq!(config.mode, NetworkMode::Compatibility);
        assert!(!config.transport.enable_noise);
        assert!(!config.discovery.enable_kademlia);
        assert!(config.features.detailed_logging);
    }

    #[test]
    fn test_network_config_primary_port() {
        let config = NetworkConfig::new().with_mode(NetworkMode::LegacyOnly);
        assert_eq!(config.primary_port(), config.legacy.legacy_port);

        let config = NetworkConfig::new().with_mode(NetworkMode::Libp2pOnly);
        assert_eq!(config.primary_port(), config.transport.tcp_port);
    }
}
