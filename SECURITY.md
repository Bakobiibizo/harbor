# Security Policy

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 1.3.x   | :white_check_mark: |
| < 1.3   | :x: |

## Calls, Relay, and Wall Sync Security Notes

- Call signaling is signed with the local Ed25519 identity and transported through Harbor/libp2p paths. WebRTC media remains DTLS-SRTP encrypted between peers.
- Harbor/libp2p relays are not WebRTC TURN/SFU/MCU media relays. Strict/symmetric NAT media connectivity requires explicit operator-configured TURN; static TURN credentials must not be committed or shipped in client binaries.
- Group-call claims are limited by ADR-0001 to a 4-participant relay-assisted mesh. Larger rooms, SFU/MCU routing, recording, and screen sharing require new implementation, validation, and security review before release claims.
- Wall sync must enforce signed visibility and `WallRead` grants for contacts-only posts across direct and relay paths. Mock peers/walls are demo fixtures and are not security validation evidence.
- Release readiness requires the automated gates plus the multi-profile validation artifacts referenced from `README.md`; do not treat unit tests alone as proof of production call or wall-sync security.

## Known Vulnerabilities

### Linux-Only: glib (Medium Severity)

**Advisory:** [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) / [GHSA-wrw7-89jp-8q8g](https://github.com/advisories/GHSA-wrw7-89jp-8q8g)

**Affected:** glib 0.18.5 (Linux builds only)

**Status:** Waiting for upstream fix

**Details:**
This vulnerability exists in Tauri's Linux GUI dependencies (gtk → glib). The GTK3 bindings are unmaintained, and Tauri is working on migrating to GTK4 ([tauri-apps/tauri#12562](https://github.com/tauri-apps/tauri/issues/12562)).

**Impact:** The vulnerable code path (`VariantStrIter`) is not directly used by Harbor. The practical risk is low, but we're tracking this until Tauri migrates to GTK4.

**Mitigation:** Windows and macOS builds are not affected.

---

### All Platforms: lru (Low Severity)

**Advisory:** [RUSTSEC-2024-0372](https://rustsec.org/advisories/RUSTSEC-2024-0372.html)

**Affected:** lru 0.12.5 (transitive dependency from libp2p)

**Status:** Waiting for libp2p upstream fix

**Details:**
The `IterMut` implementation violates Stacked Borrows rules. This is a soundness issue that only manifests when running under Miri (Rust's undefined behavior detector), not in production code.

**Impact:** Extremely low. This is a theoretical soundness issue, not an exploitable vulnerability.

**Mitigation:** None required for production use.

---

## Fixed Vulnerabilities

| Advisory | Package | Severity | Fixed In |
|----------|---------|----------|----------|
| RUSTSEC-2024-0472 | ring | Medium | libp2p 0.56 (ring 0.17.14) |

## Reporting a Vulnerability

If you discover a security vulnerability in Harbor, please report it by:

1. **DO NOT** create a public GitHub issue
2. Email the maintainers directly or use GitHub's private vulnerability reporting
3. Include steps to reproduce if possible

We aim to respond within 48 hours and will work with you to understand and address the issue.
