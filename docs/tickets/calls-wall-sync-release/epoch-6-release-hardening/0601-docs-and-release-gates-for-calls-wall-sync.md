---
ldgr_doc: 1
kind: ticket
id: ticket.0601-docs-and-release-gates-for-calls-wall-sync
schema: ldgr.ticket.v1
status: ready
produces:
- work:0601-docs-and-release-gates-for-calls-wall-sync
tags:
- harbor
- release-completion
- release
- docs
- ci
---

# Update docs and release gates for calls and wall sync

**Slug:** `0601-docs-and-release-gates-for-calls-wall-sync`

**Epoch:** Epoch 6 — Release Hardening

## Objective

Make release documentation and CI gates match the completed production calling and wall-sync behavior.

```ldgr-contract yaml
title: "Update docs and release gates for calls and wall sync"
description: "Make release documentation and CI gates match the completed production calling and wall-sync behavior."
requirements:
- id: req.01
  text: "README, SECURITY, CHANGELOG, relay docs, and any user-facing docs accurately describe implemented voice/video/group-call scope, wall host/consumer behavior, permissions, sync limits, and known limitations."
  evidence_required: true
- id: req.02
  text: "CI/release validation intentionally runs frontend TypeScript, Tauri/Rust, relay checks/tests, and any new multi-profile validation or smoke checks needed for calls/wall sync."
  evidence_required: true
- id: req.03
  text: "Release notes distinguish production behavior from demo/mock behavior and identify any deliberately deferred stretch goals such as screen sharing or mobile support."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not claim video/group calling or wall sync capabilities are supported before validation evidence exists."
- id: con.02
  text: "Do not leave stale “coming soon” UI/docs for features shipped as release-complete."
- id: con.03
  text: "Do not weaken existing release signing/updater requirements."
tests:
- id: test.01
  scenario: "Run final release validation commands and record results."
  required: true
- id: test.02
  scenario: "Manual docs review confirms no stale mock/placeholder language remains for completed features."
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

- [ ] req.01: README, SECURITY, CHANGELOG, relay docs, and any user-facing docs accurately describe implemented voice/video/group-call scope, wall host/consumer behavior, permissions, sync limits, and known limitations.
- [ ] req.02: CI/release validation intentionally runs frontend TypeScript, Tauri/Rust, relay checks/tests, and any new multi-profile validation or smoke checks needed for calls/wall sync.
- [ ] req.03: Release notes distinguish production behavior from demo/mock behavior and identify any deliberately deferred stretch goals such as screen sharing or mobile support.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Implementing missing feature code except small doc/CI wiring needed for release gates.
- Marketing copy outside repository documentation.
