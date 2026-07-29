# Harbor release-readiness documentation review (2026-07-01)

## Scope reviewed

- `README.md`
- `CHANGELOG.md`
- `SECURITY.md`
- `CLAUDE.md`
- `.dev/config.toml` / `.dev/config.linux.toml`
- `.github/workflows/ci.yml` / `.github/workflows/release.yml`
- `infrastructure/README.md`
- `relay-server/README.md`
- `scripts/generate-signing-keys.md`
- Relevant source references for placeholder/mock behavior in `src/`, `src-tauri/`, and `relay-server/`.

## Spec / roadmap locations found

Harbor does not currently have a separate product specification document. The closest current sources of intent are:

1. `README.md` — public architecture, feature list, MVP limitations, and roadmap.
2. `CLAUDE.md` — implementation notes and recent-session context, including mock-peer behavior and known issues.
3. `CHANGELOG.md` — historical feature/release notes, though it is noisy and partly duplicated.
4. `.github/workflows/release.yml` plus `scripts/generate-signing-keys.md` — release/update signing intent.
5. `infrastructure/README.md` and `relay-server/README.md` — relay/community deployment intent.

## Explicit unfinished or deferred features

From `README.md`:

- Double-ratchet / forward secrecy is not implemented.
- TURN server support remains future work.
- Group chats are future work.
- Video calling and screen sharing are future work.
- Read receipts and typing indicators are future work.
- Feed like/comment language is stale: README says like/comment are “when implemented”, while comments exist in backend/store paths and feed likes still show a “coming soon” toast.

From UI/source inspection:

- `src/pages/Feed.tsx` still uses placeholders for likes, saved posts, hiding posts, and snoozing contacts.
- `src/pages/Wall.tsx` still shows “Comments coming soon!” for journal/wall post comments.
- `src/pages/settings/SecuritySection.tsx` simulates password change, account import/recovery, and account deletion. Export uses `ENCRYPTED_KEY_DATA_PLACEHOLDER` rather than a real encrypted identity backup payload.
- `CLAUDE.md` still says chat/feed rely on mock peers for substantial testing/demo behavior.
- `src/hooks/useKeyboardNavigation.ts` marks Ctrl+K quick search as a future feature.

## Release-readiness risks / doc drift

- `src-tauri/tauri.conf.json` still contains `UPDATER_PUBKEY_PLACEHOLDER`; updater signing is not release-ready until replaced with the generated public key.
- Updater endpoint points to `https://github.com/nicholasoxford/harbor/...`, while repository docs and current checkout reference `Bakobiibizo` / `bakobiibizo`. Confirm the canonical release repository before shipping auto-update metadata.
- `SECURITY.md` says only `0.1.x` is supported, while package/app version is `1.3.0`.
- `CHANGELOG.md` contains duplicated/noisy generated history and placeholder release sections such as “Describe the notable changes here.” It should be normalized before release.
- `README.md` appears stale in several labels and feature statements, e.g. “My Wall” vs current Journal terminology and feed/comment status.
- The GitHub CI workflow has separate “frontend” and “backend” jobs, but both currently run `.dev/bin/dev ci` without an explicit language. Because devkit default language is Rust, confirm CI is actually running TypeScript checks in GitHub.
- Relay server clippy with `-D warnings` currently fails on existing warnings, even though relay `cargo check` passes.

## Validation snapshot

Commands run during this review:

- `dev check` at repo root: passed Rust/Tauri fmt, clippy, check, and tests (`186 passed`).
- `dev ci --language typescript`: passed TypeScript typecheck, ESLint, Vitest, and frontend build.
- `cargo check --manifest-path relay-server/Cargo.toml`: passed.
- `cargo clippy --manifest-path relay-server/Cargo.toml -- -D warnings`: failed on existing clippy warnings in relay server and relay smoke binary.
- `ldgr context`: works after LDGR DB migration and shows no pending/running Harbor work.

## Suggested release-prep work order

1. Replace updater placeholder public key and verify the updater endpoint/repository owner.
2. Fix release docs/security docs/version support and normalize `CHANGELOG.md`.
3. Fix GitHub CI so frontend TypeScript CI, Tauri/Rust CI, and relay checks all run intentionally.
4. Resolve relay clippy warnings or adjust the release gate explicitly.
5. Replace simulated security/account flows with real backend-backed password change, encrypted backup export/import, and account deletion, or clearly mark them non-release features.
6. Decide whether mock peers are demo-only or still part of product behavior; document and gate accordingly.
7. Finish or intentionally defer Feed likes/save/hide/snooze and Wall comments.
8. Re-run full release validation: TypeScript CI, Tauri Rust CI, relay check/clippy/tests, Tauri build, and updater artifact verification.
