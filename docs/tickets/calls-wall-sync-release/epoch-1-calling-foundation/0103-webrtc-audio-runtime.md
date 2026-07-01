---
ldgr_doc: 1
kind: ticket
id: ticket.0103-webrtc-audio-runtime
schema: ldgr.ticket.v1
status: ready
produces:
- work:0103-webrtc-audio-runtime
tags:
- harbor
- release-completion
- calling
- frontend
---

# Implement 1:1 WebRTC audio runtime

**Slug:** `0103-webrtc-audio-runtime`

**Epoch:** Epoch 1 — Calling Foundation and Voice

## Objective

Build the production frontend WebRTC audio engine for one-to-one voice calls using real media devices and the signed signaling transport.

```ldgr-contract yaml
title: "Implement 1:1 WebRTC audio runtime"
description: "Build the production frontend WebRTC audio engine for one-to-one voice calls using real media devices and the signed signaling transport."
requirements:
- id: req.01
  text: "A TypeScript call runtime creates and tears down `RTCPeerConnection` instances, requests microphone permission, attaches local audio tracks, handles remote audio playback, and forwards ICE through Tauri signaling commands."
  evidence_required: true
- id: req.02
  text: "The runtime handles permission denial, missing devices, peer disconnects, ICE failure, duplicate candidates, and call timeout with clear terminal states and cleanup of media tracks."
  evidence_required: true
- id: req.03
  text: "No production call path depends on mock peers, pre-recorded media, fake SDP, or browser-only state that bypasses the Rust permission/signature layer."
  evidence_required: true
constraints:
- id: con.01
  text: "Use browser/Tauri WebRTC APIs already available to the frontend before adding native media dependencies."
- id: con.02
  text: "Do not degrade existing chat media attachment behavior or message encryption."
- id: con.03
  text: "Do not enable video tracks in this ticket except as negotiated absence/compatibility fields."
tests:
- id: test.01
  scenario: "Vitest unit tests with mocked `RTCPeerConnection` and `mediaDevices` cover offer, answer, ICE, hangup, permission-denied, and cleanup paths."
  required: true
- id: test.02
  scenario: "A two-profile validation call confirms remote audio track attachment and call teardown."
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

- [ ] req.01: A TypeScript call runtime creates and tears down `RTCPeerConnection` instances, requests microphone permission, attaches local audio tracks, handles remote audio playback, and forwards ICE through Tauri signaling commands.
- [ ] req.02: The runtime handles permission denial, missing devices, peer disconnects, ICE failure, duplicate candidates, and call timeout with clear terminal states and cleanup of media tracks.
- [ ] req.03: No production call path depends on mock peers, pre-recorded media, fake SDP, or browser-only state that bypasses the Rust permission/signature layer.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Camera/video support.
- Group call fan-out.
- TURN infrastructure deployment.
