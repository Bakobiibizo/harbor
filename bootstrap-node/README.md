# Harbor Bootstrap Node

A lightweight bootstrap/rendezvous server that enables Harbor peers to discover each other across different networks.

## What It Does

- Maintains a Kademlia DHT (Distributed Hash Table) for peer routing
- Helps peers find each other without being on the same local network
- Does NOT store messages or user data - purely for discovery

## Quick Start

### Option 1: Run with Cargo (Development)

```bash
cd bootstrap-node
cargo run --release -- --port 9000 --external-ip YOUR_PUBLIC_IP
```

### Option 2: Docker

```bash
cd bootstrap-node
docker build -t harbor-bootstrap .
docker run -p 9000:9000 harbor-bootstrap --external-ip YOUR_PUBLIC_IP
```

### Option 3: Docker Compose

Create a `.env` file:
```
EXTERNAL_IP=1.2.3.4
```

Then run:
```bash
docker-compose up -d
```

## Command Line Options

```
harbor-bootstrap [OPTIONS]

Options:
  -p, --port <PORT>          Port to listen on [default: 9000]
      --external-ip <IP>     External IP address for NAT traversal
  -v, --verbose              Enable verbose logging
  -h, --help                 Print help
```

## Connecting Peers

When the bootstrap node starts, it will print a multiaddress like:

```
/ip4/1.2.3.4/tcp/9000/p2p/12D3KooWAbCdEfGhIjKlMnOpQrStUvWxYz...
```

Share this address with Harbor users. They can add it in Settings > Network > Bootstrap Nodes.

## Firewall Configuration

Ensure port 9000 (or your chosen port) is open for TCP traffic:

```bash
# UFW (Ubuntu)
sudo ufw allow 9000/tcp

# firewalld (CentOS/RHEL)
sudo firewall-cmd --permanent --add-port=9000/tcp
sudo firewall-cmd --reload
```

## Self-Hosting Tips

1. **Use a static IP** or dynamic DNS service
2. **Keep the node running** - peers depend on it for discovery
3. **Monitor logs** for connection issues
4. **Consider running multiple nodes** for redundancy

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Bootstrap Node                         │
│                                                          │
│  ┌──────────┐  ┌───────────┐  ┌──────────────────────┐ │
│  │   Ping   │  │ Identify  │  │    Kademlia DHT      │ │
│  │ keepalive│  │  protocol │  │  peer routing table  │ │
│  └──────────┘  └───────────┘  └──────────────────────┘ │
│                                                          │
│                    libp2p TCP + Noise                    │
└─────────────────────────────────────────────────────────┘
                           │
           ┌───────────────┼───────────────┐
           │               │               │
           ▼               ▼               ▼
      ┌─────────┐    ┌─────────┐    ┌─────────┐
      │ Peer A  │    │ Peer B  │    │ Peer C  │
      │ Harbor  │    │ Harbor  │    │ Harbor  │
      └─────────┘    └─────────┘    └─────────┘
```

## Security Notes

- The bootstrap node only facilitates peer discovery
- It does NOT relay messages or store any user data
- All actual communication happens directly between peers
- Messages are end-to-end encrypted between peers
