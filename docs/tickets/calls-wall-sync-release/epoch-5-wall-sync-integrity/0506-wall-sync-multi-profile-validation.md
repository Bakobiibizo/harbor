---
ldgr_doc: 1
kind: ticket
id: ticket.0506-wall-sync-multi-profile-validation
schema: ldgr.ticket.v1
status: ready
produces:
- work:0506-wall-sync-multi-profile-validation
tags:
- harbor
- release-completion
- wall
- validation
---

# Validate host/consumer wall synchronization

**Slug:** `0506-wall-sync-multi-profile-validation`

**Epoch:** Epoch 5 — Wall Sync Integrity

## Objective

Create repeatable end-to-end validation proving host and consumer wall views converge across direct and relay sync.

```ldgr-contract yaml
title: "Validate host/consumer wall synchronization"
description: "Create repeatable end-to-end validation proving host and consumer wall views converge across direct and relay sync."
requirements:
- id: req.01
  text: "Validation scenarios cover host creating public and contacts-only posts with media, consumer authorized/unauthorized views, relay offline availability, edits, deletes, comments, reactions, and media fetch states."
  evidence_required: true
- id: req.02
  text: "The scenarios use real Harbor profiles/databases/network/relay paths rather than mock peer stores or direct repository mutation only."
  evidence_required: true
- id: req.03
  text: "Validation artifacts identify the commands, profiles, relay configuration, observed UI/backend events, and final database/UI state for each scenario."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not accept a single frontend unit test as evidence of wall sync readiness."
- id: con.02
  text: "Do not require production user data or public infrastructure to run local validation."
- id: con.03
  text: "Do not skip negative authorization cases."
tests:
- id: test.01
  scenario: "Run the new multi-profile wall sync validation and record pass/fail observations."
  required: true
- id: test.02
  scenario: "Run `dev check`, `dev ci --language typescript`, and relay server checks after sync implementation lands."
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

- [ ] req.01: Validation scenarios cover host creating public and contacts-only posts with media, consumer authorized/unauthorized views, relay offline availability, edits, deletes, comments, reactions, and media fetch states.
- [ ] req.02: The scenarios use real Harbor profiles/databases/network/relay paths rather than mock peer stores or direct repository mutation only.
- [ ] req.03: Validation artifacts identify the commands, profiles, relay configuration, observed UI/backend events, and final database/UI state for each scenario.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Load testing beyond modest multi-profile local validation.
- Mobile client validation.
