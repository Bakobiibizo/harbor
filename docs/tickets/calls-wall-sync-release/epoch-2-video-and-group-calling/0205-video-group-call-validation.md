---
ldgr_doc: 1
kind: ticket
id: ticket.0205-video-group-call-validation
schema: ldgr.ticket.v1
status: ready
produces:
- work:0205-video-group-call-validation
tags:
- harbor
- release-completion
- calling
- validation
---

# Validate video and group calling release readiness

**Slug:** `0205-video-group-call-validation`

**Epoch:** Epoch 2 — Video and Group Calling

## Objective

Create repeatable evidence that one-to-one video and selected group calling behavior are production-ready.

```ldgr-contract yaml
title: "Validate video and group calling release readiness"
description: "Create repeatable evidence that one-to-one video and selected group calling behavior are production-ready."
requirements:
- id: req.01
  text: "Validation instructions cover device permission setup, fake-media automation where available, two-profile video calls, and the selected group participant scenario."
  evidence_required: true
- id: req.02
  text: "Observed logs/events prove signaling, roster, media connection, UI controls, and cleanup all execute through production paths."
  evidence_required: true
- id: req.03
  text: "Known topology limitations, participant caps, and NAT/TURN requirements are recorded in release docs and surfaced in UI errors."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not treat successful one-to-one voice validation as sufficient for video/group readiness."
- id: con.02
  text: "Do not require exposing real camera feeds in stored artifacts; use logs/screenshots/redacted observations where needed."
- id: con.03
  text: "Do not close if group support remains “preferable” but unresolved."
tests:
- id: test.01
  scenario: "Run focused video/group validation scenarios and record pass/fail observations."
  required: true
- id: test.02
  scenario: "Run `dev check` and `dev ci --language typescript` after video/group implementation lands."
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

- [ ] req.01: Validation instructions cover device permission setup, fake-media automation where available, two-profile video calls, and the selected group participant scenario.
- [ ] req.02: Observed logs/events prove signaling, roster, media connection, UI controls, and cleanup all execute through production paths.
- [ ] req.03: Known topology limitations, participant caps, and NAT/TURN requirements are recorded in release docs and surfaced in UI errors.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Performance tuning beyond meeting the selected participant limit.
- Third-party interoperability testing outside Harbor clients.
