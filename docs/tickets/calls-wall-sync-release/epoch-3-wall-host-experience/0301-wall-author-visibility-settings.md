---
ldgr_doc: 1
kind: ticket
id: ticket.0301-wall-author-visibility-settings
schema: ldgr.ticket.v1
status: ready
produces:
- work:0301-wall-author-visibility-settings
tags:
- harbor
- release-completion
- wall
- privacy
---

# Implement wall author visibility controls

**Slug:** `0301-wall-author-visibility-settings`

**Epoch:** Epoch 3 — Wall Host Experience

## Objective

Make wall post visibility production-real by wiring settings and per-post controls into backend post creation and previews.

```ldgr-contract yaml
title: "Implement wall author visibility controls"
description: "Make wall post visibility production-real by wiring settings and per-post controls into backend post creation and previews."
requirements:
- id: req.01
  text: "The wall composer uses persisted default visibility from settings and lets the author choose public or contacts-only per post before creation."
  evidence_required: true
- id: req.02
  text: "Backend commands validate visibility, persist it, and expose it consistently through local wall, feed, preview, RSS, direct sync, and relay sync paths."
  evidence_required: true
- id: req.03
  text: "Guest/contact/owner preview counts and rendered post lists match actual permission and visibility behavior for the current local database."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not hard-code new posts to contacts-only when settings specify public."
- id: con.02
  text: "Do not make UI-only visibility changes that fail to persist or sync."
- id: con.03
  text: "Do not expose contacts-only posts through public RSS or guest preview."
tests:
- id: test.01
  scenario: "Wall store/component tests cover default visibility, per-post override, backend payload, preview filtering, and RSS public-only filtering."
  required: true
- id: test.02
  scenario: "Cargo tests cover command validation and repository visibility filters."
  required: true
validation_instructions:
- "Run the narrowest relevant tests for changed components before broad validation."
- "Record validation commands and results as LDGR observations before closure."
- "Do not close on screenshots or self-certification alone when automated tests or multi-profile scenarios are practical."
expected_artifacts:
- "implementation changes or required architecture contract"
- "tests or validation harness updates"
- "validation evidence"
- "LDGR observation summarizing outcome"
```

# Shared Context

Harbor is an existing Tauri/React/Rust/libp2p application. Extend the current identity, permissions, request-response, relay, SQLite, Zustand, and Tauri command architecture rather than replacing it. Current docs say voice signaling exists and video/group calling are future work; source inspection shows calling has signed command/service helpers but no libp2p signaling transport, no WebRTC runtime, and no call UI. Current wall/feed code has local posts, media, comments, likes tables, content sync, and relay wall storage, but several UI actions are placeholder/local-only and relay wall reads do not enforce author grants for contacts-only posts.

## Acceptance Criteria

- [ ] req.01: The wall composer uses persisted default visibility from settings and lets the author choose public or contacts-only per post before creation.
- [ ] req.02: Backend commands validate visibility, persist it, and expose it consistently through local wall, feed, preview, RSS, direct sync, and relay sync paths.
- [ ] req.03: Guest/contact/owner preview counts and rendered post lists match actual permission and visibility behavior for the current local database.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Relay-side permission enforcement for contacts-only posts.
- Private/friends-list scopes beyond existing public/contacts visibility.
