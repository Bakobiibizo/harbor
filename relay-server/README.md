# Harbor Relay Server

A standalone libp2p relay server that enables NAT traversal for Harbor chat app users.

## What it does

This relay server allows Harbor users behind NAT/firewalls to connect with each other by:
1. Accepting relay reservations from clients
2. Forwarding traffic between peers who can't connect directly
3. Supporting DCUtR (Direct Connection Upgrade through Relay) for hole punching

## Building

```bash
cd relay-server
cargo build --release
```

The binary will be at `target/release/harbor-relay`.

## Running

### Basic usage (local testing)
```bash
./harbor-relay --port 4001
```

### Production usage (with public IP)
```bash
./harbor-relay --port 4001 --announce-ip YOUR_PUBLIC_IP
```

### Full options
```bash
./harbor-relay \
  --port 4001 \
  --announce-ip 1.2.3.4 \
  --max-reservations 128 \
  --max-circuits-per-peer 16 \
  --max-circuits 512
```

## Output

When started with `--announce-ip`, the server will print your relay address:

```
YOUR RELAY ADDRESSES:
  TCP:  /ip4/1.2.3.4/tcp/4001/p2p/12D3KooW...
  QUIC: /ip4/1.2.3.4/udp/4001/quic-v1/p2p/12D3KooW...
```

Copy the TCP address and paste it into Harbor's Network settings.

## Deploying on a VPS

### Quick start on Ubuntu/Debian

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone and build
git clone https://github.com/bakobiibizo/harbor.git
cd harbor/relay-server
cargo build --release

# Get your public IP
PUBLIC_IP=$(curl -s https://checkip.amazonaws.com)

# Run the relay
./target/release/harbor-relay --port 4001 --announce-ip $PUBLIC_IP
```

### Running as a systemd service

Create `/etc/systemd/system/harbor-relay.service`:

```ini
[Unit]
Description=Harbor libp2p Relay Server
After=network.target

[Service]
Type=simple
User=ubuntu
Restart=always
RestartSec=10
Environment=RUST_LOG=info
ExecStart=/home/ubuntu/harbor/relay-server/target/release/harbor-relay --port 4001 --announce-ip YOUR_PUBLIC_IP

[Install]
WantedBy=multi-user.target
```

Then:
```bash
sudo systemctl daemon-reload
sudo systemctl enable harbor-relay
sudo systemctl start harbor-relay
```

### Firewall

Make sure port 4001 (or your chosen port) is open for both TCP and UDP:

```bash
# UFW
sudo ufw allow 4001/tcp
sudo ufw allow 4001/udp

# iptables
sudo iptables -A INPUT -p tcp --dport 4001 -j ACCEPT
sudo iptables -A INPUT -p udp --dport 4001 -j ACCEPT
```

## Resource Usage

- Memory: ~10-20 MB idle, scales with active connections
- CPU: Minimal, mostly I/O bound
- Bandwidth: Depends on relay traffic (each circuit limited to 1 MB)

A t2.micro/t3.micro instance can handle hundreds of simultaneous reservations.
