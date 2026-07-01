---
ldgr_doc: 1
kind: ticket
id: ticket.0501-relay-wall-permission-enforcement
schema: ldgr.ticket.v1
status: ready
produces:
- work:0501-relay-wall-permission-enforcement
tags:
- harbor
- release-completion
- wall
- relay
- security
---

# Enforce wall visibility and permissions through relay sync

**Slug:** `0501-relay-wall-permission-enforcement`

**Epoch:** Epoch 5 — Wall Sync Integrity

## Objective

Fix relay-backed wall reads so contacts-only posts are not retrievable without author-granted WallRead capability evidence.

```ldgr-contract yaml
title: "Enforce wall visibility and permissions through relay sync"
description: "Fix relay-backed wall reads so contacts-only posts are not retrievable without author-granted WallRead capability evidence."
requirements:
- id: req.01
  text: "Relay GetWallPosts requests include verifiable authorization evidence or relay-stored grant state sufficient to decide public vs contacts-only access per author/requester."
  evidence_required: true
- id: req.02
  text: "Relay returns public posts to unauthenticated/unauthorized readers only when public access is allowed, and returns contacts-only posts only to valid WallRead subjects."
  evidence_required: true
- id: req.03
  text: "Client feed/contact-wall sync refuses or quarantines relay posts that violate local permission expectations or fail author signature verification."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not rely solely on “requester is a registered peer” for contacts-only wall access."
- id: con.02
  text: "Do not expose an author’s contacts-only posts to all relay users."
- id: con.03
  text: "Do not weaken direct P2P content-sync permission checks."
tests:
- id: test.01
  scenario: "Relay tests cover public reader, authorized WallRead reader, unauthorized registered reader, revoked grant, and malformed grant cases."
  required: true
- id: test.02
  scenario: "Two-profile plus relay validation proves unauthorized contacts-only relay fetches fail while public fetches succeed."
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

- [ ] req.01: Relay GetWallPosts requests include verifiable authorization evidence or relay-stored grant state sufficient to decide public vs contacts-only access per author/requester.
- [ ] req.02: Relay returns public posts to unauthenticated/unauthorized readers only when public access is allowed, and returns contacts-only posts only to valid WallRead subjects.
- [ ] req.03: Client feed/contact-wall sync refuses or quarantines relay posts that violate local permission expectations or fail author signature verification.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- End-to-end encrypted wall content beyond existing visibility model.
- Community board moderation permissions.
