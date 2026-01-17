# Harbor - Decentralized P2P Chat Application

## Project Overview
A Tauri-based decentralized chat application with local-first data storage, peer-to-peer communication via libp2p, and permission-based content sharing.

## Tech Stack
- **Desktop Framework**: Tauri (Rust backend + React frontend)
- **Frontend**: React + TypeScript + Tailwind CSS + Zustand
- **P2P Networking**: rust-libp2p with mDNS, Kademlia DHT, NAT traversal
- **Database**: SQLite with event-sourced schema
- **State Management**: Zustand stores

## Project Structure
```
D:\apps\chat-app\
├── src-tauri/           # Rust backend
│   ├── src/
│   │   ├── commands/    # Tauri command handlers
│   │   ├── services/    # Business logic (identity, posts)
│   │   ├── p2p/         # libp2p networking
│   │   ├── db/          # SQLite repositories
│   │   └── models/      # Data models
│   └── Cargo.toml
├── src/                 # React frontend
│   ├── components/      # Reusable UI components
│   │   ├── icons/       # SVG icon components
│   │   └── layout/      # MainLayout.tsx
│   ├── pages/           # Page components
│   │   ├── Chat.tsx     # Messaging (uses mock peers)
│   │   ├── Wall.tsx     # Personal journal/posts
│   │   ├── Feed.tsx     # Aggregated feed from contacts
│   │   ├── Network.tsx  # P2P network & contacts
│   │   └── Settings.tsx # Profile, security, privacy
│   ├── stores/          # Zustand state management
│   │   ├── identity.ts  # User identity state
│   │   ├── network.ts   # Network connection state
│   │   ├── settings.ts  # User settings (persisted)
│   │   └── mockPeers.ts # Mock peer data for testing
│   └── services/        # Frontend API services
├── mock-peer/           # Standalone mock peer server
│   ├── src/
│   │   ├── main.rs      # Mock peer server entry point
│   │   └── protocols.rs # Protocol type definitions
│   ├── Cargo.toml
│   └── README.md
└── package.json
```

## Implementation Status

### Completed (Phases 1-7 + UI Polish)
- Identity system with Ed25519 keypairs and encrypted storage
- P2P networking with libp2p (mDNS, Kademlia, NAT traversal)
- SQLite database with event-sourced schema
- Full UI implementation with dark theme design system
- All pages functional: Chat, Wall (Journal), Feed, Network, Settings

### Key Features Implemented
1. **Identity**: Create/unlock with passphrase, Ed25519 + X25519 keys
2. **Networking**: Start/stop network, peer discovery, connection status
3. **Messaging**: Conversations with mock auto-replies for testing
4. **Journal (Wall)**: Create posts, like, share functionality
5. **Feed**: Chronological aggregation from all mock peers
6. **Settings**: Profile editing, security (passphrase change, backup/recovery, delete account), network settings, privacy controls

## Mock System for Testing

### Mock Peers (`src/stores/mockPeers.ts`)
6 mock peers with unique identities and wall posts:
- Alice Chen (online) - Full-stack developer
- Bob Wilson (online) - Systems engineer
- Carol Davis (offline) - UX designer
- David Miller (online) - Privacy advocate
- Eva Martinez (online) - Cryptography researcher
- Frank Johnson (offline) - DevOps engineer

### Auto-Reply System
- Online peers automatically reply to messages (1-3 second delay)
- Contextual responses based on keywords (hello, harbor, p2p, thanks, etc.)
- Offline peers don't respond

### Feed Aggregation
- `getAllFeedPosts()` collects posts from all mock peers
- Sorted chronologically (most recent first)
- Each post includes author info and avatar gradient

## UI Design System
CSS custom properties in `src/index.css`:
- `--harbor-bg-primary`, `--harbor-bg-elevated`
- `--harbor-text-primary`, `--harbor-text-secondary`, `--harbor-text-tertiary`
- `--harbor-primary`, `--harbor-accent`, `--harbor-success`, `--harbor-error`, `--harbor-warning`
- `--harbor-border-subtle`, `--harbor-surface-1`, `--harbor-surface-2`

## Key Files to Know

### Frontend
- `src/stores/mockPeers.ts` - Mock peer data, conversations, auto-reply logic
- `src/stores/identity.ts` - Identity state management
- `src/pages/Chat.tsx` - Messaging with mock peers
- `src/pages/Feed.tsx` - Aggregated feed from mock peers
- `src/pages/Settings.tsx` - All settings with modals for delete/recovery
- `src/components/layout/MainLayout.tsx` - Sidebar navigation

### Backend (Rust)
- `src-tauri/src/commands/network.rs` - Network start/stop commands
- `src-tauri/src/p2p/network.rs` - NetworkService with libp2p swarm
- `src-tauri/src/services/identity.rs` - Identity management

## Recent Changes (Latest Session)
1. Added human-friendly peer names to Network page (adjective + animal)
2. Implemented account recovery (export/import backup) and delete account flows
3. Created mock peers store with 6 peers and their walls
4. Feed now aggregates posts from all mock peers chronologically
5. Chat now uses mock peers store with auto-reply for online peers
6. Renamed "My Wall" to "Journal", "Lock Wallet" to "Lock Account"
7. Fixed network state management error (Arc<IdentityService>)

### UI Improvements (Current Session)
8. Added react-hot-toast for styled toast notifications (replaced all alert() calls)
9. Implemented like limit (one per post) for Wall and Feed
10. Implemented image and video upload for Wall posts
11. Wall posts now persist in Zustand store
12. Feed save functionality persists in Zustand store
13. Fixed Network page: "P2P" renamed to "Peer-to-Peer", fixed NaN stats display
14. Implemented Settings functionality:
    - Profile photo upload
    - Copy unique ID with toast feedback
    - Save profile changes
    - Passphrase change (simulated)
    - Export backup (creates JSON file)
    - Recover account with explanation modal
    - Delete account with confirmation
15. Online status toggle now affects profile indicator in sidebar
16. Created settings store with persistence (localStorage)

## Mock Peer Server (for P2P Testing)

The `mock-peer/` directory contains a standalone Rust binary that implements the same libp2p protocols as Harbor. Use this to test real P2P connectivity.

### Building and Running
```bash
cd mock-peer
cargo build --release
cargo run --release -- --name "Test Peer" --bio "For testing" --port 9000
```

### What It Does
- Announces itself via mDNS on the local network
- Responds to identity exchange requests with a signed identity
- Acknowledges incoming messages
- Logs auto-reply content (doesn't actually send replies back yet)

### Protocols Implemented
- `/harbor/identity/1.0.0` - Identity exchange
- `/harbor/messaging/1.0.0` - Messaging

## Known Issues / Future Work
- Network stats show 0s when network starts (need to populate from actual Rust backend)
- Mock peer server logs replies but doesn't send them back through the messaging protocol yet

## Commands
```bash
# Development
npm run dev          # Start Vite dev server
npm run tauri dev    # Start Tauri app in dev mode

# Build
npm run build        # Build frontend
npm run tauri build  # Build full app

# Type check
npx tsc --noEmit

# Mock peer server (for P2P testing)
cd mock-peer && cargo run --release -- --name "Mock Peer"
```

## Plan File Location
Detailed implementation plan: `C:\Users\richa\.claude\plans\cached-sparking-backus.md`
