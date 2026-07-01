---
ldgr_doc: 1
kind: ticket
id: ticket.0502-wall-sync-cursors-pagination
schema: ldgr.ticket.v1
status: ready
produces:
- work:0502-wall-sync-cursors-pagination
tags:
- harbor
- release-completion
- wall
- sync
- database
---

# Implement durable wall sync cursors and pagination

**Slug:** `0502-wall-sync-cursors-pagination`

**Epoch:** Epoch 5 — Wall Sync Integrity

## Objective

Make wall/feed sync incremental, durable, and paginated for direct peer and relay paths.

```ldgr-contract yaml
title: "Implement durable wall sync cursors and pagination"
description: "Make wall/feed sync incremental, durable, and paginated for direct peer and relay paths."
requirements:
- id: req.01
  text: "Relay feed sync uses stored per-author cursors instead of fetching every contact from lamport zero on each poll/refresh."
  evidence_required: true
- id: req.02
  text: "Direct content sync and relay sync both advance cursors only after verified storage of posts/media metadata/events, with retry behavior for partial failures."
  evidence_required: true
- id: req.03
  text: "Pagination and `has_more` handling continue fetching until limits or user-requested bounds are met without duplicates or skipped lamport ranges."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not regress existing direct content-sync cursor behavior."
- id: con.02
  text: "Do not treat wall media fetch success as required for advancing post text/event cursors unless integrity policy requires it."
- id: con.03
  text: "Do not introduce unbounded background polling."
tests:
- id: test.01
  scenario: "Cargo tests cover cursor persistence, partial failure, duplicate response, gap detection, and relay pagination."
  required: true
- id: test.02
  scenario: "Frontend/store tests cover refresh/load-more behavior without duplicate feed rows."
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

- [ ] req.01: Relay feed sync uses stored per-author cursors instead of fetching every contact from lamport zero on each poll/refresh.
- [ ] req.02: Direct content sync and relay sync both advance cursors only after verified storage of posts/media metadata/events, with retry behavior for partial failures.
- [ ] req.03: Pagination and `has_more` handling continue fetching until limits or user-requested bounds are met without duplicates or skipped lamport ranges.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Search indexing or feed ranking.
- Cross-device sync of local user preferences.
