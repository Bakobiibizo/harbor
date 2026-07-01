---
ldgr_doc: 1
kind: ticket
id: ticket.0504-wall-social-event-model
schema: ldgr.ticket.v1
status: ready
produces:
- work:0504-wall-social-event-model
tags:
- harbor
- release-completion
- wall
- social
- sync
---

# Implement signed wall social event model

**Slug:** `0504-wall-social-event-model`

**Epoch:** Epoch 5 — Wall Sync Integrity

## Objective

Promote local comments and likes into signed, syncable wall social events with authorization, persistence, and conflict semantics.

```ldgr-contract yaml
title: "Implement signed wall social event model"
description: "Promote local comments and likes into signed, syncable wall social events with authorization, persistence, and conflict semantics."
requirements:
- id: req.01
  text: "Comment create/delete and reaction add/remove events have signed canonical payloads, migrations/repositories, and service validation for author identity and target post authorization."
  evidence_required: true
- id: req.02
  text: "Direct P2P and relay sync carry social events to post authors and authorized consumers without duplicating or forging events."
  evidence_required: true
- id: req.03
  text: "Existing local `post_comments` and `post_likes` data is migrated or bridged into the new event model without losing local user state."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not keep comments/likes as local-only UI state for production social behavior."
- id: con.02
  text: "Do not allow a peer to comment/react to a post they cannot read."
- id: con.03
  text: "Do not expose contacts-only post social metadata to unauthorized relay readers."
tests:
- id: test.01
  scenario: "Cargo migration/service/protocol tests cover event signing, verification, idempotency, deletion, authorization, relay/direct roundtrips, and legacy data handling."
  required: true
- id: test.02
  scenario: "Frontend tests using service wrappers confirm accurate counts after reload once social events are stored."
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

- [ ] req.01: Comment create/delete and reaction add/remove events have signed canonical payloads, migrations/repositories, and service validation for author identity and target post authorization.
- [ ] req.02: Direct P2P and relay sync carry social events to post authors and authorized consumers without duplicating or forging events.
- [ ] req.03: Existing local `post_comments` and `post_likes` data is migrated or bridged into the new event model without losing local user state.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Emoji reaction taxonomy beyond existing like or a small explicitly documented set.
- Threaded/nested comments.
