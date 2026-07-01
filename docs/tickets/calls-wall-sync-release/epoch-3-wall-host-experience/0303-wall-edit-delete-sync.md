---
ldgr_doc: 1
kind: ticket
id: ticket.0303-wall-edit-delete-sync
schema: ldgr.ticket.v1
status: ready
produces:
- work:0303-wall-edit-delete-sync
tags:
- harbor
- release-completion
- wall
- sync
---

# Synchronize wall edits and deletes

**Slug:** `0303-wall-edit-delete-sync`

**Epoch:** Epoch 3 — Wall Host Experience

## Objective

Make author edits and deletes propagate to consumers and relay storage with durable conflict-safe semantics.

```ldgr-contract yaml
title: "Synchronize wall edits and deletes"
description: "Make author edits and deletes propagate to consumers and relay storage with durable conflict-safe semantics."
requirements:
- id: req.01
  text: "Editing a local wall post creates a signed update event with new lamport state, persists locally, updates relay state, and reaches direct-sync consumers."
  evidence_required: true
- id: req.02
  text: "Deleting a local wall post creates a signed tombstone/delete event that removes or marks the post consistently in author UI, feed UI, relay, and already-synced consumers."
  evidence_required: true
- id: req.03
  text: "Consumers reconcile create/update/delete events by author, post ID, lamport clock, and signature without resurrecting deleted posts from stale relay or peer responses."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not make update/delete local-only."
- id: con.02
  text: "Do not hard-delete relay data in a way that prevents consumers from learning about deletes when they already cached a post unless a tombstone path exists."
- id: con.03
  text: "Do not permit non-authors to edit/delete wall posts."
tests:
- id: test.01
  scenario: "Cargo tests cover signed update/delete validation, stale event rejection, and relay response reconciliation."
  required: true
- id: test.02
  scenario: "Two-profile validation shows a consumer sees an author edit and delete after sync/refresh."
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

- [ ] req.01: Editing a local wall post creates a signed update event with new lamport state, persists locally, updates relay state, and reaches direct-sync consumers.
- [ ] req.02: Deleting a local wall post creates a signed tombstone/delete event that removes or marks the post consistently in author UI, feed UI, relay, and already-synced consumers.
- [ ] req.03: Consumers reconcile create/update/delete events by author, post ID, lamport clock, and signature without resurrecting deleted posts from stale relay or peer responses.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Comment/reaction deletes unless covered by social event tickets.
- Moderation controls for community boards.
