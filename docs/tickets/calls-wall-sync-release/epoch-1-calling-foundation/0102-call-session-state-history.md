---
ldgr_doc: 1
kind: ticket
id: ticket.0102-call-session-state-history
schema: ldgr.ticket.v1
status: ready
produces:
- work:0102-call-session-state-history
tags:
- harbor
- release-completion
- calling
- database
---

# Persist call session state and history

**Slug:** `0102-call-session-state-history`

**Epoch:** Epoch 1 — Calling Foundation and Voice

## Objective

Turn the existing unused call state/history concepts into a durable state machine for active and completed calls.

```ldgr-contract yaml
title: "Persist call session state and history"
description: "Turn the existing unused call state/history concepts into a durable state machine for active and completed calls."
requirements:
- id: req.01
  text: "SQLite schema/repository/service code records call lifecycle state, peer IDs, direction, media kind, start/end timestamps, duration, and terminal reason without losing existing `call_history` data."
  evidence_required: true
- id: req.02
  text: "The call state machine rejects invalid transitions such as answering an ended call, double-answering, hanging up unknown calls, and simultaneous duplicate incoming sessions unless explicitly handled as busy."
  evidence_required: true
- id: req.03
  text: "Frontend-accessible commands or store state expose active calls and call history consistently after app restart."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not store raw SDP, ICE candidates, microphone audio, or video frames in call history."
- id: con.02
  text: "Do not make in-memory state the sole source of active-call truth when backend events can arrive asynchronously."
- id: con.03
  text: "Preserve migration compatibility for existing user databases."
tests:
- id: test.01
  scenario: "Cargo migration/repository/service tests cover upgrades, valid transitions, invalid transitions, and restart/history retrieval."
  required: true
- id: test.02
  scenario: "Frontend store tests cover active call updates from backend events and initial hydration."
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

- [ ] req.01: SQLite schema/repository/service code records call lifecycle state, peer IDs, direction, media kind, start/end timestamps, duration, and terminal reason without losing existing `call_history` data.
- [ ] req.02: The call state machine rejects invalid transitions such as answering an ended call, double-answering, hanging up unknown calls, and simultaneous duplicate incoming sessions unless explicitly handled as busy.
- [ ] req.03: Frontend-accessible commands or store state expose active calls and call history consistently after app restart.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- WebRTC media transport setup.
- Group participant roster persistence beyond fields needed by later group tickets.
