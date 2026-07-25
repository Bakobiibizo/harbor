# Harbor Relay Server

A standalone libp2p relay server that enables NAT traversal and relay-scoped identity for Harbor chat app users. Every mode persists identity, introductions, and wall-sync data. Community mode additionally enables community boards.

## What it does

This relay server allows Harbor users behind NAT/firewalls to connect with each other by:

1. Accepting relay reservations from clients
2. Forwarding libp2p traffic between peers who can't connect directly
3. Supporting DCUtR (Direct Connection Upgrade through Relay) for hole punching
4. Persisting relay names, introductions, wall data, media metadata, and `WallRead` grants in every mode
5. Optionally enabling community boards with `--community`

The relay carries Harbor/libp2p traffic and call signaling. It is **not** a WebRTC TURN, SFU, MCU, or media-recording server; call media relay requires separately configured TURN.

## Building

```bash
cd relay-server
cargo build --release
```

The binary will be at `target/release/harbor-relay`.

## Running

### Basic usage (local testing)

```bash
./harbor-relay \
  --port 4001 \
  --identity-namespace relay.example.com \
  --data-dir /tmp/harbor-relay-data
```

### Community board testing

```bash
./harbor-relay \
  --port 4001 \
  --identity-namespace relay.example.com \
  --community \
  --community-name "Harbor Test" \
  --data-dir /tmp/harbor-relay-data
```

### Production usage (with public IP)

```bash
./harbor-relay \
  --port 4001 \
  --announce-ip YOUR_PUBLIC_IP \
  --identity-namespace relay.example.com \
  --data-dir /var/lib/harbor-relay/data
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

Circuit bytes, duration, idle connections, concurrency, admission rates, cleanup, abuse budgets, and persistent relay storage all have finite validated defaults. Every option also has a `HARBOR_RELAY_*` environment binding. See [relay-resource-limits.md](../docs/relay-resource-limits.md) for the complete table and AWS parameter mapping.

## Output

When started with `--announce-ip`, the server will print your relay address:

```
YOUR RELAY ADDRESSES:
  TCP:  /ip4/1.2.3.4/tcp/4001/p2p/12D3KooW...
  QUIC: /ip4/1.2.3.4/udp/4001/quic-v1/p2p/12D3KooW...
```

Copy the TCP address and paste it into Harbor's Network settings. Use the same relay address when running the multi-profile wall-sync validation in `../docs/wall-sync-multi-profile-validation.md`.

`--announce-ip` accepts only a public, routable IPv4 address. Wildcard, loopback, private, link-local, multicast, documentation, and other reserved addresses are rejected and are never advertised to peers.

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
ExecStart=/home/ubuntu/harbor/relay-server/target/release/harbor-relay --port 4001 --announce-ip YOUR_PUBLIC_IP --identity-namespace relay.example.com

[Install]
WantedBy=multi-user.target
```

Then:

```bash
sudo systemctl daemon-reload
sudo systemctl enable harbor-relay
sudo systemctl start harbor-relay
```

`--identity-namespace` is required for relay-scoped names such as `@alice@relay.example.com`. Use a lowercase DNS hostname you control, point its DNS record at the stable relay address, and provision HTTPS before exposing HTTP-based discovery endpoints. The relay is authoritative only for its own namespace; it is not a global Harbor user directory.

The identity key is also the relay namespace authority. Do not replace it as an ordinary binary update. Planned rotations and compromise recovery must follow [`../docs/relay-key-rotation-operations.md`](../docs/relay-key-rotation-operations.md).

On Unix, the relay creates a missing identity key atomically with mode `0600`. It refuses to start when an existing key is a symlink or non-regular file, is owned by a different user, or has a mode other than owner-read-only (`0400`) or owner-read/write (`0600`). Repair the ownership or permissions deliberately; do not copy the key through logs or command-line arguments.

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

## Release validation

Before publishing relay artifacts or claiming wall-sync release readiness, run:

```bash
cargo fmt --manifest-path relay-server/Cargo.toml -- --check
cargo check --manifest-path relay-server/Cargo.toml
cargo clippy --manifest-path relay-server/Cargo.toml -- -D warnings
cargo test --manifest-path relay-server/Cargo.toml
```

Relay tests are still not a substitute for the Harbor multi-profile scenarios documented in `../docs/wall-sync-multi-profile-validation.md`.

## Resource Usage

- Memory: ~10-20 MB idle, scales with active connections
- CPU: Minimal, mostly I/O bound
- Bandwidth: Depends on relay traffic (each circuit defaults to 64 MiB and one hour maximum)

A t2.micro/t3.micro instance can handle hundreds of simultaneous reservations.
