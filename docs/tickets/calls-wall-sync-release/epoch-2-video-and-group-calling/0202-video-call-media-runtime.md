---
ldgr_doc: 1
kind: ticket
id: ticket.0202-video-call-media-runtime
schema: ldgr.ticket.v1
status: ready
produces:
- work:0202-video-call-media-runtime
tags:
- harbor
- release-completion
- calling
- video
---

# Implement one-to-one video call runtime

**Slug:** `0202-video-call-media-runtime`

**Epoch:** Epoch 2 — Video and Group Calling

## Objective

Extend the calling runtime from audio-only to negotiated one-to-one video with camera controls and audio/video fallback.

```ldgr-contract yaml
title: "Implement one-to-one video call runtime"
description: "Extend the calling runtime from audio-only to negotiated one-to-one video with camera controls and audio/video fallback."
requirements:
- id: req.01
  text: "Call offers/answers negotiate audio-only and audio-video modes, including camera unavailable/disabled fallback without dropping an otherwise valid audio call."
  evidence_required: true
- id: req.02
  text: "The frontend runtime captures local camera tracks, renders local/remote video, supports camera on/off and device switching, and cleans up tracks on hangup or failure."
  evidence_required: true
- id: req.03
  text: "Permissions, errors, and device labels are handled according to browser/Tauri constraints without leaking camera access beyond active calls."
  evidence_required: true
constraints:
- id: con.01
  text: "Build on the production voice runtime and signaling transport instead of duplicating call code."
- id: con.02
  text: "Do not require group-call topology to complete one-to-one video."
- id: con.03
  text: "Do not add screen sharing in this ticket unless the release contract explicitly expands scope."
tests:
- id: test.01
  scenario: "Vitest tests mock media devices and peer connections for video negotiation, camera denial, device switch, fallback, and cleanup."
  required: true
- id: test.02
  scenario: "Two-profile validation confirms a one-to-one video call can connect, render remote video, toggle camera, and hang up."
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

- [ ] req.01: Call offers/answers negotiate audio-only and audio-video modes, including camera unavailable/disabled fallback without dropping an otherwise valid audio call.
- [ ] req.02: The frontend runtime captures local camera tracks, renders local/remote video, supports camera on/off and device switching, and cleans up tracks on hangup or failure.
- [ ] req.03: Permissions, errors, and device labels are handled according to browser/Tauri constraints without leaking camera access beyond active calls.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Group participant layout/fan-out.
- Screen sharing.
- External SFU/TURN deployment.
