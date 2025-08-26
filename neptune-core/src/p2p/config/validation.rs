//! Configuration validation for network settings

use crate::p2p::P2pResult;

/// Configuration validation errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid port number: {0}")]
    InvalidPort,
    
    #[error("Invalid peer limit: {0}")]
    InvalidPeerLimit,
    
    #[error("Invalid message size: {0}")]
    InvalidMessageSize,
    
    #[error("Mode mismatch: {0}")]
    ModeMismatch(String),
    
    #[error("Invalid bootstrap node: {0}")]
    InvalidBootstrapNode(String),
    
    #[error("Invalid timeout value: {0}")]
    InvalidTimeout(String),
    
    #[error("Configuration conflict: {0}")]
    ConfigurationConflict(String),
}

/// Configuration validator trait
pub trait ConfigValidator {
    /// Validate the configuration
    fn validate(&self) -> P2pResult<()>;
}

/// Validation utilities for configuration
pub struct ValidationUtils;

impl ValidationUtils {
    /// Validate port number
    pub fn validate_port(port: u16) -> P2pResult<()> {
        if port == 0 {
            return Err(crate::p2p::P2pError::Config(ConfigError::InvalidPort));
        }
        Ok(())
    }

    /// Validate peer limit
    pub fn validate_peer_limit(limit: usize) -> P2pResult<()> {
        if limit == 0 {
            return Err(crate::p2p::P2pError::Config(ConfigError::InvalidPeerLimit));
        }
        Ok(())
    }

    /// Validate message size
    pub fn validate_message_size(size: usize) -> P2pResult<()> {
        if size == 0 {
            return Err(crate::p2p::P2pError::Config(ConfigError::InvalidMessageSize));
        }
        
        // Check reasonable upper bound (1GB)
        if size > 1024 * 1024 * 1024 {
            return Err(crate::p2p::P2pError::Config(
                ConfigError::InvalidMessageSize
            ));
        }
        
        Ok(())
    }

    /// Validate timeout duration
    pub fn validate_timeout(timeout: std::time::Duration) -> P2pResult<()> {
        if timeout.as_secs() == 0 {
            return Err(crate::p2p::P2pError::Config(
                ConfigError::InvalidTimeout("Timeout must be greater than 0 seconds".to_string())
            ));
        }
        
        // Check reasonable upper bound (1 hour)
        if timeout.as_secs() > 3600 {
            return Err(crate::p2p::P2pError::Config(
                ConfigError::InvalidTimeout("Timeout must be less than 1 hour".to_string())
            ));
        }
        
        Ok(())
    }

    /// Validate bootstrap node format
    pub fn validate_bootstrap_node(node: &str) -> P2pResult<()> {
        if node.is_empty() {
            return Err(crate::p2p::P2pError::Config(
                ConfigError::InvalidBootstrapNode("Bootstrap node cannot be empty".to_string())
            ));
        }
        
        // Basic validation for libp2p peer ID format
        if !node.starts_with("Qm") && !node.starts_with("12D3KooW") {
            return Err(crate::p2p::P2pError::Config(
                ConfigError::InvalidBootstrapNode(
                    format!("Invalid bootstrap node format: {}", node)
                )
            ));
        }
        
        Ok(())
    }

    /// Validate IP address
    pub fn validate_ip_address(ip: &std::net::IpAddr) -> P2pResult<()> {
        match ip {
            std::net::IpAddr::V4(addr) => {
                // Check for private/local addresses
                if addr.is_private() || addr.is_loopback() || addr.is_link_local() {
                    return Err(crate::p2p::P2pError::Config(
                        ConfigError::ConfigurationConflict(
                            "Private/local IP addresses are not allowed for bootstrap nodes".to_string()
                        )
                    ));
                }
            }
            std::net::IpAddr::V6(addr) => {
                // Check for local addresses
                if addr.is_loopback() || addr.is_unspecified() {
                    return Err(crate::p2p::P2pError::Config(
                        ConfigError::ConfigurationConflict(
                            "Local IPv6 addresses are not allowed for bootstrap nodes".to_string()
                        )
                    ));
                }
            }
        }
        
        Ok(())
    }

    /// Validate socket address
    pub fn validate_socket_address(addr: &std::net::SocketAddr) -> P2pResult<()> {
        Self::validate_ip_address(&addr.ip())?;
        Self::validate_port(addr.port())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_validate_port() {
        assert!(ValidationUtils::validate_port(8080).is_ok());
        assert!(ValidationUtils::validate_port(0).is_err());
    }

    #[test]
    fn test_validate_peer_limit() {
        assert!(ValidationUtils::validate_peer_limit(10).is_ok());
        assert!(ValidationUtils::validate_peer_limit(0).is_err());
    }

    #[test]
    fn test_validate_message_size() {
        assert!(ValidationUtils::validate_message_size(1024).is_ok());
        assert!(ValidationUtils::validate_message_size(0).is_err());
        
        // Test upper bound
        let max_size = 1024 * 1024 * 1024; // 1GB
        assert!(ValidationUtils::validate_message_size(max_size).is_ok());
        
        let too_large = max_size + 1;
        assert!(ValidationUtils::validate_message_size(too_large).is_err());
    }

    #[test]
    fn test_validate_timeout() {
        assert!(ValidationUtils::validate_timeout(Duration::from_secs(30)).is_ok());
        assert!(ValidationUtils::validate_timeout(Duration::from_secs(0)).is_err());
        assert!(ValidationUtils::validate_timeout(Duration::from_secs(3601)).is_err());
    }

    #[test]
    fn test_validate_bootstrap_node() {
        assert!(ValidationUtils::validate_bootstrap_node("QmNnooDu7bfjPFoTaLxpMpVq4uEbgvnq6ckv2ytLc4fqxG").is_ok());
        assert!(ValidationUtils::validate_bootstrap_node("12D3KooWABC123").is_ok());
        assert!(ValidationUtils::validate_bootstrap_node("").is_err());
        assert!(ValidationUtils::validate_bootstrap_node("invalid").is_err());
    }

    #[test]
    fn test_validate_ip_address() {
        // Valid public IPs
        assert!(ValidationUtils::validate_ip_address(&"8.8.8.8".parse().unwrap()).is_ok());
        assert!(ValidationUtils::validate_ip_address(&"2001:4860:4860::8888".parse().unwrap()).is_ok());
        
        // Invalid private/local IPs
        assert!(ValidationUtils::validate_ip_address(&"127.0.0.1".parse().unwrap()).is_err());
        assert!(ValidationUtils::validate_ip_address(&"192.168.1.1".parse().unwrap()).is_err());
        assert!(ValidationUtils::validate_ip_address(&"::1".parse().unwrap()).is_err());
    }

    #[test]
    fn test_validate_socket_address() {
        assert!(ValidationUtils::validate_socket_address(&"8.8.8.8:8080".parse().unwrap()).is_ok());
        assert!(ValidationUtils::validate_socket_address(&"127.0.0.1:8080".parse().unwrap()).is_err());
        assert!(ValidationUtils::validate_socket_address(&"8.8.8.8:0".parse().unwrap()).is_err());
    }
}
