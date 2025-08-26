//! Transport service for libp2p networking
//! 
//! This module handles transport configuration and management
//! for the libp2p networking layer.

use super::{NetworkError, ServiceStatus};
use crate::p2p::config::TransportConfig;
use crate::p2p::P2pResult;
use libp2p::{
    core::upgrade,
    noise,
    tcp, yamux, PeerId, Transport,
};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Transport service for libp2p
pub struct TransportService {
    /// Transport configuration
    config: TransportConfig,
    /// Service status
    status: ServiceStatus,
    /// Service running flag
    running: bool,
}

impl TransportService {
    /// Create a new transport service
    pub async fn new(config: TransportConfig) -> P2pResult<Self> {
        Ok(Self {
            config,
            status: ServiceStatus::Stopped,
            running: false,
        })
    }

    /// Start the transport service
    pub async fn start(&mut self) -> P2pResult<()> {
        if self.running {
            return Err(NetworkError::TransportError(
                "Transport service already running".to_string()
            ).into());
        }

        info!("Starting transport service on port {}", self.config.tcp_port);
        self.status = ServiceStatus::Starting;
        self.running = true;
        self.status = ServiceStatus::Running;

        info!("Transport service started successfully");
        Ok(())
    }

    /// Stop the transport service
    pub async fn stop(&mut self) -> P2pResult<()> {
        if !self.running {
            return Ok(());
        }

        info!("Stopping transport service...");
        self.status = ServiceStatus::Stopping;
        self.running = false;
        self.status = ServiceStatus::Stopped;

        info!("Transport service stopped");
        Ok(())
    }

    /// Get transport status
    pub fn status(&self) -> ServiceStatus {
        self.status.clone()
    }

    /// Create a libp2p transport with current configuration
    pub fn create_transport(&self) -> P2pResult<libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>> {
        // Create TCP transport with configuration
        let tcp_config = tcp::Config::default()
            .nodelay(true)
            .connection_timeout(self.config.connection_timeout)
            .keepalive(Some(self.config.keep_alive_interval));

        let mut transport = tcp::tokio::Transport::new(tcp_config);

        // Add Noise encryption if enabled
        if self.config.enable_noise {
            debug!("Adding Noise encryption to transport");
            let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
                .into_authentic(&libp2p::identity::Keypair::generate_ed25519())
                .map_err(|e| NetworkError::TransportError(
                    format!("Failed to create Noise keys: {}", e)
                ))?;

            transport = transport
                .upgrade(upgrade::Version::V1)
                .authenticate(noise::NoiseAuthenticated::xx(noise_keys).unwrap())
                .multiplex(self.create_yamux_config())
                .boxed();
        } else {
            debug!("Transport without encryption");
            transport = transport
                .upgrade(upgrade::Version::V1)
                .multiplex(self.create_yamux_config())
                .boxed();
        }

        Ok(transport)
    }

    /// Create Yamux configuration
    fn create_yamux_config(&self) -> yamux::YamuxConfig {
        let mut config = yamux::YamuxConfig::default();
        
        if self.config.enable_yamux {
            config = config
                .with_max_num_streams(1024)
                .with_receive_window_size(1024 * 1024) // 1MB
                .with_max_message_size(1024 * 1024 * 100); // 100MB
        }

        config
    }

    /// Get transport configuration
    pub fn config(&self) -> &TransportConfig {
        &self.config
    }

    /// Check if service is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get TCP port
    pub fn tcp_port(&self) -> u16 {
        self.config.tcp_port
    }

    /// Check if Noise encryption is enabled
    pub fn noise_enabled(&self) -> bool {
        self.config.enable_noise
    }

    /// Check if Yamux multiplexing is enabled
    pub fn yamux_enabled(&self) -> bool {
        self.config.enable_yamux
    }

    /// Get connection timeout
    pub fn connection_timeout(&self) -> Duration {
        self.config.connection_timeout
    }

    /// Get keep-alive interval
    pub fn keep_alive_interval(&self) -> Duration {
        self.config.keep_alive_interval
    }

    /// Validate transport configuration
    pub fn validate_config(&self) -> P2pResult<()> {
        if self.config.tcp_port == 0 {
            return Err(NetworkError::TransportError(
                "TCP port cannot be 0".to_string()
            ).into());
        }

        if self.config.connection_timeout.as_secs() == 0 {
            return Err(NetworkError::TransportError(
                "Connection timeout must be greater than 0".to_string()
            ).into());
        }

        if self.config.keep_alive_interval.as_secs() == 0 {
            return Err(NetworkError::TransportError(
                "Keep-alive interval must be greater than 0".to_string()
            ).into());
        }

        if self.config.keep_alive_interval < self.config.connection_timeout {
            return Err(NetworkError::TransportError(
                "Keep-alive interval must be greater than connection timeout".to_string()
            ).into());
        }

        Ok(())
    }

    /// Create a test transport configuration
    pub fn test_config() -> TransportConfig {
        TransportConfig {
            tcp_port: 0, // Let OS assign port
            enable_noise: false, // Disable for testing
            enable_yamux: false, // Disable for testing
            connection_timeout: Duration::from_secs(5),
            keep_alive_interval: Duration::from_secs(10),
        }
    }
}

impl Drop for TransportService {
    fn drop(&mut self) {
        if self.running {
            // Try to stop gracefully, but don't block
            let _ = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(self.stop())
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::config::TransportConfig;

    #[tokio::test]
    async fn test_transport_service_creation() {
        let config = TransportConfig::default();
        let service = TransportService::new(config).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_transport_service_lifecycle() {
        let config = TransportConfig::default();
        let mut service = TransportService::new(config).await.unwrap();
        
        // Service should not be running initially
        assert!(!service.is_running());
        assert_eq!(service.status(), ServiceStatus::Stopped);
        
        // Start service
        assert!(service.start().await.is_ok());
        assert!(service.is_running());
        assert_eq!(service.status(), ServiceStatus::Running);
        
        // Stop service
        assert!(service.stop().await.is_ok());
        assert!(!service.is_running());
        assert_eq!(service.status(), ServiceStatus::Stopped);
    }

    #[test]
    fn test_transport_config_access() {
        let config = TransportConfig::default();
        let service = TransportService {
            config,
            status: ServiceStatus::Stopped,
            running: false,
        };
        
        assert_eq!(service.tcp_port(), 9798);
        assert!(service.noise_enabled());
        assert!(service.yamux_enabled());
        assert_eq!(service.connection_timeout(), Duration::from_secs(30));
        assert_eq!(service.keep_alive_interval(), Duration::from_secs(60));
    }

    #[test]
    fn test_transport_config_validation() {
        let config = TransportConfig::default();
        let service = TransportService {
            config,
            status: ServiceStatus::Stopped,
            running: false,
        };
        
        // Valid configuration should pass
        assert!(service.validate_config().is_ok());
    }

    #[test]
    fn test_transport_test_config() {
        let config = TransportService::test_config();
        assert_eq!(config.tcp_port, 0);
        assert!(!config.enable_noise);
        assert!(!config.enable_yamux);
        assert_eq!(config.connection_timeout, Duration::from_secs(5));
        assert_eq!(config.keep_alive_interval, Duration::from_secs(10));
    }

    #[test]
    fn test_yamux_config_creation() {
        let config = TransportConfig::default();
        let service = TransportService {
            config,
            status: ServiceStatus::Stopped,
            running: false,
        };
        
        let yamux_config = service.create_yamux_config();
        // Yamux config should be created successfully
        assert!(yamux_config.max_num_streams() > 0);
    }
}
