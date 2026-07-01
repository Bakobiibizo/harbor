---
ldgr_doc: 1
kind: ticket
id: ticket.0104-calling-ui-and-events
schema: ldgr.ticket.v1
status: ready
produces:
- work:0104-calling-ui-and-events
tags:
- harbor
- release-completion
- calling
- ui
---

# Add production call UI and event handling

**Slug:** `0104-calling-ui-and-events`

**Epoch:** Epoch 1 — Calling Foundation and Voice

## Objective

Expose one-to-one voice calling in the chat UI with incoming call notifications, active call controls, and backend event integration.

```ldgr-contract yaml
title: "Add production call UI and event handling"
description: "Expose one-to-one voice calling in the chat UI with incoming call notifications, active call controls, and backend event integration."
requirements:
- id: req.01
  text: "Chat/contact UI surfaces call eligibility, starts outgoing calls only for contacts with call capability, and displays actionable errors for offline or unauthorized peers."
  evidence_required: true
- id: req.02
  text: "Incoming call events show accept, decline, busy, and ignore/timeout affordances without interrupting unrelated conversations or losing queued events while locked/unfocused."
  evidence_required: true
- id: req.03
  text: "Active call UI supports mute/unmute, speaker/remote audio status, hangup, elapsed duration, and post-call status/history refresh."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not use blocking `alert()` or placeholder toasts for production call actions."
- id: con.02
  text: "Do not assume mock peer online state represents real call availability."
- id: con.03
  text: "Keep existing chat send/read/edit flows functional."
tests:
- id: test.01
  scenario: "Component/store tests cover outgoing, incoming, accept, decline, busy, timeout, hangup, and permission-denied UI states."
  required: true
- id: test.02
  scenario: "Manual validation across two profiles confirms incoming call UI appears when the caller starts a call from Chat."
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

- [ ] req.01: Chat/contact UI surfaces call eligibility, starts outgoing calls only for contacts with call capability, and displays actionable errors for offline or unauthorized peers.
- [ ] req.02: Incoming call events show accept, decline, busy, and ignore/timeout affordances without interrupting unrelated conversations or losing queued events while locked/unfocused.
- [ ] req.03: Active call UI supports mute/unmute, speaker/remote audio status, hangup, elapsed duration, and post-call status/history refresh.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Video tiles or camera controls.
- Group-call participant roster UI.
