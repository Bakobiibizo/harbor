---
ldgr_doc: 1
kind: ticket
id: ticket.0106-voice-call-integration-validation
schema: ldgr.ticket.v1
status: ready
produces:
- work:0106-voice-call-integration-validation
tags:
- harbor
- release-completion
- calling
- validation
---

# Validate end-to-end 1:1 voice calling

**Slug:** `0106-voice-call-integration-validation`

**Epoch:** Epoch 1 — Calling Foundation and Voice

## Objective

Create repeatable validation for the full one-to-one voice call path across two Harbor profiles.

```ldgr-contract yaml
title: "Validate end-to-end 1:1 voice calling"
description: "Create repeatable validation for the full one-to-one voice call path across two Harbor profiles."
requirements:
- id: req.01
  text: "A documented test scenario starts two isolated profiles, grants call capability, connects peers, places a voice call, accepts it, exchanges ICE, verifies connected state, and hangs up cleanly."
  evidence_required: true
- id: req.02
  text: "Automated or semi-automated validation captures logs/events proving signaling transport, session state, frontend runtime, and UI states all participated in the call."
  evidence_required: true
- id: req.03
  text: "Regression coverage prevents closing voice work when only local command signing tests pass without network and WebRTC execution."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not require real user secrets, production relay credentials, or uncontrolled public infrastructure for the validation path."
- id: con.02
  text: "Do not accept a mock-only or single-process-only validation as evidence of production call readiness."
- id: con.03
  text: "Keep validation runnable by future agents/operators from repository instructions."
tests:
- id: test.01
  scenario: "Run the new focused voice-call validation instructions and record observed events/logs."
  required: true
- id: test.02
  scenario: "Run `dev check` and `dev ci --language typescript` after call implementation tickets land."
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

- [ ] req.01: A documented test scenario starts two isolated profiles, grants call capability, connects peers, places a voice call, accepts it, exchanges ICE, verifies connected state, and hangs up cleanly.
- [ ] req.02: Automated or semi-automated validation captures logs/events proving signaling transport, session state, frontend runtime, and UI states all participated in the call.
- [ ] req.03: Regression coverage prevents closing voice work when only local command signing tests pass without network and WebRTC execution.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Group call validation.
- Video device/performance validation.
