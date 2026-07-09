---
ldgr_doc: 1
kind: ticket
id: ticket.0203-group-call-signaling-membership
schema: ldgr.ticket.v1
status: validation_pending
produces:
- work:0203-group-call-signaling-membership
tags:
- harbor
- release-completion
- calling
- group
---

# Implement group-call signaling and membership control

**Slug:** `0203-group-call-signaling-membership`

**Epoch:** Epoch 2 — Video and Group Calling

## Objective

Add signed group-call room, invite, join, leave, roster, and media-intent signaling consistent with the selected topology.

```ldgr-contract yaml
title: "Implement group-call signaling and membership control"
description: "Add signed group-call room, invite, join, leave, roster, and media-intent signaling consistent with the selected topology."
requirements:
- id: req.01
  text: "Group-call signaling messages identify room/session IDs, creator, invited participants, active roster, media modes, and per-participant state with signatures and replay protection."
  evidence_required: true
- id: req.02
  text: "Joining a group call requires contact/call capability checks for invited participants and rejects unauthorized or stale join attempts with auditable errors."
  evidence_required: true
- id: req.03
  text: "Roster changes are delivered to all affected participants and persisted enough to recover/terminate active sessions after frontend refresh or backend restart."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not allow arbitrary peers to join by guessing a room ID."
- id: con.02
  text: "Do not use chat text messages as hidden signaling payloads."
- id: con.03
  text: "Honor the topology contract selected by `0201-group-call-topology-contract`."
tests:
- id: test.01
  scenario: "Cargo protocol/service tests cover invite, join, leave, unauthorized join, stale invite, duplicate participant, and room termination cases."
  required: true
- id: test.02
  scenario: "A three-profile scenario proves roster events propagate between participants."
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

- [x] req.01: Group-call signaling messages identify room/session IDs, creator, invited participants, active roster, media modes, and per-participant state with signatures and replay protection.
- [x] req.02: Joining a group call requires contact/call capability checks for invited participants and rejects unauthorized or stale join attempts with auditable errors.
- [x] req.03: Roster changes are delivered to all affected participants and persisted enough to recover/terminate active sessions after frontend refresh or backend restart.

## Implementation Status (2026-07-09)

Implemented in the current working tree:

- signed `group_membership` invite/join/leave/roster/terminate envelopes over the existing libp2p signaling protocol;
- canonical roster ordering, four-participant enforcement, topology/media binding, nonce replay protection, stale-version rejection, and creator/member authorization;
- durable group room and nonce storage through schema migration 015, including active-room hydration after restart;
- explicit incoming group-call acceptance before media capture;
- deterministic missing-leg creation so accepted remote participants form a mesh instead of remaining in a caller-centered star;
- creator termination, participant leave signaling, degraded-participant isolation, and persisted room recovery.

Automated evidence: 233 Rust tests, 363 frontend tests, strict Rust clippy, ESLint, TypeScript, and production frontend build pass. Interactive three/four-profile evidence remains required by `test.02`; use `/opt/webkitgtk-2.52.3-webrtc-gtk3` for the Tauri-compatible WebKit2GTK 4.1 runtime.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Video tile rendering.
- Media forwarding/SFU deployment beyond signaling hooks required by the selected topology.
