---
ldgr_doc: 1
kind: ticket
id: ticket.0101-calling-signaling-transport
schema: ldgr.ticket.v1
status: ready
produces:
- work:0101-calling-signaling-transport
tags:
- harbor
- release-completion
- calling
- p2p
---

# Implement signed calling signaling transport

**Slug:** `0101-calling-signaling-transport`

**Epoch:** Epoch 1 — Calling Foundation and Voice

## Objective

Add a real libp2p/Tauri signaling transport for call offers, answers, ICE candidates, hangups, declines, and busy responses using existing signed calling payloads.

```ldgr-contract yaml
title: "Implement signed calling signaling transport"
description: "Add a real libp2p/Tauri signaling transport for call offers, answers, ICE candidates, hangups, declines, and busy responses using existing signed calling payloads."
requirements:
- id: req.01
  text: "A `/harbor/signaling/1.x` request-response protocol or equivalent existing network command path is registered in `ChatBehaviour` and carries signed offer, answer, ICE, hangup, decline, and busy messages between peers."
  evidence_required: true
- id: req.02
  text: "Incoming signaling verifies contact identity, signatures, timestamps, call permission grants, target peer IDs, and replay/duplicate handling before emitting frontend events."
  evidence_required: true
- id: req.03
  text: "Outgoing Tauri commands both sign and transmit signaling messages to the target peer, returning deterministic errors for offline peers, missing permissions, invalid SDP, and network failures."
  evidence_required: true
constraints:
- id: con.01
  text: "Preserve existing Ed25519 signing helpers and capability semantics; do not bypass `CallingService` validation."
- id: con.02
  text: "Do not route production signaling through mock peers, alerts, local storage, or feed/message text payload hacks."
- id: con.03
  text: "Do not introduce a centralized signaling server unless the release contract is amended."
tests:
- id: test.01
  scenario: "Cargo tests cover valid and invalid signaling requests/responses, permission denials, wrong-recipient payloads, and replay/duplicate handling."
  required: true
- id: test.02
  scenario: "A two-profile manual or automated scenario demonstrates offer->answer->ICE->hangup events crossing the libp2p network."
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

- [ ] req.01: A `/harbor/signaling/1.x` request-response protocol or equivalent existing network command path is registered in `ChatBehaviour` and carries signed offer, answer, ICE, hangup, decline, and busy messages between peers.
- [ ] req.02: Incoming signaling verifies contact identity, signatures, timestamps, call permission grants, target peer IDs, and replay/duplicate handling before emitting frontend events.
- [ ] req.03: Outgoing Tauri commands both sign and transmit signaling messages to the target peer, returning deterministic errors for offline peers, missing permissions, invalid SDP, and network failures.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Capturing microphone/camera streams.
- Rendering the call UI.
- Group-call membership semantics.
