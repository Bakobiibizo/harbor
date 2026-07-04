# Harbor

A decentralized peer-to-peer chat application with local-first data storage, end-to-end encryption, and permission-based content sharing.

## Branding

- **Product name:** Harbor
- **Tagline:** Decentralized Chat
- **Public site / invite handoff:** `https://social-harbor.com`
- **Default community relay:** Harbor Community Relay
- **Default relay address:**
  ```text
  /ip4/100.49.236.191/tcp/4001/p2p/12D3KooWMfwHKfzDrZ2V3Zniw3Qu797bHrKsFKAdG9CtQiaEhbQ3
  ```

Harbor contact invites use the public site as a friendly handoff URL and embed the full `harbor://` contact bundle needed by the desktop app to add/connect to a contact.

## Features

- **Decentralized Identity**: Ed25519 keypairs for signing, X25519 for key agreement
- **Local-First**: All data stored locally in SQLite, you own your data
- **P2P Networking**: Direct peer connections via libp2p (mDNS, Kademlia DHT, NAT traversal)
- **End-to-End Encryption**: AES-256-GCM with HKDF-derived conversation keys
- **Permission System**: Signed capability grants for content access (Chat, WallRead, Call)
- **Event Sourcing**: Append-only logs with lamport clocks for conflict-free sync
- **One-to-One Voice Calling**: signed libp2p signaling, WebRTC audio runtime, persisted call history, and configurable ICE/STUN/TURN settings; release readiness still requires the two-profile evidence in [`docs/voice-call-e2e-validation.md`](docs/voice-call-e2e-validation.md)
- **Wall and Feed Sync**: local-first wall posts with visibility controls, media, preview/RSS/share surfaces, contact-wall/feed reads, signed comments/reactions, edit/delete reconciliation, and direct/relay sync; release readiness still requires the three-profile evidence in [`docs/wall-sync-multi-profile-validation.md`](docs/wall-sync-multi-profile-validation.md)
- **Group-Call Topology Contract**: first production group calls use the [ADR-0001 relay-assisted small-group mesh](docs/architecture/adr-0001-group-call-topology.md) with a hard 4-participant limit; video/group claims require [`docs/video-group-call-validation.md`](docs/video-group-call-validation.md)

## Quick Start

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/) (stable)
- [Tauri Prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites)
  - Windows: Microsoft Visual Studio C++ Build Tools
  - macOS: Xcode Command Line Tools
  - Linux: `sudo apt install libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`

### Installation

```bash
# Clone the repository
git clone https://github.com/Bakobiibizo/harbor.git
cd harbor

# Install frontend dependencies
pnpm install

# Run in development mode
pnpm tauri dev
```

### Building for Production

```bash
# Build the application
pnpm tauri build

# The executable will be in src-tauri/target/release/
```

## Usage Guide

### First Launch - Create Your Identity

1. When you first open Harbor, you'll be prompted to create an identity
2. Enter a **Display Name** (how others will see you)
3. Optionally add a **Bio**
4. Create a **Passphrase** (at least 8 characters) - this encrypts your private keys
5. **Important**: Store your passphrase safely! If you lose it, you cannot recover your identity

### Unlocking Your Identity

- On subsequent launches, enter your passphrase to unlock
- Your identity remains encrypted on disk when locked

### Starting the Network

1. Go to the **Network** tab
2. Click **Start Network** to connect to the P2P network
3. Peers on your local network running Harbor will be discovered automatically via mDNS
4. The status indicator shows your connection state

### Managing Contacts

1. In the **Network** tab, you'll see discovered peers
2. Click the checkmark to add a peer as a contact
3. You can search for peers by their Peer ID
4. Use the **Contacts** tab to manage your contact list

### Direct Messaging

1. Go to the **Messages** tab
2. Select a contact to open a conversation
3. Messages are end-to-end encrypted using derived conversation keys
4. Click the phone icon to initiate a voice call (if supported)

### Posting to Your Wall

1. Go to the **Wall** tab
2. Use the composer at the top to create a post and choose **Public** or **Contacts only** visibility
3. You can add images and videos to your posts
4. Use **Preview and share your wall** to view guest/contact/owner perspectives, copy/export public-only RSS XML, copy your public feed URI, or copy a contact invite
5. Posts are stored locally and shared with contacts who have permission

RSS XML is generated locally from public posts only; Harbor does not currently host RSS over HTTP. See [Wall preview, RSS, and share surfaces](docs/wall-preview-rss-share.md) for the exact visibility behavior.

### Viewing Your Feed

1. Go to the **Feed** tab
2. See posts from contacts whose public posts are available or who have granted you WallRead permission for contacts-only posts
3. Like/react, comment, save, hide, snooze, and share through the production feed/contact-wall surfaces where the current build exposes those controls; relay and multi-profile release evidence is tracked in `docs/wall-sync-multi-profile-validation.md`

### Settings

Access settings to:
- **Profile**: Update your display name, bio, and avatar
- **Security**: Change passphrase, export/import identity
- **Network**: Configure auto-start and mDNS discovery
- **Privacy**: Control post visibility and read receipts

## Architecture

### Frontend (React + TypeScript)

```
src/
├── components/
│   ├── common/          # Button, Input, etc.
│   ├── icons/           # SVG icon components
│   ├── layout/          # MainLayout with sidebar
│   └── onboarding/      # CreateIdentity, UnlockIdentity
├── pages/
│   ├── Chat.tsx         # Direct messaging
│   ├── Wall.tsx         # Your posts
│   ├── Feed.tsx         # Posts from contacts
│   ├── Network.tsx      # Peer discovery & contacts
│   └── Settings.tsx     # App configuration
├── services/            # Tauri command wrappers
│   ├── identity.ts
│   ├── network.ts
│   ├── contacts.ts
│   ├── permissions.ts
│   ├── messaging.ts
│   ├── posts.ts
│   ├── feed.ts
│   └── calling.ts
├── stores/              # Zustand state management
│   ├── identity.ts
│   └── network.ts
├── types/               # TypeScript interfaces
└── styles/
    └── design-system.css  # CSS custom properties
```

### Backend (Rust + Tauri)

```
src-tauri/src/
├── commands/            # Tauri command handlers
│   ├── identity.rs
│   ├── network.rs
│   ├── contacts.rs
│   ├── permissions.rs
│   ├── messaging.rs
│   ├── posts.rs
│   ├── feed.rs
│   └── calling.rs
├── services/            # Business logic
│   ├── identity_service.rs    # Key management
│   ├── crypto_service.rs      # Encryption/signing
│   ├── contacts_service.rs    # Contact management
│   ├── permissions_service.rs # Capability grants
│   ├── messaging_service.rs   # Direct messages
│   ├── posts_service.rs       # Wall posts
│   ├── feed_service.rs        # Feed aggregation
│   ├── content_sync_service.rs # P2P sync
│   └── calling_service.rs     # Voice calls
├── db/
│   ├── mod.rs           # Database initialization
│   ├── migrations/      # SQL migrations
│   └── repositories/    # Data access layer
├── models/              # Data structures
└── p2p/
    ├── network.rs       # libp2p swarm
    └── protocols/       # Request-response protocols
```

### Database Schema (SQLite)

- `local_identity` - Your encrypted keypairs and profile
- `contacts` - Peer information and trust levels
- `permission_events` - Grant/revoke events (event sourced)
- `permissions_current` - Materialized permission state
- `message_events` - Message lifecycle events
- `messages` - Materialized messages for UI
- `post_events` - Post lifecycle events
- `posts` - Materialized posts
- `post_media` - Media metadata (files stored on disk)
- `call_history` - Voice call records
- `sync_state` - Per-peer sync progress
- `sync_queue` - Offline message queue
- `lamport_clock` - Logical clock for ordering

## Security Model

### Cryptography

| Purpose | Algorithm | Notes |
|---------|-----------|-------|
| Identity signing | Ed25519 | All messages signed |
| Key agreement | X25519 | Derived from Ed25519 |
| Conversation encryption | AES-256-GCM | HKDF-derived keys |
| Key encryption | Argon2id + AES-GCM | Passphrase-based |
| Content hashing | SHA-256 | Media content-addressing |

### Permission System

Permissions are signed, portable capability grants:

```rust
struct PermissionGrant {
    grant_id: Uuid,
    issuer_peer_id: PeerId,      // Who grants
    subject_peer_id: PeerId,     // Who receives
    capability: Capability,       // Chat, WallRead, Call
    issued_at: u64,
    expires_at: Option<u64>,
    signature: Vec<u8>,          // Ed25519 signature
}
```

### Protected Against
- MITM attacks (Noise protocol transport + E2E encryption)
- Message spoofing (all content signed with Ed25519)
- Replay attacks (nonce tracking, lamport clocks, message IDs)
- Unauthorized access (permission grants verified on every request)

### Known Limitations (MVP)
- No forward secrecy (no double-ratchet yet - compromise exposes history)
- No HSM/secure enclave integration
- Connection patterns visible (metadata leakage)
- Calls use no hard-coded third-party STUN/TURN service by default; strict NAT pairs require operator-configured TURN, and group calls are capped at 4 total participants by [ADR-0001](docs/architecture/adr-0001-group-call-topology.md)

## Protocol Messages (CBOR)

### Identity Exchange
- `IdentityRequest` / `IdentityResponse` - Exchange peer info

### Permissions
- `PermissionRequest` - Request capability from peer
- `PermissionGrant` - Grant capability to peer
- `PermissionRevoke` - Revoke previously granted capability

### Messaging
- `DirectMessage` - Encrypted message with signature
- `MessageAck` - Delivery/read receipt

### Content Sync
- `ContentManifestRequest/Response` - List available posts
- `ContentFetchRequest` - Request specific post
- `MediaChunkRequest/Response` - Transfer media files

### Calling
One-to-one voice signaling and WebRTC call runtime paths are implemented, but release notes must be gated by the two-profile validation checklist. Group audio/video signaling must follow [ADR-0001](docs/architecture/adr-0001-group-call-topology.md): relay-assisted small-group full mesh, maximum 4 total participants, signed roster-bound messages, and no SFU/MCU behavior without a replacement ADR. Screen sharing remains deferred until implementation and validation land.

- `SignalingOffer/Answer` - WebRTC SDP exchange
- `SignalingIce` - ICE candidate exchange
- `SignalingHangup` - End call

## Voice Call ICE/STUN/TURN Behavior

Harbor keeps voice calls LAN/direct-capable by default and does not bundle private TURN credentials or depend on an undeclared third-party TURN service.

- Default runtime: `iceServers: []`, `iceTransportPolicy: "all"`. Browser host candidates remain enabled, so LAN/direct calls are not blocked when no TURN server is configured.
- Operators/users can add `stun:`, `stuns:`, `turn:`, and `turns:` entries in **Settings → Calls**. TURN/TURNS entries require username and credential fields; credentials embedded in URLs are rejected.
- TURN credential persistence is explicit. The default is **This session only**, which is usable for the current runtime but redacts the credential from persisted settings. **Save on this device** stores the credential locally for operator-managed deployments.
- libp2p relay connectivity and WebRTC media relay are separate. Harbor/libp2p relays can carry call signaling, but audio media relay requires TURN/TURNS.
- If ICE fails without usable TURN, Harbor reports strict-NAT guidance. If relay-only media is requested without TURN, Harbor reports that TURN is required rather than implying libp2p relay can carry media.

Manual validation checklist for call networking changes:

1. Start two local Harbor profiles on the same LAN with no ICE servers configured and confirm a voice call still reaches ICE gathering/connection through host candidates.
2. Add an operator STUN or TURN test entry in Settings → Calls and confirm the generated `RTCPeerConnection` configuration contains the configured ICE server.
3. Force a controlled failure with `iceTransportPolicy: "relay"` and no TURN entry; confirm the user-facing error mentions WebRTC TURN media relay and distinguishes it from libp2p relay signaling.

## Development

### Running Tests

```bash
# Rust/Tauri release gate
.dev/bin/dev ci --language rust

# Frontend TypeScript release gate
.dev/bin/dev ci --language typescript

# Relay release gate
cargo fmt --manifest-path relay-server/Cargo.toml -- --check
cargo check --manifest-path relay-server/Cargo.toml
cargo clippy --manifest-path relay-server/Cargo.toml -- -D warnings
cargo test --manifest-path relay-server/Cargo.toml

# Interactive release evidence (desktop/WebView required)
# See docs/release-gates-calls-wall-sync.md,
# docs/voice-call-e2e-validation.md, docs/video-group-call-validation.md,
# and docs/wall-sync-multi-profile-validation.md.
```

### Code Structure

The codebase follows these patterns:
- **Event Sourcing**: All state changes are events with lamport clocks
- **CQRS**: Events stored separately from materialized views
- **Repository Pattern**: Data access abstracted behind repositories
- **Service Layer**: Business logic in services, commands are thin wrappers

## Roadmap

### Completed (Phases 1-8)
- [x] Identity system with encrypted key storage
- [x] P2P networking with libp2p
- [x] Contact management
- [x] Permission grants/revokes
- [x] Direct messaging (encrypted)
- [x] Wall/blog posts with media
- [x] Feed aggregation
- [x] Voice calling implementation paths (signaling/runtime/UI) with automated coverage; two-profile release evidence remains a required gate
- [x] Wall/feed sync implementation paths with automated coverage; three-profile release evidence remains a required gate
- [x] Modern, polished UI

### Future (Stretch Goals)
- [ ] Double-ratchet for forward secrecy
- [ ] Screen sharing within the ADR-0001 small-group mesh contract
- [ ] Larger group rooms beyond the 4-participant ADR-0001 cap
- [ ] Group chats
- [ ] Mobile app (iOS/Android via Tauri)
- [ ] Production TURN deployment/credential rotation guide for strict-NAT support
- [ ] Profile photo uploads
- [ ] Read receipts
- [ ] Typing indicators

## Contributing

Contributions are welcome! Please open an issue or PR.

## License

MIT License - see [LICENSE](LICENSE)
