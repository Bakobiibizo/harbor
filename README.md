# Harbor

A decentralized peer-to-peer chat application with local-first data storage, end-to-end encryption, and permission-based content sharing.

## Features

- **Decentralized Identity**: Ed25519 keypairs for signing, X25519 for key agreement
- **Local-First**: All data stored locally in SQLite, you own your data
- **P2P Networking**: Direct peer connections via libp2p (mDNS, Kademlia DHT, NAT traversal)
- **End-to-End Encryption**: AES-256-GCM with counter-based nonces
- **Permission System**: Signed capability grants for content access
- **Event Sourcing**: Append-only logs with lamport clocks for conflict-free sync

## Tech Stack

- **Desktop Framework**: [Tauri](https://tauri.app/) (Rust backend, WebView frontend)
- **Frontend**: React + TypeScript + Zustand
- **P2P**: [rust-libp2p](https://github.com/libp2p/rust-libp2p)
- **Database**: SQLite (via rusqlite)
- **Crypto**: ed25519-dalek, x25519-dalek, aes-gcm, argon2

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/) (stable)
- [Tauri Prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites)

### Setup

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Run Rust tests
cd src-tauri && cargo test
```

### Build

```bash
npm run tauri build
```

## Project Structure

```
harbor/
├── src/                    # React frontend
│   ├── components/         # UI components
│   ├── pages/              # Page components
│   ├── services/           # Tauri command wrappers
│   ├── stores/             # Zustand state stores
│   └── types/              # TypeScript types
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── commands/       # Tauri commands
│   │   ├── db/             # Database & migrations
│   │   ├── models/         # Data models
│   │   ├── p2p/            # libp2p networking
│   │   └── services/       # Business logic
│   └── Cargo.toml
└── package.json
```

## Security Model

### Protected Against
- MITM attacks (Noise transport + E2E encryption)
- Message spoofing (all content signed with Ed25519)
- Replay attacks (nonce tracking, lamport clocks)
- Unauthorized access (permission grants verified)

### Known Limitations (MVP)
- No forward secrecy (no double-ratchet yet)
- Device compromise exposes history
- Connection patterns visible (metadata leakage)

## License

MIT License - see [LICENSE](LICENSE)
