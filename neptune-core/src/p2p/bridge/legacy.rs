//! Legacy TCP networking service for compatibility
//! 
//! This module provides a wrapper around the existing legacy TCP-based
//! networking implementation to maintain compatibility during migration.

use super::{BridgeError, BridgeStatus, MigrationPhase};
use crate::p2p::config::LegacyConfig;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// Legacy network service that wraps existing TCP networking
pub struct LegacyNetworkService {
    /// Legacy configuration
    config: LegacyConfig,
    /// Bridge status reference
    status: Arc<RwLock<BridgeStatus>>,
    /// TCP listener for incoming connections
    listener: Option<TcpListener>,
    /// Active connection handles
    connections: Vec<JoinHandle<()>>,
    /// Service running flag
    running: bool,
}

impl LegacyNetworkService {
    /// Create a new legacy network service
    pub async fn new(
        config: LegacyConfig,
        status: Arc<RwLock<BridgeStatus>>,
    ) -> Result<Self, BridgeError> {
        let listener = if config.enable_legacy {
            let addr = format!("0.0.0.0:{}", config.legacy_port);
            let listener = TcpListener::bind(&addr).await
                .map_err(|e| BridgeError::ConnectionError(
                    format!("Failed to bind legacy listener: {}", e)
                ))?;
            Some(listener)
        } else {
            None
        };

        Ok(Self {
            config,
            status,
            listener,
            connections: Vec::new(),
            running: false,
        })
    }

    /// Start the legacy network service
    pub async fn start(&mut self) -> Result<(), BridgeError> {
        if !self.config.enable_legacy {
            return Ok(());
        }

        if self.running {
            return Err(BridgeError::ConnectionError(
                "Legacy service already running".to_string()
            ));
        }

        self.running = true;

        // Start connection acceptor if listener is available
        if let Some(listener) = self.listener.take() {
            let config = self.config.clone();
            let status = self.status.clone();
            
            let acceptor_handle = tokio::spawn(async move {
                Self::accept_connections(listener, config, status).await;
            });
            
            self.connections.push(acceptor_handle);
        }

        Ok(())
    }

    /// Stop the legacy network service
    pub async fn stop(&mut self) -> Result<(), BridgeError> {
        if !self.running {
            return Ok(());
        }

        self.running = false;

        // Cancel all connection handles
        for handle in self.connections.drain(..) {
            handle.abort();
        }

        // Update status
        let mut status = self.status.write().await;
        status.legacy_connections = 0;

        Ok(())
    }

    /// Accept incoming legacy connections
    async fn accept_connections(
        mut listener: TcpListener,
        config: LegacyConfig,
        status: Arc<RwLock<BridgeStatus>>,
    ) {
        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    // Check connection limit
                    let current_connections = {
                        let status_guard = status.read().await;
                        status_guard.legacy_connections
                    };

                    if current_connections >= config.max_legacy_connections {
                        // Reject connection if limit reached
                        let _ = socket.shutdown().await;
                        continue;
                    }

                    // Spawn connection handler
                    let config_clone = config.clone();
                    let status_clone = status.clone();
                    
                    let connection_handle = tokio::spawn(async move {
                        Self::handle_legacy_connection(socket, addr, config_clone, status_clone).await;
                    });

                    // Update connection count
                    {
                        let mut status_guard = status.write().await;
                        status_guard.legacy_connections += 1;
                    }

                    // Store handle (this would need to be managed properly in a real implementation)
                    drop(connection_handle);
                }
                Err(e) => {
                    // Log error but continue accepting
                    tracing::warn!("Error accepting legacy connection: {}", e);
                }
            }
        }
    }

    /// Handle individual legacy connection
    async fn handle_legacy_connection(
        socket: tokio::net::TcpStream,
        addr: SocketAddr,
        config: LegacyConfig,
        status: Arc<RwLock<BridgeStatus>>,
    ) {
        // TODO: Implement legacy connection handling
        // This will integrate with existing peer handling logic
        
        // For now, just log the connection
        tracing::info!("Legacy connection from {}", addr);
        
        // Simulate connection processing
        tokio::time::sleep(config.legacy_timeout).await;
        
        // Update connection count when connection closes
        let mut status_guard = status.write().await;
        if status_guard.legacy_connections > 0 {
            status_guard.legacy_connections -= 1;
        }
        
        tracing::info!("Legacy connection from {} closed", addr);
    }

    /// Get current connection count
    pub async fn connection_count(&self) -> usize {
        let status = self.status.read().await;
        status.legacy_connections
    }

    /// Check if service is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get configuration
    pub fn config(&self) -> &LegacyConfig {
        &self.config
    }
}

impl Drop for LegacyNetworkService {
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
    use crate::p2p::config::LegacyConfig;

    #[tokio::test]
    async fn test_legacy_service_creation() {
        let config = LegacyConfig::default();
        let status = Arc::new(RwLock::new(BridgeStatus {
            migration_phase: MigrationPhase::DualProtocol,
            legacy_connections: 0,
            libp2p_connections: 0,
            migration_progress: 0.0,
        }));
        
        let service = LegacyNetworkService::new(config, status).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_legacy_service_lifecycle() {
        let mut config = LegacyConfig::default();
        config.legacy_port = 0; // Let OS assign port for testing
        
        let status = Arc::new(RwLock::new(BridgeStatus {
            migration_phase: MigrationPhase::DualProtocol,
            legacy_connections: 0,
            libp2p_connections: 0,
            migration_progress: 0.0,
        }));
        
        let mut service = LegacyNetworkService::new(config, status).await.unwrap();
        
        // Service should not be running initially
        assert!(!service.is_running());
        
        // Start service
        assert!(service.start().await.is_ok());
        assert!(service.is_running());
        
        // Stop service
        assert!(service.stop().await.is_ok());
        assert!(!service.is_running());
    }

    #[test]
    fn test_legacy_config_access() {
        let config = LegacyConfig::default();
        let status = Arc::new(RwLock::new(BridgeStatus {
            migration_phase: MigrationPhase::DualProtocol,
            legacy_connections: 0,
            libp2p_connections: 0,
            migration_progress: 0.0,
        }));
        
        // This would need to be async in real usage
        // For now, just test the config access
        assert_eq!(config.legacy_port, 9798);
        assert!(config.enable_legacy);
    }
}
