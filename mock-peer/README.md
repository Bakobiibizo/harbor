# Harbor Mock Peer Server

A standalone mock peer server that uses the same libp2p protocols as Harbor for testing P2P connectivity.

## Features

- **mDNS Discovery**: Automatically announces itself on the local network
- **Identity Exchange**: Responds to identity requests with a valid signed identity
- **Messaging**: Receives messages and sends acknowledgments
- **Auto-Reply**: Generates contextual responses to incoming messages

## Building

```bash
cd mock-peer
cargo build --release
```

## Usage

```bash
# Run with default settings
cargo run --release

# Run with custom name and bio
cargo run --release -- --name "Alice" --bio "Test peer for Harbor"

# Run on a specific port
cargo run --release -- --port 9000
```

### Command Line Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--name` | `-n` | "Mock Peer" | Display name for the peer |
| `--bio` | `-b` | "A mock peer for testing Harbor P2P" | Bio/description |
| `--port` | `-p` | 0 (random) | TCP port to listen on |

## How It Works

1. **Startup**: Generates a new Ed25519 keypair and derives the libp2p Peer ID
2. **Discovery**: Announces via mDNS so Harbor instances can discover it
3. **Connection**: Accepts incoming connections from Harbor clients
4. **Identity Exchange**: When Harbor requests identity, responds with signed identity info
5. **Messaging**: Acknowledges received messages and logs auto-reply content

## Protocol Compatibility

This server implements the same protocols as Harbor:

- `/harbor/1.0.0` - Identify protocol
- `/harbor/identity/1.0.0` - Identity exchange (request/response)
- `/harbor/messaging/1.0.0` - Messaging (request/response)

## Testing Workflow

1. Start the mock peer server:
   ```bash
   cargo run --release -- --name "Test Peer" --port 9000
   ```

2. Start Harbor application and enable networking

3. The mock peer should appear in Harbor's Network page

4. Try sending a message to the mock peer from Harbor's Chat page

## Logging

Set the `RUST_LOG` environment variable to control log verbosity:

```bash
# Show all logs
RUST_LOG=debug cargo run

# Show only info and above
RUST_LOG=info cargo run

# Show libp2p mDNS debug info
RUST_LOG=libp2p_mdns=debug cargo run
```

## Example Output

```
2024-01-15T10:30:00 INFO harbor_mock_peer: Starting Harbor Mock Peer Server
2024-01-15T10:30:00 INFO harbor_mock_peer: Name: Mock Peer
2024-01-15T10:30:00 INFO harbor_mock_peer: Bio: A mock peer for testing Harbor P2P
2024-01-15T10:30:00 INFO harbor_mock_peer: Peer ID: 12D3KooWExample...
2024-01-15T10:30:00 INFO harbor_mock_peer: Listening on /ip4/0.0.0.0/tcp/52431/p2p/12D3KooWExample...
2024-01-15T10:30:00 INFO harbor_mock_peer: Mock peer is running. Press Ctrl+C to stop.
2024-01-15T10:30:05 INFO harbor_mock_peer: mDNS discovered peer: 12D3KooWHarbor...
2024-01-15T10:30:06 INFO harbor_mock_peer: Connected to peer: 12D3KooWHarbor...
2024-01-15T10:30:07 INFO harbor_mock_peer: Identity request from 12D3KooWHarbor...
2024-01-15T10:30:07 INFO harbor_mock_peer: Sending identity response: name=Mock Peer
```
