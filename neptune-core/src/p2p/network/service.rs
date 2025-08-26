//! Main libp2p network service
//! 
//! This module contains the main NetworkService implementation
//! that orchestrates all libp2p networking components.

use super::{NetworkError, ServiceStatus, ConnectionInfo, ConnectionDirection};
use crate::p2p::config::{NetworkConfig, TransportConfig, DiscoveryConfig, ProtocolConfig};
use crate::p2p::P2pResult;
use libp2p::{
    core::upgrade,
    noise,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    tcp, yamux, PeerId, Transport,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Main network service using libp2p
pub struct NetworkService {
    /// Network configuration
    config: NetworkConfig,
    /// libp2p swarm for network management
    swarm: Option<Swarm<NeptuneBehaviour>>,
    /// Active connections tracking
    connections: Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    /// Service status
    status: Arc<RwLock<ServiceStatus>>,
    /// Service running flag
    running: bool,
}

/// Neptune-specific network behaviour
#[derive(NetworkBehaviour)]
struct NeptuneBehaviour {
    /// Identify protocol for peer information
    identify: libp2p::identify::Behaviour,
    /// Ping protocol for connection health
    ping: libp2p::ping::Behaviour,
    /// Kademlia DHT for peer discovery
    kademlia: libp2p::kad::Behaviour<libp2p::kad::store::MemoryStore>,
    /// mDNS for local network discovery
    mdns: libp2p::mdns::tokio::Behaviour,
    /// Gossipsub for block/transaction broadcasting
    gossipsub: libp2p::gossipsub::Behaviour,
}

impl NetworkService {
    /// Create a new network service
    pub async fn new(config: NetworkConfig) -> P2pResult<Self> {
        // Validate configuration
        config.validate()?;

        let connections = Arc::new(RwLock::new(HashMap::new()));
        let status = Arc::new(RwLock::new(ServiceStatus::Starting));

        Ok(Self {
            config,
            swarm: None,
            connections,
            status,
            running: false,
        })
    }

    /// Start the network service
    pub async fn start(&mut self) -> P2pResult<()> {
        if self.running {
            return Err(NetworkError::ConfigurationError(
                "Network service already running".to_string()
            ).into());
        }

        info!("Starting libp2p network service...");

        // Update status
        {
            let mut status = self.status.write().await;
            *status = ServiceStatus::Starting;
        }

        // Create and configure swarm
        let swarm = self.create_swarm().await?;
        self.swarm = Some(swarm);

        // Start network event loop
        self.start_network_loop().await?;

        self.running = true;
        
        // Update status
        {
            let mut status = self.status.write().await;
            *status = ServiceStatus::Running;
        }

        info!("libp2p network service started successfully");
        Ok(())
    }

    /// Stop the network service
    pub async fn stop(&mut self) -> P2pResult<()> {
        if !self.running {
            return Ok(());
        }

        info!("Stopping libp2p network service...");

        // Update status
        {
            let mut status = self.status.write().await;
            *status = ServiceStatus::Stopping;
        }

        // Clear connections
        {
            let mut connections = self.connections.write().await;
            connections.clear();
        }

        // Drop swarm
        self.swarm = None;
        self.running = false;

        // Update status
        {
            let mut status = self.status.write().await;
            *status = ServiceStatus::Stopped;
        }

        info!("libp2p network service stopped");
        Ok(())
    }

    /// Create and configure the libp2p swarm
    async fn create_swarm(&self) -> P2pResult<Swarm<NeptuneBehaviour>> {
        // Create transport
        let transport = self.create_transport().await?;

        // Create local peer ID
        let local_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        info!("Local peer ID: {}", local_peer_id);

        // Create behaviour
        let behaviour = self.create_behaviour().await?;

        // Create swarm
        let mut swarm = Swarm::new(transport, behaviour, local_peer_id);

        // Configure swarm
        self.configure_swarm(&mut swarm).await?;

        Ok(swarm)
    }

    /// Create the libp2p transport
    async fn create_transport(&self) -> P2pResult<libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>> {
        let tcp_config = tcp::Config::default()
            .nodelay(true)
            .connection_timeout(Duration::from_secs(30));

        let mut transport = tcp::tokio::Transport::new(tcp_config);

        // Add Noise encryption if enabled
        if self.config.transport.enable_noise {
            let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
                .into_authentic(&libp2p::identity::Keypair::generate_ed25519())
                .expect("Signing libp2p-noise static DH keypair failed.");

            transport = transport
                .upgrade(upgrade::Version::V1)
                .authenticate(noise::NoiseAuthenticated::xx(noise_keys).unwrap())
                .multiplex(if self.config.transport.enable_yamux {
                    yamux::YamuxConfig::default()
                } else {
                    yamux::YamuxConfig::default()
                })
                .boxed();
        } else {
            transport = transport
                .upgrade(upgrade::Version::V1)
                .multiplex(yamux::YamuxConfig::default())
                .boxed();
        }

        Ok(transport)
    }

    /// Create the network behaviour
    async fn create_behaviour(&self) -> P2pResult<NeptuneBehaviour> {
        let local_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        // Identify behaviour
        let identify = libp2p::identify::Behaviour::new(
            libp2p::identify::Config::new(
                "/neptune/1.0.0".to_string(),
                local_key.public(),
            )
            .with_agent_version("neptune-core/0.3.0".to_string()),
        );

        // Ping behaviour
        let ping = libp2p::ping::Behaviour::new(
            libp2p::ping::Config::new()
                .with_interval(Duration::from_secs(60))
                .with_timeout(Duration::from_secs(30)),
        );

        // Kademlia DHT behaviour
        let kademlia = if self.config.discovery.enable_kademlia {
            let store = libp2p::kad::store::MemoryStore::new(local_peer_id);
            let mut kad = libp2p::kad::Behaviour::new(local_peer_id, store);
            
            // Add bootstrap nodes
            for bootstrap_node in &self.config.discovery.bootstrap_nodes {
                if let Ok(peer_id) = bootstrap_node.parse::<PeerId>() {
                    let addr = format!("/dnsaddr/bootstrap.libp2p.io/p2p/{}", peer_id);
                    if let Ok(multiaddr) = addr.parse() {
                        kad.add_address(&peer_id, multiaddr);
                    }
                }
            }
            
            kad
        } else {
            let store = libp2p::kad::store::MemoryStore::new(local_peer_id);
            libp2p::kad::Behaviour::new(local_peer_id, store)
        };

        // mDNS behaviour
        let mdns = if self.config.discovery.enable_mdns {
            libp2p::mdns::tokio::Behaviour::new(
                libp2p::mdns::Config::default(),
            )?
        } else {
            libp2p::mdns::tokio::Behaviour::new(
                libp2p::mdns::Config::default(),
            )?
        };

        // Gossipsub behaviour
        let gossipsub = if self.config.protocol.enable_gossipsub {
            let message_id_fn = |message: &libp2p::gossipsub::Message| {
                let mut s = DefaultHasher::new();
                message.data.hash(&mut s);
                libp2p::gossipsub::MessageId::from(s.finish().to_string())
            };

            let gossipsub_config = libp2p::gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(1))
                .validation_mode(libp2p::gossipsub::ValidationMode::Strict)
                .message_id_fn(message_id_fn)
                .build()
                .expect("Valid gossipsub config");

            libp2p::gossipsub::Behaviour::new(
                libp2p::gossipsub::MessageAuthenticity::Signed(local_key),
                gossipsub_config,
            )?
        } else {
            let config = libp2p::gossipsub::ConfigBuilder::default()
                .build()
                .expect("Valid gossipsub config");
            libp2p::gossipsub::Behaviour::new(
                libp2p::gossipsub::MessageAuthenticity::Anonymous,
                config,
            )?
        };

        Ok(NeptuneBehaviour {
            identify,
            ping,
            kademlia,
            mdns,
            gossipsub,
        })
    }

    /// Configure the swarm settings
    async fn configure_swarm(&self, swarm: &mut Swarm<NeptuneBehaviour>) -> P2pResult<()> {
        // Listen on configured address
        let listen_addr = format!("/ip4/0.0.0.0/tcp/{}", self.config.transport.tcp_port);
        let multiaddr = listen_addr.parse()
            .map_err(|e| NetworkError::ConfigurationError(
                format!("Invalid listen address: {}", e)
            ))?;

        swarm.listen_on(multiaddr)
            .map_err(|e| NetworkError::ConfigurationError(
                format!("Failed to listen on address: {}", e)
            ))?;

        info!("Listening on {}", listen_addr);

        // Connect to bootstrap nodes
        for bootstrap_node in &self.config.discovery.bootstrap_nodes {
            if let Ok(peer_id) = bootstrap_node.parse::<PeerId>() {
                let addr = format!("/dnsaddr/bootstrap.libp2p.io/p2p/{}", peer_id);
                if let Ok(multiaddr) = addr.parse() {
                    swarm.dial(multiaddr)
                        .map_err(|e| NetworkError::ConfigurationError(
                            format!("Failed to dial bootstrap node: {}", e)
                        ))?;
                }
            }
        }

        Ok(())
    }

    /// Start the network event loop
    async fn start_network_loop(&mut self) -> P2pResult<()> {
        let swarm = self.swarm.as_mut()
            .ok_or_else(|| NetworkError::ConfigurationError(
                "Swarm not initialized".to_string()
            ))?;

        let connections = self.connections.clone();
        let status = self.status.clone();

        // Spawn network event loop
        tokio::spawn(async move {
            loop {
                match swarm.next_event().await {
                    SwarmEvent::Behaviour(NeptuneBehaviourEvent::Identify(identify_event)) => {
                        Self::handle_identify_event(identify_event, &connections).await;
                    }
                    SwarmEvent::Behaviour(NeptuneBehaviourEvent::Ping(ping_event)) => {
                        Self::handle_ping_event(ping_event, &connections).await;
                    }
                    SwarmEvent::Behaviour(NeptuneBehaviourEvent::Kademlia(kad_event)) => {
                        Self::handle_kademlia_event(kad_event, &connections).await;
                    }
                    SwarmEvent::Behaviour(NeptuneBehaviourEvent::Mdns(mdns_event)) => {
                        Self::handle_mdns_event(mdns_event, &connections).await;
                    }
                    SwarmEvent::Behaviour(NeptuneBehaviourEvent::Gossipsub(gossip_event)) => {
                        Self::handle_gossipsub_event(gossip_event, &connections).await;
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        Self::handle_connection_established(peer_id, endpoint, &connections).await;
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        Self::handle_connection_closed(peer_id, &connections).await;
                    }
                    SwarmEvent::IncomingConnection { .. } => {
                        // Accept all incoming connections for now
                    }
                    SwarmEvent::IncomingConnectionError { .. } => {
                        // Log connection errors
                    }
                    SwarmEvent::OutgoingConnectionError { .. } => {
                        // Log connection errors
                    }
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Listening on new address: {}", address);
                    }
                    SwarmEvent::ExpiredListenAddr { address, .. } => {
                        warn!("Expired listen address: {}", address);
                    }
                    SwarmEvent::ListenerClosed { addresses, .. } => {
                        info!("Listener closed for addresses: {:?}", addresses);
                    }
                    SwarmEvent::ListenerError { error, .. } => {
                        error!("Listener error: {}", error);
                    }
                    _ => {
                        // Handle other events as needed
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle identify events
    async fn handle_identify_event(
        event: libp2p::identify::Event,
        connections: &Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    ) {
        match event {
            libp2p::identify::Event::Received { peer_id, info } => {
                debug!("Received identify info from {}: {:?}", peer_id, info);
            }
            libp2p::identify::Event::Sent { peer_id } => {
                debug!("Sent identify info to {}", peer_id);
            }
            libp2p::identify::Event::Error { peer_id, error } => {
                warn!("Identify error with {}: {}", peer_id, error);
            }
            libp2p::identify::Event::Pushed { peer_id, info } => {
                debug!("Pushed identify info to {}: {:?}", peer_id, info);
            }
        }
    }

    /// Handle ping events
    async fn handle_ping_event(
        event: libp2p::ping::Event,
        connections: &Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    ) {
        match event {
            libp2p::ping::Event::Ping { peer } => {
                debug!("Ping sent to {}", peer);
            }
            libp2p::ping::Event::Pong { peer, rtt } => {
                debug!("Pong received from {} with RTT: {:?}", peer, rtt);
            }
            libp2p::ping::Event::PingFailure { peer, error } => {
                warn!("Ping failed to {}: {}", peer, error);
            }
        }
    }

    /// Handle Kademlia events
    async fn handle_kademlia_event(
        event: libp2p::kad::Event,
        connections: &Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    ) {
        match event {
            libp2p::kad::Event::OutboundQueryCompleted { result, .. } => {
                match result {
                    Ok(peers) => {
                        debug!("Kademlia query completed, found {} peers", peers.len());
                    }
                    Err(e) => {
                        warn!("Kademlia query failed: {}", e);
                    }
                }
            }
            _ => {
                // Handle other Kademlia events as needed
            }
        }
    }

    /// Handle mDNS events
    async fn handle_mdns_event(
        event: libp2p::mdns::Event,
        connections: &Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    ) {
        match event {
            libp2p::mdns::Event::Discovered(list) => {
                for (peer_id, multiaddr) in list {
                    debug!("mDNS discovered peer {} at {}", peer_id, multiaddr);
                }
            }
            libp2p::mdns::Event::Expired(list) => {
                for (peer_id, multiaddr) in list {
                    debug!("mDNS expired peer {} at {}", peer_id, multiaddr);
                }
            }
        }
    }

    /// Handle gossipsub events
    async fn handle_gossipsub_event(
        event: libp2p::gossipsub::Event,
        connections: &Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    ) {
        match event {
            libp2p::gossipsub::Event::Message { 
                propagation_source, 
                message_id, 
                message 
            } => {
                debug!("Gossipsub message from {}: {:?}", propagation_source, message_id);
                // TODO: Handle Neptune-specific messages
            }
            libp2p::gossipsub::Event::Subscribed { peer_id, topic } => {
                debug!("Peer {} subscribed to topic {}", peer_id, topic);
            }
            libp2p::gossipsub::Event::Unsubscribed { peer_id, topic } => {
                debug!("Peer {} unsubscribed from topic {}", peer_id, topic);
            }
            _ => {
                // Handle other gossipsub events as needed
            }
        }
    }

    /// Handle connection established
    async fn handle_connection_established(
        peer_id: PeerId,
        endpoint: libp2p::core::ConnectedPoint,
        connections: &Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    ) {
        let direction = match endpoint {
            libp2p::core::ConnectedPoint::Dialer { .. } => ConnectionDirection::Outbound,
            libp2p::core::ConnectedPoint::Listener { .. } => ConnectionDirection::Inbound,
        };

        let connection_info = ConnectionInfo {
            peer_id: peer_id.to_string(),
            remote_addr: format!("{:?}", endpoint),
            direction,
            established: Instant::now(),
        };

        let mut connections = connections.write().await;
        connections.insert(peer_id, connection_info);

        info!("Connection established with {} ({:?})", peer_id, direction);
    }

    /// Handle connection closed
    async fn handle_connection_closed(
        peer_id: PeerId,
        connections: &Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    ) {
        let mut connections = connections.write().await;
        connections.remove(&peer_id);

        info!("Connection closed with {}", peer_id);
    }

    /// Get service status
    pub async fn status(&self) -> ServiceStatus {
        self.status.read().await.clone()
    }

    /// Get connection count
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Get connection information
    pub async fn get_connections(&self) -> Vec<ConnectionInfo> {
        self.connections.read().await.values().cloned().collect()
    }

    /// Check if service is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get configuration
    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }
}

impl Drop for NetworkService {
    fn drop(&mut self) {
        if self.running {
            // Try to stop gracefully, but don't block
            let _ = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(self.stop())
            });
        }
    }
}

// Helper struct for hashing
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::config::NetworkConfig;

    #[tokio::test]
    async fn test_network_service_creation() {
        let config = NetworkConfig::test_config();
        let service = NetworkService::new(config).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_network_service_status() {
        let config = NetworkConfig::test_config();
        let service = NetworkService::new(config).await.unwrap();
        
        let status = service.status().await;
        assert_eq!(status, ServiceStatus::Starting);
    }

    #[test]
    fn test_connection_direction() {
        assert_eq!(ConnectionDirection::Inbound, ConnectionDirection::Inbound);
        assert_eq!(ConnectionDirection::Outbound, ConnectionDirection::Outbound);
        assert_ne!(ConnectionDirection::Inbound, ConnectionDirection::Outbound);
    }
}
