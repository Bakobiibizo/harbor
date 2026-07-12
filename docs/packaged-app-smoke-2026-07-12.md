# Packaged app smoke evidence, 2026-07-12

The Linux x86_64 packaged Harbor binary passed the onboarding recovery and restart smoke against the production `harbor.social` relay.

## Build

- Commit: `c02987123b0069bba004328b97db438280772536`
- Binary SHA-256: `d82fb5a1996e0b9cdff252b420311bccb5664266d19614ed01b823c9f1001b45`
- Platform: Linux x86_64 under WSL2
- Completed: `2026-07-12T18:26:10Z`
- Harness: `scripts/validate-packaged-ux-smoke.sh`

## Relay and identity result

- Namespace: `harbor.social`
- Verified name: `@release-smoke-0712@harbor.social`
- Peer ID: `12D3KooWFSwZqJtNDLQ4WbQ2d3H9oiSpqkBMDvP68RmMAHCLEtGD`
- Migration mode after recovery: `verified`
- Production relay version: `0.2.0`
- Production relay peer ID: `12D3KooWMfwHKfzDrZ2V3Zniw3Qu797bHrKsFKAdG9CtQiaEhbQ3`
- Production relay artifact SHA-256: `b6d3a64b27c818ca67b1d9cccbb8a0629da641b5d10438e93001f751221eba40`

The harness unlocked an interrupted onboarding profile, decoded the stored signed claim through the Tauri DTO boundary, re-verified it without accepting a same-sequence substitution, persisted `verified` migration state, shut the app down, restarted it, unlocked it again, and recovered the same verified qualified name.

Profile persistence was confined to the supplied disposable root, including `harbor.db`, `accounts.json`, media, logs, and WebKit cookies/local storage/cache under `webview/`. The run did not use or alter the default user profile.

## Supporting gates

The same commit passed:

- 407 frontend tests
- 260 Rust client tests plus the CLI test
- 44 relay tests, including the two-relay/three-identity adversarial restart scenario
- strict TypeScript, ESLint, Rust formatting, `cargo check`, and all-target Clippy
- production build namespace validation before and after bundling
- production dependency audit, both Rust dependency-policy audits, and tracked-file credential-pattern scan

Screenshots and machine-readable state snapshots are retained as LDGR artifacts rather than committed application assets.
