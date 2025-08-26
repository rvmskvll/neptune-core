//! Peer discovery service for libp2p networking
//! 
//! This module handles peer discovery using Kademlia DHT and mDNS
//! for the libp2p networking layer.

use super::{NetworkError, ServiceStatus};
use crate::p2p::config::DiscoveryConfig;
use crate::p2p::P2pResult;
use libp2p::{
    kad::{Behaviour as KadBehaviour, Event as KadEvent, QueryResult, Record, RecordKey, StoreInserts},
    mdns::{tokio::Behaviour as MdnsBehaviour, Event as MdnsEvent},
    PeerId,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Discovery service for libp2p
pub struct DiscoveryService {
    /// Discovery configuration
    config: DiscoveryConfig,
    /// Service status
    status: ServiceStatus,
    /// Service running flag
    running: bool,
    /// Discovered peers cache
    discovered_peers: HashMap<PeerId, PeerInfo>,
    /// Discovery statistics
    stats: DiscoveryStats,
}

/// Information about a discovered peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Peer ID
    pub peer_id: PeerId,
    /// Multiaddresses
    pub addresses: Vec<String>,
    /// Discovery time
    pub discovered_at: Instant,
    /// Last seen time
    pub last_seen: Instant,
    /// Discovery method
    pub discovery_method: DiscoveryMethod,
}

/// Discovery method used to find the peer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryMethod {
    /// Discovered via Kademlia DHT
    Kademlia,
    /// Discovered via mDNS
    Mdns,
    /// Discovered via bootstrap
    Bootstrap,
    /// Discovered via other means
    Other,
}

/// Discovery service statistics
#[derive(Debug, Clone)]
pub struct DiscoveryStats {
    /// Total peers discovered
    pub total_discovered: usize,
    /// Peers discovered via Kademlia
    pub kademlia_discovered: usize,
    /// Peers discovered via mDNS
    pub mdns_discovered: usize,
    /// Peers discovered via bootstrap
    pub bootstrap_discovered: usize,
    /// Last discovery run time
    pub last_discovery_run: Option<Instant>,
    /// Discovery interval
    pub discovery_interval: Duration,
}

impl DiscoveryService {
    /// Create a new discovery service
    pub async fn new(config: DiscoveryConfig) -> P2pResult<Self> {
        let stats = DiscoveryStats {
            total_discovered: 0,
            kademlia_discovered: 0,
            mdns_discovered: 0,
            bootstrap_discovered: 0,
            last_discovery_run: None,
            discovery_interval: config.discovery_interval,
        };

        Ok(Self {
            config,
            status: ServiceStatus::Stopped,
            running: false,
            discovered_peers: HashMap::new(),
            stats,
        })
    }

    /// Start the discovery service
    pub async fn start(&mut self) -> P2pResult<()> {
        if self.running {
            return Err(NetworkError::DiscoveryError(
                "Discovery service already running".to_string()
            ).into());
        }

        info!("Starting discovery service...");
        self.status = ServiceStatus::Starting;
        self.running = true;
        self.status = ServiceStatus::Running;

        // Start discovery loop
        self.start_discovery_loop().await?;

        info!("Discovery service started successfully");
        Ok(())
    }

    /// Stop the discovery service
    pub async fn stop(&mut self) -> P2pResult<()> {
        if !self.running {
            return Ok(());
        }

        info!("Stopping discovery service...");
        self.status = ServiceStatus::Stopping;
        self.running = false;
        self.status = ServiceStatus::Stopped;

        info!("Discovery service stopped");
        Ok(())
    }

    /// Get discovery status
    pub fn status(&self) -> ServiceStatus {
        self.status.clone()
    }

    /// Start the discovery loop
    async fn start_discovery_loop(&mut self) -> P2pResult<()> {
        let config = self.config.clone();
        let mut interval = tokio::time::interval(config.discovery_interval);

        tokio::spawn(async move {
            loop {
                interval.tick().await;
                
                // Perform discovery operations
                // This would integrate with the actual libp2p swarm
                debug!("Discovery tick - would perform peer discovery");
            }
        });

        Ok(())
    }

    /// Add a discovered peer
    pub fn add_peer(&mut self, peer_id: PeerId, addresses: Vec<String>, method: DiscoveryMethod) {
        let now = Instant::now();
        
        if let Some(existing_peer) = self.discovered_peers.get_mut(&peer_id) {
            // Update existing peer
            existing_peer.addresses = addresses;
            existing_peer.last_seen = now;
            existing_peer.discovery_method = method;
        } else {
            // Add new peer
            let peer_info = PeerInfo {
                peer_id,
                addresses,
                discovered_at: now,
                last_seen: now,
                discovery_method: method,
            };
            
            self.discovered_peers.insert(peer_id, peer_info);
            self.stats.total_discovered += 1;
            
            // Update method-specific stats
            match method {
                DiscoveryMethod::Kademlia => self.stats.kademlia_discovered += 1,
                DiscoveryMethod::Mdns => self.stats.mdns_discovered += 1,
                DiscoveryMethod::Bootstrap => self.stats.bootstrap_discovered += 1,
                DiscoveryMethod::Other => {}
            }
        }
    }

    /// Remove a peer
    pub fn remove_peer(&mut self, peer_id: &PeerId) {
        if let Some(peer_info) = self.discovered_peers.remove(peer_id) {
            // Update method-specific stats
            match peer_info.discovery_method {
                DiscoveryMethod::Kademlia => {
                    if self.stats.kademlia_discovered > 0 {
                        self.stats.kademlia_discovered -= 1;
                    }
                }
                DiscoveryMethod::Mdns => {
                    if self.stats.mdns_discovered > 0 {
                        self.stats.mdns_discovered -= 1;
                    }
                }
                DiscoveryMethod::Bootstrap => {
                    if self.stats.bootstrap_discovered > 0 {
                        self.stats.bootstrap_discovered -= 1;
                    }
                }
                DiscoveryMethod::Other => {}
            }
            
            if self.stats.total_discovered > 0 {
                self.stats.total_discovered -= 1;
            }
        }
    }

    /// Get discovered peers
    pub fn get_peers(&self) -> Vec<PeerInfo> {
        self.discovered_peers.values().cloned().collect()
    }

    /// Get peer count
    pub fn peer_count(&self) -> usize {
        self.discovered_peers.len()
    }

    /// Get peer by ID
    pub fn get_peer(&self, peer_id: &PeerId) -> Option<&PeerInfo> {
        self.discovered_peers.get(peer_id)
    }

    /// Check if peer exists
    pub fn has_peer(&self, peer_id: &PeerId) -> bool {
        self.discovered_peers.contains_key(peer_id)
    }

    /// Get discovery statistics
    pub fn get_stats(&self) -> DiscoveryStats {
        self.stats.clone()
    }

    /// Get configuration
    pub fn config(&self) -> &DiscoveryConfig {
        &self.config
    }

    /// Check if service is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Check if Kademlia is enabled
    pub fn kademlia_enabled(&self) -> bool {
        self.config.enable_kademlia
    }

    /// Check if mDNS is enabled
    pub fn mdns_enabled(&self) -> bool {
        self.config.enable_mdns
    }

    /// Get bootstrap nodes
    pub fn get_bootstrap_nodes(&self) -> &[String] {
        &self.config.bootstrap_nodes
    }

    /// Get discovery interval
    pub fn discovery_interval(&self) -> Duration {
        self.config.discovery_interval
    }

    /// Get maximum discovered peers
    pub fn max_discovered_peers(&self) -> usize {
        self.config.max_discovered_peers
    }

    /// Clean up old peers
    pub fn cleanup_old_peers(&mut self, max_age: Duration) {
        let now = Instant::now();
        let old_peers: Vec<PeerId> = self.discovered_peers
            .iter()
            .filter(|(_, peer_info)| {
                now.duration_since(peer_info.last_seen) > max_age
            })
            .map(|(peer_id, _)| *peer_id)
            .collect();

        for peer_id in old_peers {
            self.remove_peer(&peer_id);
        }

        if !old_peers.is_empty() {
            debug!("Cleaned up {} old peers", old_peers.len());
        }
    }

    /// Validate discovery configuration
    pub fn validate_config(&self) -> P2pResult<()> {
        if self.config.max_discovered_peers == 0 {
            return Err(NetworkError::DiscoveryError(
                "Maximum discovered peers cannot be 0".to_string()
            ).into());
        }

        if self.config.discovery_interval.as_secs() == 0 {
            return Err(NetworkError::DiscoveryError(
                "Discovery interval must be greater than 0".to_string()
            ).into());
        }

        // Validate bootstrap nodes
        for bootstrap_node in &self.config.bootstrap_nodes {
            if bootstrap_node.is_empty() {
                return Err(NetworkError::DiscoveryError(
                    "Bootstrap node cannot be empty".to_string()
                ).into());
            }
        }

        Ok(())
    }

    /// Create a test discovery configuration
    pub fn test_config() -> DiscoveryConfig {
        DiscoveryConfig {
            enable_kademlia: false, // Disable for testing
            enable_mdns: false, // Disable for testing
            bootstrap_nodes: vec![],
            discovery_interval: Duration::from_secs(60),
            max_discovered_peers: 10,
        }
    }
}

impl Drop for DiscoveryService {
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
    use crate::p2p::config::DiscoveryConfig;

    #[tokio::test]
    async fn test_discovery_service_creation() {
        let config = DiscoveryConfig::default();
        let service = DiscoveryService::new(config).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_discovery_service_lifecycle() {
        let config = DiscoveryConfig::default();
        let mut service = DiscoveryService::new(config).await.unwrap();
        
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
    fn test_peer_management() {
        let config = DiscoveryConfig::default();
        let mut service = DiscoveryService {
            config,
            status: ServiceStatus::Stopped,
            running: false,
            discovered_peers: HashMap::new(),
            stats: DiscoveryStats {
                total_discovered: 0,
                kademlia_discovered: 0,
                mdns_discovered: 0,
                bootstrap_discovered: 0,
                last_discovery_run: None,
                discovery_interval: Duration::from_secs(300),
            },
        };
        
        let peer_id = PeerId::random();
        let addresses = vec!["/ip4/127.0.0.1/tcp/8080".to_string()];
        
        // Add peer
        service.add_peer(peer_id, addresses.clone(), DiscoveryMethod::Kademlia);
        assert_eq!(service.peer_count(), 1);
        assert!(service.has_peer(&peer_id));
        
        // Get peer
        let peer_info = service.get_peer(&peer_id).unwrap();
        assert_eq!(peer_info.addresses, addresses);
        assert_eq!(peer_info.discovery_method, DiscoveryMethod::Kademlia);
        
        // Remove peer
        service.remove_peer(&peer_id);
        assert_eq!(service.peer_count(), 0);
        assert!(!service.has_peer(&peer_id));
    }

    #[test]
    fn test_discovery_stats() {
        let config = DiscoveryConfig::default();
        let mut service = DiscoveryService {
            config,
            status: ServiceStatus::Stopped,
            running: false,
            discovered_peers: HashMap::new(),
            stats: DiscoveryStats {
                total_discovered: 0,
                kademlia_discovered: 0,
                mdns_discovered: 0,
                bootstrap_discovered: 0,
                last_discovery_run: None,
                discovery_interval: Duration::from_secs(300),
            },
        };
        
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();
        
        // Add peers via different methods
        service.add_peer(peer1, vec![], DiscoveryMethod::Kademlia);
        service.add_peer(peer2, vec![], DiscoveryMethod::Mdns);
        
        let stats = service.get_stats();
        assert_eq!(stats.total_discovered, 2);
        assert_eq!(stats.kademlia_discovered, 1);
        assert_eq!(stats.mdns_discovered, 1);
        assert_eq!(stats.bootstrap_discovered, 0);
    }

    #[test]
    fn test_config_access() {
        let config = DiscoveryConfig::default();
        let service = DiscoveryService {
            config,
            status: ServiceStatus::Stopped,
            running: false,
            discovered_peers: HashMap::new(),
            stats: DiscoveryStats {
                total_discovered: 0,
                kademlia_discovered: 0,
                mdns_discovered: 0,
                bootstrap_discovered: 0,
                last_discovery_run: None,
                discovery_interval: Duration::from_secs(300),
            },
        };
        
        assert!(service.kademlia_enabled());
        assert!(service.mdns_enabled());
        assert!(!service.get_bootstrap_nodes().is_empty());
        assert_eq!(service.discovery_interval(), Duration::from_secs(300));
        assert_eq!(service.max_discovered_peers(), 100);
    }

    #[test]
    fn test_config_validation() {
        let config = DiscoveryConfig::default();
        let service = DiscoveryService {
            config,
            status: ServiceStatus::Stopped,
            running: false,
            discovered_peers: HashMap::new(),
            stats: DiscoveryStats {
                total_discovered: 0,
                kademlia_discovered: 0,
                mdns_discovered: 0,
                bootstrap_discovered: 0,
                last_discovery_run: None,
                discovery_interval: Duration::from_secs(300),
            },
        };
        
        // Valid configuration should pass
        assert!(service.validate_config().is_ok());
    }

    #[test]
    fn test_test_config() {
        let config = DiscoveryService::test_config();
        assert!(!config.enable_kademlia);
        assert!(!config.enable_mdns);
        assert!(config.bootstrap_nodes.is_empty());
        assert_eq!(config.discovery_interval, Duration::from_secs(60));
        assert_eq!(config.max_discovered_peers, 10);
    }

    #[test]
    fn test_discovery_method() {
        assert_eq!(DiscoveryMethod::Kademlia, DiscoveryMethod::Kademlia);
        assert_eq!(DiscoveryMethod::Mdns, DiscoveryMethod::Mdns);
        assert_eq!(DiscoveryMethod::Bootstrap, DiscoveryMethod::Bootstrap);
        assert_eq!(DiscoveryMethod::Other, DiscoveryMethod::Other);
        assert_ne!(DiscoveryMethod::Kademlia, DiscoveryMethod::Mdns);
    }
}
