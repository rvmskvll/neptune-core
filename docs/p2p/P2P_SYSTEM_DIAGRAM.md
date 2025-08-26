# Neptune Core P2P System Architecture

This document contains the Mermaid diagram showing our new P2P system architecture and how it integrates with the legacy network via our compatibility bridge.

## System Architecture Overview

```mermaid
graph TB
    %% Application Layer
    subgraph "Application Layer"
        ML[Main Loop]
        MLM[Mining Loop]
        RPC[RPC Server]
        BC[Blockchain]
        MP[Mempool]
    end

    %% New P2P Layer
    subgraph "New P2P Layer (libp2p)"
        NS[NetworkService]
        TS[TransportService]
        DS[DiscoveryService]
        PH[ProtocolHandler]
        MC[MessageCodec]
    end

    %% libp2p Protocols
    subgraph "libp2p Protocols"
        ID[Identify]
        PG[Ping]
        KD[Kademlia DHT]
        MD[mDNS]
        GS[Gossipsub]
        RR[Request-Response]
    end

    %% Compatibility Bridge
    subgraph "Compatibility Bridge"
        LB[LegacyBridge]
        MA[MessageAdapter]
        LNS[LegacyNetworkService]
        MM[MigrationManager]
    end

    %% Legacy Network
    subgraph "Legacy Network (TCP)"
        LML[Legacy Main Loop]
        LPL[Legacy Peer Loop]
        LCL[Legacy Connection Logic]
        LPM[Legacy PeerMessage]
    end

    %% Network Transport
    subgraph "Network Transport"
        TCP[TCP]
        NO[Noise Encryption]
        YM[Yamux Multiplexing]
    end

    %% Peer Network
    subgraph "Peer Network"
        P1[Peer 1]
        P2[Peer 2]
        P3[Peer 3]
        P4[Peer 4]
    end

    %% Application Layer Connections
    ML --> NS
    MLM --> NS
    RPC --> NS
    BC --> NS
    MP --> NS

    %% P2P Layer Internal
    NS --> TS
    NS --> DS
    NS --> PH
    PH --> MC

    %% Protocol Connections
    NS --> ID
    NS --> PG
    NS --> KD
    NS --> MD
    NS --> GS
    NS --> RR

    %% Transport Layer
    TS --> TCP
    TS --> NO
    TS --> YM

    %% Compatibility Bridge Connections
    NS --> LB
    LB --> MA
    LB --> LNS
    LB --> MM

    %% Legacy Network Integration
    LNS --> LML
    LNS --> LPL
    LNS --> LCL
    MA --> LPM

    %% Network to Peers
    TCP --> P1
    TCP --> P2
    TCP --> P3
    TCP --> P4

    %% Peer Discovery
    KD --> P1
    KD --> P2
    KD --> P3
    KD --> P4
    MD --> P1
    MD --> P2
    MD --> P3
    MD --> P4

    %% Message Flow
    GS --> P1
    GS --> P2
    GS --> P3
    GS --> P4
    RR --> P1
    RR --> P2
    RR --> P3
    RR --> P4

    %% Styling
    classDef appLayer fill:#e1f5fe,stroke:#01579b,stroke-width:2px
    classDef p2pLayer fill:#f3e5f5,stroke:#4a148c,stroke-width:2px
    classDef protocols fill:#e8f5e8,stroke:#1b5e20,stroke-width:2px
    classDef bridge fill:#fff3e0,stroke:#e65100,stroke-width:2px
    classDef legacy fill:#ffebee,stroke:#c62828,stroke-width:2px
    classDef transport fill:#f1f8e9,stroke:#33691e,stroke-width:2px
    classDef peers fill:#fce4ec,stroke:#880e4f,stroke-width:2px

    class ML,MLM,RPC,BC,MP appLayer
    class NS,TS,DS,PH,MC p2pLayer
    class ID,PG,KD,MD,GS,RR protocols
    class LB,MA,LNS,MM bridge
    class LML,LPL,LCL,LPM legacy
    class TCP,NO,YM transport
    class P1,P2,P3,P4 peers
```

## Detailed Component Relationships

```mermaid
flowchart TD
    %% Main Application Flow
    subgraph "Application Flow"
        direction TB
        A[Application Request] --> B{Network Mode?}
        B -->|Legacy| C[Legacy Bridge]
        B -->|Libp2p| D[New P2P Layer]
        B -->|Compatibility| E[Dual Mode]
    end

    %% Legacy Bridge Flow
    subgraph "Legacy Bridge Flow"
        direction TB
        C --> F[MessageAdapter]
        F --> G[Convert Legacy to Internal]
        G --> H[Route to New P2P]
        F --> I[Convert Internal to Legacy]
        I --> J[Route to Legacy Network]
    end

    %% New P2P Flow
    subgraph "New P2P Flow"
        direction TB
        D --> K[ProtocolHandler]
        K --> L[MessageCodec]
        L --> M[NetworkService]
        M --> N[libp2p Swarm]
        N --> O[Transport Layer]
        O --> P[Peer Network]
    end

    %% Dual Mode Flow
    subgraph "Dual Mode Flow"
        direction TB
        E --> Q[MessageRouter]
        Q --> R{Message Type?}
        R -->|Block/Transaction| S[New P2P]
        R -->|Handshake/Sync| T[Legacy]
        R -->|Hybrid| U[Both Networks]
    end

    %% Migration Flow
    subgraph "Migration Flow"
        direction TB
        V[MigrationManager] --> W{Phase?}
        W -->|Phase 1| X[Foundation]
        W -->|Phase 2| Y[Core Implementation]
        W -->|Phase 3| Z[Integration]
        W -->|Phase 4| AA[Testing]
        W -->|Phase 5| BB[Legacy Removal]
    end

    %% Styling
    classDef app fill:#e3f2fd,stroke:#1976d2,stroke-width:2px
    classDef bridge fill:#fff8e1,stroke:#f57c00,stroke-width:2px
    classDef p2p fill:#f3e5f5,stroke:#7b1fa2,stroke-width:2px
    classDef dual fill:#e8f5e8,stroke:#388e3c,stroke-width:2px
    classDef migration fill:#fce4ec,stroke:#c2185b,stroke-width:2px

    class A,B,C,D,E app
    class F,G,H,I,J bridge
    class K,L,M,N,O,P p2p
    class Q,R,S,T,U dual
    class V,W,X,Y,Z,AA,BB migration
```

## Message Flow Architecture

```mermaid
sequenceDiagram
    participant App as Application
    participant PH as ProtocolHandler
    participant MC as MessageCodec
    participant NS as NetworkService
    participant LB as LegacyBridge
    participant LN as LegacyNetwork
    participant Peer as Peer Node

    %% New P2P Message Flow
    App->>PH: Create ProtocolMessage
    PH->>MC: Encode Message
    MC->>NS: Send Encoded Message
    NS->>Peer: Send via libp2p

    %% Legacy Message Flow
    App->>LB: Create Legacy Message
    LB->>LN: Send via TCP
    LN->>Peer: Send via Legacy Protocol

    %% Hybrid Message Flow
    App->>PH: Create Hybrid Message
    PH->>LB: Route to Both Networks
    LB->>LN: Send to Legacy
    PH->>NS: Send to New P2P
    NS->>Peer: Send via libp2p
    LN->>Peer: Send via Legacy

    %% Response Flow
    Peer->>NS: Response via libp2p
    NS->>MC: Decode Response
    MC->>PH: Process Response
    PH->>App: Deliver Response

    Peer->>LN: Response via Legacy
    LN->>LB: Process Legacy Response
    LB->>App: Deliver Legacy Response
```

## Network Topology

```mermaid
graph LR
    %% Neptune Node
    subgraph "Neptune Core Node"
        NC[Neptune Core]
        NS[Network Service]
        LB[Legacy Bridge]
    end

    %% Network Connections
    subgraph "Network Connections"
        direction TB
        TCP[TCP Connections]
        UDP[UDP Discovery]
        RELAY[Circuit Relay]
    end

    %% Peer Types
    subgraph "Peer Types"
        direction TB
        LP[Legacy Peers<br/>TCP Only]
        NP[New Peers<br/>libp2p Only]
        HP[Hybrid Peers<br/>Both Protocols]
    end

    %% Discovery Methods
    subgraph "Discovery Methods"
        direction TB
        KD[Kademlia DHT]
        MD[mDNS Local]
        BOOT[Bootstrap Nodes]
        MAN[Manual Entry]
    end

    %% Connection Flow
    NC --> NS
    NC --> LB
    NS --> TCP
    NS --> UDP
    LB --> TCP
    TCP --> LP
    TCP --> NP
    TCP --> HP
    UDP --> KD
    UDP --> MD
    UDP --> BOOT
    UDP --> MAN

    %% Styling
    classDef node fill:#e3f2fd,stroke:#1976d2,stroke-width:2px
    classDef network fill:#f3e5f5,stroke:#7b1fa2,stroke-width:2px
    classDef peers fill:#e8f5e8,stroke:#388e3c,stroke-width:2px
    classDef discovery fill:#fff3e0,stroke:#f57c00,stroke-width:2px

    class NC,NS,LB node
    class TCP,UDP,RELAY network
    class LP,NP,HP peers
    class KD,MD,BOOT,MAN discovery
```

## Protocol Stack

```mermaid
graph TB
    %% Application Layer
    subgraph "Application Layer"
        APP[Neptune Application]
        PROTO[Protocol Handler]
        CODEC[Message Codec]
    end

    %% libp2p Layer
    subgraph "libp2p Layer"
        SWARM[Swarm]
        ID[Identify]
        PG[Ping]
        KD[Kademlia]
        MD[mDNS]
        GS[Gossipsub]
        RR[Request-Response]
    end

    %% Transport Layer
    subgraph "Transport Layer"
        TCP[TCP]
        NOISE[Noise]
        YAMUX[Yamux]
    end

    %% Network Layer
    subgraph "Network Layer"
        IP[IP]
        ETH[Ethernet]
    end

    %% Legacy Compatibility
    subgraph "Legacy Compatibility"
        LB[Legacy Bridge]
        LPM[Legacy PeerMessage]
        LTCP[Legacy TCP]
    end

    %% Stack Connections
    APP --> PROTO
    PROTO --> CODEC
    CODEC --> SWARM
    SWARM --> ID
    SWARM --> PG
    SWARM --> KD
    SWARM --> MD
    SWARM --> GS
    SWARM --> RR
    SWARM --> TCP
    TCP --> NOISE
    NOISE --> YAMUX
    YAMUX --> IP
    IP --> ETH

    %% Legacy Path
    APP --> LB
    LB --> LPM
    LPM --> LTCP
    LTCP --> IP

    %% Styling
    classDef app fill:#e3f2fd,stroke:#1976d2,stroke-width:2px
    classDef p2p fill:#f3e5f5,stroke:#7b1fa2,stroke-width:2px
    classDef transport fill:#e8f5e8,stroke:#388e3c,stroke-width:2px
    classDef network fill:#fff3e0,stroke:#f57c00,stroke-width:2px
    classDef legacy fill:#ffebee,stroke:#c62828,stroke-width:2px

    class APP,PROTO,CODEC app
    class SWARM,ID,PG,KD,MD,GS,RR p2p
    class TCP,NOISE,YAMUX transport
    class IP,ETH network
    class LB,LPM,LTCP legacy
```

## Key Features and Benefits

### **Enhanced libp2p Protocols**
- **Identify**: Peer information exchange and version negotiation
- **Ping**: Connection health monitoring with RTT calculation
- **Kademlia DHT**: Distributed peer discovery and routing
- **mDNS**: Local network peer discovery
- **Gossipsub**: Efficient message broadcasting and subscription
- **Request-Response**: Direct peer communication for specific requests

### **Compatibility Bridge Benefits**
- **Seamless Migration**: Gradual transition from legacy to new network
- **Dual Protocol Support**: Operate both networks simultaneously
- **Message Translation**: Automatic conversion between protocol formats
- **Backward Compatibility**: Maintain existing functionality during transition

### **Performance Improvements**
- **Modern Networking**: Industry-standard libp2p protocols
- **Efficient Discovery**: Kademlia DHT for scalable peer discovery
- **Optimized Transport**: TCP + Noise + Yamux for performance
- **Message Compression**: zstd compression for reduced bandwidth
- **Encryption**: ChaCha20-Poly1305 for secure communication

### **Scalability Features**
- **Distributed Discovery**: Kademlia DHT for large-scale networks
- **Efficient Routing**: TTL-based message routing
- **Connection Multiplexing**: Yamux for multiple streams per connection
- **Peer Management**: Intelligent peer caching and cleanup

This architecture provides a robust, scalable, and future-proof P2P networking solution while maintaining full backward compatibility with the existing Neptune Core implementation.
