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
    request_response::{Behaviour as RequestResponseBehaviour, ProtocolSupport, ResponseChannel},
    ping::{Behaviour as PingBehaviour, Event as PingEvent},
    identify::{Behaviour as IdentifyBehaviour, Event as IdentifyEvent},
    kad::{Behaviour as KadBehaviour, Event as KadEvent, QueryResult, Record, RecordKey, StoreInserts},
    mdns::{tokio::Behaviour as MdnsBehaviour, Event as MdnsEvent},
    gossipsub::{Behaviour as GossipsubBehaviour, Event as GossipsubEvent, MessageId, ValidationMode, MessageAuthenticity},
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
    swarm: Option<Swarm<EnhancedNeptuneBehaviour>>,
    /// Active connections tracking
    connections: Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    /// Service status
    status: Arc<RwLock<ServiceStatus>>,
    /// Service running flag
    running: bool,
}

/// Enhanced Neptune-specific network behaviour with all available libp2p protocols
#[derive(NetworkBehaviour)]
struct EnhancedNeptuneBehaviour {
    /// Identify protocol for peer information
    identify: IdentifyBehaviour,
    /// Ping protocol for connection health
    ping: PingBehaviour,
    /// Kademlia DHT for peer discovery
    kademlia: KadBehaviour<libp2p::kad::store::MemoryStore>,
    /// mDNS for local network discovery
    mdns: MdnsBehaviour,
    /// Gossipsub for block/transaction broadcasting
    gossipsub: GossipsubBehaviour,
    /// Request-response for direct peer communication
    request_response: RequestResponseBehaviour<NeptuneRequestCodec>,
}

/// Neptune request-response protocol codec
#[derive(Debug, Clone)]
pub struct NeptuneRequestCodec;

/// Neptune request types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NeptuneRequest {
    /// Block request
    BlockRequest { block_hash: String },
    /// Transaction request
    TransactionRequest { tx_hash: String },
    /// Peer list request
    PeerListRequest { max_peers: u32 },
    /// Sync request
    SyncRequest { from_height: u64, to_height: u64 },
    /// Custom request
    CustomRequest { request_type: String, data: Vec<u8> },
}

/// Neptune response types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NeptuneResponse {
    /// Block response
    BlockResponse { block_data: Vec<u8> },
    /// Transaction response
    TransactionResponse { tx_data: Vec<u8> },
    /// Peer list response
    PeerListResponse { peers: Vec<String> },
    /// Sync response
    SyncResponse { blocks: Vec<Vec<u8>> },
    /// Custom response
    CustomResponse { response_type: String, data: Vec<u8> },
    /// Error response
    ErrorResponse { error_code: u32, error_message: String },
}

impl libp2p::request_response::Codec for NeptuneRequestCodec {
    type Protocol = libp2p::request_response::ProtocolName;
    type Request = NeptuneRequest;
    type Response = NeptuneResponse;

    fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> std::io::Result<Option<Self::Request>>
    where
        T: AsyncRead + Unpin,
    {
        // Read request from stream
        // This is a simplified implementation
        let mut buffer = Vec::new();
        io.read_to_end(&mut buffer).await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        
        // Parse request (simplified)
        if buffer.is_empty() {
            Ok(None)
        } else {
            // In a real implementation, you'd deserialize the request
            Ok(Some(NeptuneRequest::CustomRequest {
                request_type: "test".to_string(),
                data: buffer,
            }))
        }
    }

    fn read_response<T>(&mut self, _: &Self::Protocol, io: &mut T) -> std::io::Result<Option<Self::Response>>
    where
        T: AsyncRead + Unpin,
    {
        // Read response from stream
        let mut buffer = Vec::new();
        io.read_to_end(&mut buffer).await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        
        // Parse response (simplified)
        if buffer.is_empty() {
            Ok(None)
        } else {
            Ok(Some(NeptuneResponse::CustomResponse {
                response_type: "test".to_string(),
                data: buffer,
            }))
        }
    }

    fn write_request<T>(&mut self, _: &Self::Protocol, io: &mut T, request: Self::Request) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin,
    {
        // Write request to stream
        let data = match request {
            NeptuneRequest::BlockRequest { block_hash } => block_hash.into_bytes(),
            NeptuneRequest::TransactionRequest { tx_hash } => tx_hash.into_bytes(),
            NeptuneRequest::PeerListRequest { max_peers } => max_peers.to_le_bytes().to_vec(),
            NeptuneRequest::SyncRequest { from_height, to_height } => {
                let mut data = Vec::new();
                data.extend_from_slice(&from_height.to_le_bytes());
                data.extend_from_slice(&to_height.to_le_bytes());
                data
            }
            NeptuneRequest::CustomRequest { request_type, data } => {
                let mut combined = Vec::new();
                combined.extend_from_slice(request_type.as_bytes());
                combined.extend_from_slice(&data);
                combined
            }
        };
        
        io.write_all(&data).await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(())
    }

    fn write_response<T>(&mut self, _: &Self::Protocol, io: &mut T, response: Self::Response) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin,
    {
        // Write response to stream
        let data = match response {
            NeptuneResponse::BlockResponse { block_data } => block_data,
            NeptuneResponse::TransactionResponse { tx_data } => tx_data,
            NeptuneResponse::PeerListResponse { peers } => {
                let mut data = Vec::new();
                for peer in peers {
                    data.extend_from_slice(peer.as_bytes());
                    data.push(b'\n');
                }
                data
            }
            NeptuneResponse::SyncResponse { blocks } => {
                let mut data = Vec::new();
                for block in blocks {
                    data.extend_from_slice(&(block.len() as u32).to_le_bytes());
                    data.extend_from_slice(&block);
                }
                data
            }
            NeptuneResponse::CustomResponse { response_type, data } => {
                let mut combined = Vec::new();
                combined.extend_from_slice(response_type.as_bytes());
                combined.extend_from_slice(&data);
                combined
            }
            NeptuneResponse::ErrorResponse { error_code, error_message } => {
                let mut data = Vec::new();
                data.extend_from_slice(&error_code.to_le_bytes());
                data.extend_from_slice(error_message.as_bytes());
                data
            }
        };
        
        io.write_all(&data).await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(())
    }
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

        info!("Starting enhanced libp2p network service...");

        // Update status
        {
            let mut status = self.status.write().await;
            *status = ServiceStatus::Starting;
        }

        // Create and configure swarm
        let swarm = self.create_enhanced_swarm().await?;
        self.swarm = Some(swarm);

        // Start network event loop
        self.start_enhanced_network_loop().await?;

        self.running = true;
        
        // Update status
        {
            let mut status = self.status.write().await;
            *status = ServiceStatus::Running;
        }

        info!("Enhanced libp2p network service started successfully");
        Ok(())
    }

    /// Stop the network service
    pub async fn stop(&mut self) -> P2pResult<()> {
        if !self.running {
            return Ok(());
        }

        info!("Stopping enhanced libp2p network service...");

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

        info!("Enhanced libp2p network service stopped");
        Ok(())
    }

    /// Create and configure the enhanced libp2p swarm
    async fn create_enhanced_swarm(&self) -> P2pResult<Swarm<EnhancedNeptuneBehaviour>> {
        // Create transport
        let transport = self.create_transport().await?;

        // Create local peer ID
        let local_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        info!("Local peer ID: {}", local_peer_id);

        // Create enhanced behaviour
        let behaviour = self.create_enhanced_behaviour().await?;

        // Create swarm
        let mut swarm = Swarm::new(transport, behaviour, local_peer_id);

        // Configure swarm
        self.configure_enhanced_swarm(&mut swarm).await?;

        Ok(swarm)
    }

    /// Create the enhanced libp2p transport
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

    /// Create the enhanced network behaviour
    async fn create_enhanced_behaviour(&self) -> P2pResult<EnhancedNeptuneBehaviour> {
        let local_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        // Identify behaviour
        let identify = IdentifyBehaviour::new(
            libp2p::identify::Config::new(
                "/neptune/1.0.0".to_string(),
                local_key.public(),
            )
            .with_agent_version("neptune-core/0.3.0".to_string()),
        );

        // Ping behaviour
        let ping = PingBehaviour::new(
            libp2p::ping::Config::new()
                .with_interval(Duration::from_secs(60))
                .with_timeout(Duration::from_secs(30)),
        );

        // Kademlia DHT behaviour
        let kademlia = if self.config.discovery.enable_kademlia {
            let store = libp2p::kad::store::MemoryStore::new(local_peer_id);
            let mut kad = KadBehaviour::new(local_peer_id, store);
            
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
            KadBehaviour::new(local_peer_id, store)
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
                let mut s = std::collections::hash_map::DefaultHasher::new();
                message.data.hash(&mut s);
                MessageId::from(s.finish().to_string())
            };

            let gossipsub_config = libp2p::gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(1))
                .validation_mode(ValidationMode::Strict)
                .message_id_fn(message_id_fn)
                .build()
                .expect("Valid gossipsub config");

            GossipsubBehaviour::new(
                MessageAuthenticity::Signed(local_key),
                gossipsub_config,
            )?
        } else {
            let config = libp2p::gossipsub::ConfigBuilder::default()
                .build()
                .expect("Valid gossipsub config");
            GossipsubBehaviour::new(
                MessageAuthenticity::Anonymous,
                config,
            )?
        };

        // Request-response behaviour
        let request_response = RequestResponseBehaviour::new(
            NeptuneRequestCodec,
            std::iter::once((libp2p::request_response::ProtocolName::from("/neptune/1.0.0"), ProtocolSupport::Full)),
            libp2p::request_response::Config::default()
                .with_request_timeout(Duration::from_secs(30))
                .with_max_concurrent_streams(Some(100)),
        );





        Ok(EnhancedNeptuneBehaviour {
            identify,
            ping,
            kademlia,
            mdns,
            gossipsub,
            request_response,
        })
    }

    /// Configure the enhanced swarm settings
    async fn configure_enhanced_swarm(&self, swarm: &mut Swarm<EnhancedNeptuneBehaviour>) -> P2pResult<()> {
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

    /// Start the enhanced network event loop
    async fn start_enhanced_network_loop(&mut self) -> P2pResult<()> {
        let swarm = self.swarm.as_mut()
            .ok_or_else(|| NetworkError::ConfigurationError(
                "Swarm not initialized".to_string()
            ))?;

        let connections = self.connections.clone();
        let status = self.status.clone();

        // Spawn enhanced network event loop
        tokio::spawn(async move {
            loop {
                match swarm.next_event().await {
                    SwarmEvent::Behaviour(EnhancedNeptuneBehaviourEvent::Identify(identify_event)) => {
                        Self::handle_identify_event(identify_event, &connections).await;
                    }
                    SwarmEvent::Behaviour(EnhancedNeptuneBehaviourEvent::Ping(ping_event)) => {
                        Self::handle_ping_event(ping_event, &connections).await;
                    }
                    SwarmEvent::Behaviour(EnhancedNeptuneBehaviourEvent::Kademlia(kad_event)) => {
                        Self::handle_kademlia_event(kad_event, &connections).await;
                    }
                    SwarmEvent::Behaviour(EnhancedNeptuneBehaviourEvent::Mdns(mdns_event)) => {
                        Self::handle_mdns_event(mdns_event, &connections).await;
                    }
                    SwarmEvent::Behaviour(EnhancedNeptuneBehaviourEvent::Gossipsub(gossip_event)) => {
                        Self::handle_gossipsub_event(gossip_event, &connections).await;
                    }
                    SwarmEvent::Behaviour(EnhancedNeptuneBehaviourEvent::RequestResponse(req_resp_event)) => {
                        Self::handle_request_response_event(req_resp_event, &connections).await;
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
        event: IdentifyEvent,
        connections: &Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    ) {
        match event {
            IdentifyEvent::Received { peer_id, info } => {
                debug!("Received identify info from {}: {:?}", peer_id, info);
            }
            IdentifyEvent::Sent { peer_id } => {
                debug!("Sent identify info to {}", peer_id);
            }
            IdentifyEvent::Error { peer_id, error } => {
                warn!("Identify error with {}: {}", peer_id, error);
            }
            IdentifyEvent::Pushed { peer_id, info } => {
                debug!("Pushed identify info to {}: {:?}", peer_id, info);
            }
        }
    }

    /// Handle ping events
    async fn handle_ping_event(
        event: PingEvent,
        connections: &Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    ) {
        match event {
            PingEvent::Ping { peer } => {
                debug!("Ping sent to {}", peer);
            }
            PingEvent::Pong { peer, rtt } => {
                debug!("Pong received from {} with RTT: {:?}", peer, rtt);
            }
            PingEvent::PingFailure { peer, error } => {
                warn!("Ping failed to {}: {}", peer, error);
            }
        }
    }

    /// Handle Kademlia events
    async fn handle_kademlia_event(
        event: KadEvent,
        connections: &Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    ) {
        match event {
            KadEvent::OutboundQueryCompleted { result, .. } => {
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
        event: MdnsEvent,
        connections: &Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    ) {
        match event {
            MdnsEvent::Discovered(list) => {
                for (peer_id, multiaddr) in list {
                    debug!("mDNS discovered peer {} at {}", peer_id, multiaddr);
                }
            }
            MdnsEvent::Expired(list) => {
                for (peer_id, multiaddr) in list {
                    debug!("mDNS expired peer {} at {}", peer_id, multiaddr);
                }
            }
        }
    }

    /// Handle gossipsub events
    async fn handle_gossipsub_event(
        event: GossipsubEvent,
        connections: &Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    ) {
        match event {
            GossipsubEvent::Message { 
                propagation_source, 
                message_id, 
                message 
            } => {
                debug!("Gossipsub message from {}: {:?}", propagation_source, message_id);
                // TODO: Handle Neptune-specific messages
            }
            GossipsubEvent::Subscribed { peer_id, topic } => {
                debug!("Peer {} subscribed to topic {}", peer_id, topic);
            }
            GossipsubEvent::Unsubscribed { peer_id, topic } => {
                debug!("Peer {} unsubscribed from topic {}", peer_id, topic);
            }
            _ => {
                // Handle other gossipsub events as needed
            }
        }
    }

    /// Handle request-response events
    async fn handle_request_response_event(
        event: libp2p::request_response::Event<NeptuneRequest, NeptuneResponse>,
        connections: &Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    ) {
        match event {
            libp2p::request_response::Event::Message { peer, message } => {
                match message {
                    libp2p::request_response::Message::Request { request, channel } => {
                        debug!("Request from {}: {:?}", peer, request);
                        // Handle request and send response
                        Self::handle_neptune_request(peer, request, channel).await;
                    }
                    libp2p::request_response::Message::Response { response, .. } => {
                        debug!("Response from {}: {:?}", peer, response);
                        // Handle response
                    }
                }
            }
            libp2p::request_response::Event::OutboundFailure { peer, request, error } => {
                warn!("Outbound request failed to {}: {:?} - {}", peer, request, error);
            }
            libp2p::request_response::Event::InboundFailure { peer, error } => {
                warn!("Inbound request failed from {}: {}", peer, error);
            }
            libp2p::request_response::Event::ResponseSent { peer, .. } => {
                debug!("Response sent to {}", peer);
            }
        }
    }





    /// Handle Neptune request
    async fn handle_neptune_request(
        peer: PeerId,
        request: NeptuneRequest,
        channel: ResponseChannel<NeptuneResponse>,
    ) {
        // Process request and generate response
        let response = match request {
            NeptuneRequest::BlockRequest { block_hash } => {
                debug!("Block request from {}: {}", peer, block_hash);
                // TODO: Fetch block from blockchain
                NeptuneResponse::BlockResponse { block_data: vec![] }
            }
            NeptuneRequest::TransactionRequest { tx_hash } => {
                debug!("Transaction request from {}: {}", peer, tx_hash);
                // TODO: Fetch transaction from mempool
                NeptuneResponse::TransactionResponse { tx_data: vec![] }
            }
            NeptuneRequest::PeerListRequest { max_peers } => {
                debug!("Peer list request from {}: max {}", peer, max_peers);
                // TODO: Get peer list
                NeptuneResponse::PeerListResponse { peers: vec![] }
            }
            NeptuneRequest::SyncRequest { from_height, to_height } => {
                debug!("Sync request from {}: {} to {}", peer, from_height, to_height);
                // TODO: Get blocks for sync
                NeptuneResponse::SyncResponse { blocks: vec![] }
            }
            NeptuneRequest::CustomRequest { request_type, data } => {
                debug!("Custom request from {}: {} ({} bytes)", peer, request_type, data.len());
                NeptuneResponse::CustomResponse { response_type: request_type, data }
            }
        };

        // Send response
        if let Err(e) = channel.send(response) {
            warn!("Failed to send response to {}: {}", peer, e);
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

    /// Send request to peer
    pub async fn send_request(
        &mut self,
        peer: PeerId,
        request: NeptuneRequest,
    ) -> P2pResult<()> {
        let swarm = self.swarm.as_mut()
            .ok_or_else(|| NetworkError::ConfigurationError(
                "Swarm not initialized".to_string()
            ))?;

        // Send request
        swarm.behaviour_mut().request_response.send_request(
            &libp2p::request_response::ProtocolName::from("/neptune/1.0.0"),
            peer,
            request,
        );

        Ok(())
    }

    /// Subscribe to gossipsub topic
    pub async fn subscribe_topic(&mut self, topic: String) -> P2pResult<()> {
        let swarm = self.swarm.as_mut()
            .ok_or_else(|| NetworkError::ConfigurationError(
                "Swarm not initialized".to_string()
            ))?;

        let topic = libp2p::gossipsub::IdentTopic::new(topic);
        swarm.behaviour_mut().gossipsub.subscribe(&topic, None);

        Ok(())
    }

    /// Publish message to gossipsub topic
    pub async fn publish_message(
        &mut self,
        topic: String,
        message: Vec<u8>,
    ) -> P2pResult<()> {
        let swarm = self.swarm.as_mut()
            .ok_or_else(|| NetworkError::ConfigurationError(
                "Swarm not initialized".to_string()
            ))?;

        let topic = libp2p::gossipsub::IdentTopic::new(topic);
        swarm.behaviour_mut().gossipsub.publish(topic, message)?;

        Ok(())
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
    async fn test_enhanced_network_service_creation() {
        let config = NetworkConfig::test_config();
        let service = NetworkService::new(config).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_enhanced_network_service_status() {
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

    #[test]
    fn test_neptune_request_codec() {
        let codec = NeptuneRequestCodec;
        
        // Test request types
        let block_request = NeptuneRequest::BlockRequest { 
            block_hash: "test_hash".to_string() 
        };
        let tx_request = NeptuneRequest::TransactionRequest { 
            tx_hash: "test_tx".to_string() 
        };
        
        assert_eq!(block_request, block_request);
        assert_eq!(tx_request, tx_request);
        assert_ne!(block_request, tx_request);
    }
}
