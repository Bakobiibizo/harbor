# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Development
npm run tauri dev          # Start full app (Vite + Tauri)
npm run dev                # Frontend only (no Rust)

# Build
npm run tauri build        # Production build + bundle

# Frontend checks
npx tsc --noEmit           # TypeScript type check
npm test                   # Vitest unit tests (frontend)
npm run test:coverage      # With coverage

# Rust checks (run from src-tauri/)
cargo check                # Fast compile check (no linking)
cargo test                 # Run all Rust unit tests
cargo test <test_name>     # Run a specific test
```

## Architecture

### Request flow

```
React page → services/<domain>.ts (invoke) → Tauri IPC
  → src-tauri/src/commands/<domain>.rs          (thin handler, validates input)
  → src-tauri/src/services/<domain>_service.rs  (business logic)
  → src-tauri/src/db/repositories/<domain>_repo.rs (SQLite via rusqlite)
```

Commands are registered in `src-tauri/src/lib.rs` inside `tauri::generate_handler![]`. Every new command must be added there.

### Real-time events (Rust → Frontend)

The backend emits typed events via `app_handle.emit(event_name, payload)`. All listeners live in one place: `src/hooks/useTauriEvents.ts`, called once at the app root. Use `useStore.getState()` inside event callbacks to avoid stale closures — see the `message_received` handler as the reference pattern.

### State management

Zustand stores in `src/stores/` are the single source of truth for UI state. Stores call `src/services/` functions which call `invoke()`. Stores are also updated directly by event handlers in `useTauriEvents.ts` via `getState()`.

### Identity & lock model

`IdentityService` holds `Arc<RwLock<Option<UnlockedKeys>>>` in memory. When locked, keys are `None` — any service method that needs keys returns `AppError::IdentityLocked`. The frontend routes between `<CreateIdentity>`, `<UnlockIdentity>`, and the main layout based on `useIdentityStore` state status.

### Contact string format

`harbor://<base64url_no_pad_json>` where the JSON is `ContactBundle`:
```
{ multiaddr, display_name, public_key (base64), x25519_public (base64), bio?, avatar_hash? }
```
Peer ID is the last segment of `multiaddr` after `/p2p/`. The base64 uses `URL_SAFE_NO_PAD` (Rust) — in JS, replace `-→+` and `_→/` then add `=` padding before `atob()`. The command `add_contact_from_string` handles a double-base64 key encoding for backward compatibility.

### Permission system

Capabilities (`Chat`, `WallRead`, `Call`) are signed grants stored in `permission_events` (append-only) and materialized into `permissions_current`. When a contact is added via `add_contact_from_string`, `WallRead` and `Chat` are automatically granted.

### Deep-link flow (harbor:// scheme)

`tauri-plugin-deep-link` forwards `harbor://` URLs to `on_open_url` in `lib.rs`. The handler normalises `harbor://add-friend/<base64>` → `harbor://<base64>` and either emits `deep_link_contact` immediately (if identity is unlocked) or queues it in `PendingDeepLink(Mutex<Vec<String>>)`. The queue is drained in `unlock_identity` and `create_identity` commands. The frontend listener in `useTauriEvents` stores the payload in `useNetworkStore.pendingDeepLinkContact`, which triggers `AddContactDialog`.

### Database

SQLite via `rusqlite` (bundled). Schema managed through numbered migrations in `src-tauri/src/db/migrations/`. Uses event sourcing for permissions, messages, and posts — raw events are stored and materialized views are derived. `Database` is managed Tauri state (`Arc<Database>`), shared across all services.

### Multi-account / profiles

Setting `HARBOR_PROFILE=<name>` uses a separate DB subdirectory (`profile-<name>`). Useful for running two instances simultaneously during development.

## UI conventions

- All colours use CSS custom properties: `hsl(var(--harbor-text-primary))`, `hsl(var(--harbor-bg-elevated))`, etc. — defined in `src/styles/design-system.css`. Do not use hardcoded colours.
- Modals follow the pattern in `KeyboardShortcutsModal.tsx`: `fixed inset-0 z-50` overlay, backdrop click to close, `e.stopPropagation()` on the card.
- Toasts use `react-hot-toast`. Success toasts for `contact_added` are fired in `useTauriEvents` — do not duplicate them in components.
- New common components go in `src/components/common/` and must be exported from `src/components/common/index.ts`.
