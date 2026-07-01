---
ldgr_doc: 1
kind: ticket
id: ticket.0204-group-call-media-layout-runtime
schema: ldgr.ticket.v1
status: ready
produces:
- work:0204-group-call-media-layout-runtime
tags:
- harbor
- release-completion
- calling
- group
- ui
---

# Implement group call media runtime and UI

**Slug:** `0204-group-call-media-layout-runtime`

**Epoch:** Epoch 2 — Video and Group Calling

## Objective

Deliver production group voice/video media handling and participant UI according to the selected topology.

```ldgr-contract yaml
title: "Implement group call media runtime and UI"
description: "Deliver production group voice/video media handling and participant UI according to the selected topology."
requirements:
- id: req.01
  text: "The runtime establishes the required peer connections or server sessions for each participant and enforces the selected participant limit with clear errors before overload."
  evidence_required: true
- id: req.02
  text: "The UI renders participant tiles, names, mute/camera state, active speaker or selected layout, join/leave controls, and degraded media states without breaking one-to-one calls."
  evidence_required: true
- id: req.03
  text: "Failures such as one participant disconnecting, media permission denial, or partial ICE failure are isolated so remaining participants can continue when the topology supports it."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not ship a group UI that only simulates participants or loops local media."
- id: con.02
  text: "Do not exceed browser/network limits without enforced caps or topology-specific infrastructure."
- id: con.03
  text: "Preserve accessibility basics for controls and status announcements."
tests:
- id: test.01
  scenario: "Frontend tests cover roster rendering, join/leave, per-participant mute/camera state, participant limit enforcement, and partial failure UI."
  required: true
- id: test.02
  scenario: "A multi-profile validation call demonstrates at least the participant count required by the selected topology contract."
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

- [ ] req.01: The runtime establishes the required peer connections or server sessions for each participant and enforces the selected participant limit with clear errors before overload.
- [ ] req.02: The UI renders participant tiles, names, mute/camera state, active speaker or selected layout, join/leave controls, and degraded media states without breaking one-to-one calls.
- [ ] req.03: Failures such as one participant disconnecting, media permission denial, or partial ICE failure are isolated so remaining participants can continue when the topology supports it.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Screen sharing unless added to the release contract.
- Recording calls.
- Mobile-specific call UI.
