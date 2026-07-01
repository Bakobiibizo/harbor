---
ldgr_doc: 1
kind: ticket
id: ticket.0505-wall-sync-status-observability
schema: ldgr.ticket.v1
status: ready
produces:
- work:0505-wall-sync-status-observability
tags:
- harbor
- release-completion
- wall
- observability
---

# Expose wall sync status and observability

**Slug:** `0505-wall-sync-status-observability`

**Epoch:** Epoch 5 — Wall Sync Integrity

## Objective

Make wall/feed synchronization status visible and diagnosable for users and operators.

```ldgr-contract yaml
title: "Expose wall sync status and observability"
description: "Make wall/feed synchronization status visible and diagnosable for users and operators."
requirements:
- id: req.01
  text: "Network/backend emits structured events for wall sync start, author requested, posts/events stored, media queued/fetched/failed, permission denied, cursor advanced, and relay unavailable states."
  evidence_required: true
- id: req.02
  text: "Frontend feed/contact-wall/author-wall UI shows last sync time, in-progress state, partial failure state, and retry actions without blocking local-first usage."
  evidence_required: true
- id: req.03
  text: "Logs redact private content while preserving peer/post IDs or hashes needed to diagnose sync failures."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not log raw private keys, passphrases, raw media bytes, or full contacts-only post content in production logs."
- id: con.02
  text: "Do not turn best-effort relay failures into data-loss errors for locally saved posts."
- id: con.03
  text: "Do not flood users with repeated identical toasts during background polling."
tests:
- id: test.01
  scenario: "Frontend tests cover sync status rendering and retry actions for success, partial failure, permission denied, and offline relay."
  required: true
- id: test.02
  scenario: "Cargo tests or tracing assertions cover structured event emission for key sync outcomes."
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

- [ ] req.01: Network/backend emits structured events for wall sync start, author requested, posts/events stored, media queued/fetched/failed, permission denied, cursor advanced, and relay unavailable states.
- [ ] req.02: Frontend feed/contact-wall/author-wall UI shows last sync time, in-progress state, partial failure state, and retry actions without blocking local-first usage.
- [ ] req.03: Logs redact private content while preserving peer/post IDs or hashes needed to diagnose sync failures.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- External telemetry/analytics service integration.
- Crash reporting infrastructure.
