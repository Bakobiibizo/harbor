# Harbor Project Audit Report

**Date**: 2026-02-07
**Scope**: Full codebase audit — Rust backend, React frontend, mock-peer, relay-server, bootstrap-node, infrastructure, GitHub issues

---

## Executive Summary

| Category | Critical | High | Medium | Low |
|----------|----------|------|--------|-----|
| Rust Backend | 1 | 3 | 4 | 5 |
| React Frontend | 0 | 2 | 4 | 8 |
| Infrastructure & Config | 3 | 3 | 3 | 4 |
| **Totals** | **4** | **8** | **11** | **17** |

**Open GitHub Issues**: 18 (1 high priority, 12 medium, 5 low)
**Open PRs**: 3 (2 Dependabot, 1 external contribution)

---

## CRITICAL ISSUES

### C1. Missing Signature Verification in Relay Server
- **Files**: `relay-server/src/board_service.rs`, `relay-server/src/main.rs`
- **Issue**: Signatures are accepted and stored but **never verified**. Any peer can impersonate others or submit posts as different authors.
- **Impact**: Any malicious actor can forge posts attributed to other users.

### C2. Missing Signature Verification in Identity Exchange
- **File**: `src-tauri/src/p2p/network.rs:1614-1615`
- **Code**: `// TODO: Verify signature on the response`
- **Issue**: Identity responses from peers are not cryptographically verified, enabling man-in-the-middle attacks.

### C3. Invalid Updater Configuration
- **File**: `src-tauri/tauri.conf.json:38-41`
- **Issue**: Updater pubkey is `"UPDATER_PUBKEY_PLACEHOLDER"` and endpoint references wrong GitHub user (`nicholasoxford/harbor` instead of `bakobiibizo/harbor`).
- **Impact**: Auto-updates completely broken. No build process replaces the placeholder.

### C4. Hardcoded Identity Key in CloudFormation Template
- **File**: `infrastructure/community-relay-cloudformation.yaml:247-250`
- **Issue**: Contains plaintext base64-encoded identity key: `IDENTITY_KEY_B64="CAESQKx8WZyv..."`. Anyone with repo access can extract and impersonate this relay.

---

## HIGH SEVERITY

### H1. Excessive Unwrap Calls (236 total) — Panic Risk
- **Worst offenders**:
  - `src-tauri/src/db/connection.rs` — 43 unwraps
  - `src-tauri/src/db/repositories/likes_repo.rs` — 21 unwraps
  - `src-tauri/src/db/repositories/contacts_repo.rs` — 21 unwraps
  - `src-tauri/src/services/identity_service.rs` — 14 unwraps
- **Impact**: Any mutex poisoning or unexpected None will crash the application.

### H2. Unprocessed Message Acknowledgments
- **File**: `src-tauri/src/p2p/network.rs:1718`
- **Code**: `// TODO: Process acknowledgment (update message status)`
- **Impact**: Users never see message delivery/read status.

### H3. setInterval Leak in Network Page
- **File**: `src/pages/Network.tsx:217`
- **Issue**: `setInterval` for relay connection monitoring has **no cleanup** on component unmount.
- **Impact**: Memory leak; stale intervals keep running.

### H4. Unhandled Promise Rejections in Stores
- **Files**: `src/stores/messaging.ts:101,114`, `src/stores/boards.ts:100,110`
- **Issue**: Async calls without `await` or `.catch()` — failures are silent.

### H5. Infrastructure README References Non-Existent Docker Setup
- **File**: `infrastructure/README.md:66-145`
- **Issue**: Instructions reference `docker logs harbor-relay`, `systemctl status docker`, etc. — neither CloudFormation template uses Docker.

### H6. No Rate Limiting on Relay Server
- **Files**: `relay-server/src/main.rs`, `relay-server/src/board_service.rs`
- **Issue**: No throttling on board list requests, post submissions, or peer registration. Vulnerable to DoS.

### H7. Version Mismatch
- `package.json` → `0.2.0`, `src-tauri/tauri.conf.json` → `0.1.1`
- Sub-crates (mock-peer, bootstrap-node, relay-server) → `0.1.0`

### H8. Error Discrimination via String Matching
- **File**: `src-tauri/src/error.rs:155-184`
- **Issue**: Error codes derived by matching on error message text (`msg.contains("locked")`). Fragile — message changes break error categorization.

---

## MEDIUM SEVERITY

### M1. Missing Content Security Policy
- **File**: `src-tauri/tauri.conf.json:5`
- **Issue**: CSP is explicitly set to `null` — no XSS protection.

### M2. SQL Construction via format!()
- **Files**: `src-tauri/src/db/repositories/likes_repo.rs:177-179`, `bootstrap_repo.rs:149-151`
- **Issue**: Dynamic SQL via `format!()`. Currently safe (hardcoded columns, parameterized values) but fragile pattern.

### M3. Mutex Lock Poisoning Risk
- **File**: `src-tauri/src/db/connection.rs` (multiple locations)
- **Issue**: `self.conn.lock().unwrap()` — poisoned mutex cascades panics.

### M4. Lamport Clock Never Validated
- **File**: `relay-server/src/board_service.rs:48`
- **Issue**: `lamport_clock` accepted but never validated — replay attacks possible.

### M5. Nonce Generation Counter-Based
- **File**: `mock-peer/src/main.rs:196-198`
- **Issue**: AES-GCM nonce from counter with first 4 bytes zero. Counter reset = nonce reuse = cryptographic break.

### M6. Inefficient Query with TODO
- **File**: `src-tauri/src/services/content_sync_service.rs:493`
- **Code**: `// TODO: Add a more efficient query` — fetches 1000 posts, filters in memory.

### M7. SSH Security Group Open to World
- **File**: `infrastructure/libp2p-relay-cloudformation.yaml:137-141`
- **Issue**: Port 22 open to `0.0.0.0/0` regardless of whether a key pair exists.

### M8. Console Logs Throughout Frontend
- **Files**: 15+ files with `console.log/error/warn` — should be dev-only or use structured logger.

### M9. Complex Network Event Loop
- **File**: `src-tauri/src/p2p/network.rs` (~500-2000+ lines)
- **Issue**: Single massive match statement for all P2P events. Hard to maintain.

### M10. Event Handlers Lack Try-Catch
- **File**: `src/hooks/useTauriEvents.ts`
- **Issue**: Network event handlers don't have try-catch — errors crash silently.

### M11. Inconsistent Relay Server DB Path Handling
- **File**: `relay-server/src/main.rs:229-237`
- **Issue**: Different fallback logic for identity key path vs data directory could split data.

---

## LOW SEVERITY / CODE QUALITY

### L1. Dead Code
- `src-tauri/src/services/calling_service.rs:52-53` — unused `db` field with `#[allow(dead_code)]`
- `src-tauri/src/p2p/network.rs:24-28` — empty `FALLBACK_RELAYS` constant

### L2. Clippy Suppressions
- 15+ `#[allow(clippy::too_many_arguments)]` across commands and repos — consider builder pattern / param structs.

### L3. Duplicated Utility Functions (Frontend)
- `getInitials()` — duplicated in Chat.tsx, Wall.tsx, Feed.tsx
- `getContactColor()` — duplicated in Feed.tsx and other pages
- `formatDate()` — duplicated across pages
- `shortPeerId()` — duplicated in Boards.tsx and Network.tsx
- **Fix**: Extract to shared `src/utils/` module.

### L4. Large Files Needing Decomposition
- `src/pages/Settings.tsx` — 1700+ lines, should be split into sub-components
- `src/stores/mockPeers.ts` — 500+ lines of mock data, could be JSON

### L5. Incomplete Keyboard Navigation
- **File**: `src/hooks/useKeyboardNavigation.ts:173-186`
- **Issue**: All keyboard shortcut actions are `() => {}` (empty).

### L6. Feed Tabs Placeholder
- **File**: `src/pages/Feed.tsx:198`
- **Comment**: "Select posts based on active tab (saved tab placeholder for future)"

### L7. Wall Likes Local-Only
- **File**: `src/stores/wall.ts:34,173`
- **Issue**: `likePost()` is local-only, backend doesn't track likes. Hard-coded zeros.

### L8. Wall Media Handling Simplified
- **File**: `src/stores/wall.ts:96-113`
- **Issue**: TODO comment — media handling is "simplified."

### L9. Mock vs Real Data Boundary Undocumented
- Mock peers store (`src/stores/mockPeers.ts`) still used in Feed but removed from Chat contact list. No clear documentation of what's mock vs real.

### L10. No Global Unhandled Promise Rejection Handler
- Frontend has `ErrorBoundary` for React errors but no global handler for async rejections.

### L11. Performance: Recalculated Values
- `src/pages/Chat.tsx:246-254` — `searchResults` recalculated on every render (needs `useMemo`).
- `src/pages/Network.tsx:95-100` — hash function for peer names recalculates each render.

---

## TESTING GAPS

### Backend (Rust)
| Area | Has Tests | Notes |
|------|-----------|-------|
| Database repositories | Yes | Good coverage |
| Protocol codecs | Yes | Encode/decode tested |
| Identity/crypto services | Yes | Core security tested |
| Command handlers | **No** | Most commands untested |
| Messaging service | **No** | Critical path untested |
| Feed service | **No** | |
| Content sync service | **No** | |
| Board service | **No** | |
| Calling service | **No** | |

### Frontend (React)
| Area | Has Tests | Notes |
|------|-----------|-------|
| Identity store | Yes | Basic tests |
| Network store | Yes | Basic tests |
| Pages (6 total) | **No** | Zero page tests |
| Components | **No** | Zero component tests |
| Hooks | **No** | useTauriEvents untested |
| Services | **No** | All 13 untested |

**Coverage**: ~2 test files for 67 source files on frontend. ~19 test files for 58 Rust files on backend (but concentrated on DB/protocol layer).

---

## GITHUB ISSUES SUMMARY

### Open Issues (18)

**High Priority (1)**:
- **#2** — Implement Feed Content Sync Protocol (`/harbor/content/1.0.0`)

**Medium Priority (12)**:
| # | Title |
|---|-------|
| 22 | P2P Discovery & Federation Architecture Research |
| 21 | Feed Share Functionality |
| 18 | Feed Comments System |
| 17 | Wall Content Type Sections with Filters |
| 14 | Message Search Within Conversations |
| 13 | Edit Sent Messages with Peer Sync |
| 12 | CRUD Controls for Conversations |
| 11 | Full Keyboard Navigation Support |
| 9 | Restructure Settings into Sidebar Navigation |
| 7 | Hint Phrase for Account Recovery |
| 6 | Voice Calling with WebRTC |
| 5 | Content-Addressed Media Storage |

**Low Priority (5)**:
| # | Title |
|---|-------|
| 27 | Wall Embedded Preview Cards for Links |
| 26 | Messages GIF Support |
| 25 | Messages Image and Video Upload |
| 24 | Messages Emoji Support |
| 23 | Harbor Logo Customization Menu |

### Open PRs (3)
- **#68** — Dependabot: bump `time` 0.3.45 → 0.3.47 (relay-server)
- **#67** — Dependabot: bump `bytes` 1.11.0 → 1.11.1 (src-tauri)
- **#56** — External: Nix flake for reproducible builds (meta-introspector)

---

## PRIORITIZED RECOMMENDATIONS

### Immediate (Security / Correctness)
1. **Implement signature verification** in relay server (`board_service.rs`) and identity exchange (`network.rs:1614`)
2. **Remove hardcoded identity key** from `community-relay-cloudformation.yaml` — generate at deploy time
3. **Fix updater config** — correct GitHub URL and replace placeholder pubkey or disable updater
4. **Add CSP** to `tauri.conf.json`

### Short-Term (Stability)
5. **Replace `unwrap()` on mutex locks** with `.expect()` or proper error recovery (236 instances)
6. **Fix setInterval leak** in `Network.tsx:217`
7. **Add `.catch()` handlers** to fire-and-forget async calls in stores
8. **Implement message ACK processing** (`network.rs:1718`)
9. **Fix version numbers** — align across all Cargo.toml files and tauri.conf.json
10. **Add rate limiting** to relay server

### Medium-Term (Quality)
11. **Refactor error discrimination** from string matching to typed errors
12. **Add tests** for command handlers, messaging service, and feed service
13. **Extract shared utilities** (getInitials, getContactColor, formatDate, shortPeerId)
14. **Split Settings.tsx** into sub-components
15. **Update infrastructure README** — remove Docker references
16. **Add try-catch** to event handlers in `useTauriEvents.ts`

### Long-Term (Polish)
17. **Add frontend test coverage** — page tests, component tests, hook tests
18. **Break down network event loop** into smaller handler methods
19. **Implement keyboard navigation** (currently empty handlers)
20. **Create production logger** — replace raw console.log calls
21. **Add relay health monitoring** to CloudFormation infrastructure
