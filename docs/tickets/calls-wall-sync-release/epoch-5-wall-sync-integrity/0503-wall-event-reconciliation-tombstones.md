---
ldgr_doc: 1
kind: ticket
id: ticket.0503-wall-event-reconciliation-tombstones
schema: ldgr.ticket.v1
status: ready
produces:
- work:0503-wall-event-reconciliation-tombstones
tags:
- harbor
- release-completion
- wall
- sync
- reconciliation
---

# Implement wall event reconciliation and tombstones

**Slug:** `0503-wall-event-reconciliation-tombstones`

**Epoch:** Epoch 5 — Wall Sync Integrity

## Objective

Define and enforce conflict resolution for wall post create/update/delete/media/social events across host and consumer stores.

```ldgr-contract yaml
title: "Implement wall event reconciliation and tombstones"
description: "Define and enforce conflict resolution for wall post create/update/delete/media/social events across host and consumer stores."
requirements:
- id: req.01
  text: "A reconciliation service applies signed events by author, object ID, event type, lamport clock, timestamp tie-breaker, and tombstone rules consistently across direct and relay sources."
  evidence_required: true
- id: req.02
  text: "Stale or conflicting events are rejected or ignored with observable logs, and deleted posts/comments/reactions are not resurrected by older snapshots."
  evidence_required: true
- id: req.03
  text: "Repositories preserve enough tombstone state to synchronize deletes to consumers who were offline during deletion."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not rely only on `created_at` wall-clock ordering for conflict resolution."
- id: con.02
  text: "Do not discard tombstones before all sync paths can observe them unless a retention policy is documented."
- id: con.03
  text: "Do not accept events with invalid author signatures or mismatched author IDs."
tests:
- id: test.01
  scenario: "Cargo tests cover out-of-order create/update/delete, stale relay snapshot, duplicate event, forged author, and tombstone retention cases."
  required: true
- id: test.02
  scenario: "Multi-profile validation shows offline consumer catches up to deletes/edits after reconnect."
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

- [ ] req.01: A reconciliation service applies signed events by author, object ID, event type, lamport clock, timestamp tie-breaker, and tombstone rules consistently across direct and relay sources.
- [ ] req.02: Stale or conflicting events are rejected or ignored with observable logs, and deleted posts/comments/reactions are not resurrected by older snapshots.
- [ ] req.03: Repositories preserve enough tombstone state to synchronize deletes to consumers who were offline during deletion.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Full CRDT rich-text editing.
- Moderation/audit exports.
