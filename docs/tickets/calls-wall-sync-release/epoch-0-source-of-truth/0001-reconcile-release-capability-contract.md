---
ldgr_doc: 1
kind: ticket
id: ticket.0001-reconcile-release-capability-contract
schema: ldgr.ticket.v1
status: ready
produces:
- work:0001-reconcile-release-capability-contract
tags:
- harbor
- release-completion
- source-of-truth
---

# Reconcile Harbor release capability contract

**Slug:** `0001-reconcile-release-capability-contract`

**Epoch:** Epoch 0 — Source of Truth and Release Contract

## Objective

Create a single release capability contract that resolves doc/source drift for calling, wall, feed, permissions, relay sync, and mock/demo behavior before implementation work branches.

```ldgr-contract yaml
title: "Reconcile Harbor release capability contract"
description: "Create a single release capability contract that resolves doc/source drift for calling, wall, feed, permissions, relay sync, and mock/demo behavior before implementation work branches."
requirements:
- id: req.01
  text: "A durable spec document enumerates production-required behavior for 1:1 voice, 1:1 video, optional group video, wall host views, wall consumer views, wall sync, and release blockers."
  evidence_required: true
- id: req.02
  text: "The spec explicitly maps each currently unfinished or placeholder behavior to a ticket in this decomposition or records a deliberate out-of-scope decision with rationale."
  evidence_required: true
- id: req.03
  text: "README, SECURITY, and CHANGELOG references that contradict the selected release contract are identified for later correction without silently changing feature scope."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not mark simulated, mock-only, or local-only behavior as production-complete."
- id: con.02
  text: "Do not redefine Harbor as a greenfield system; use existing source and docs as baseline evidence."
- id: con.03
  text: "Do not resolve group-video topology by assumption; record the required decision if still open."
tests:
- id: test.01
  scenario: "Manual review confirms every source-spec section maps to at least one ready ticket or explicit exclusion."
  required: true
- id: test.02
  scenario: "ldgr status shows no conflicting active Harbor conduct work before recording planning artifacts."
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

## Implementation Notes

Evidence inspected includes `README.md`, `CHANGELOG.md`, `docs/release-readiness-review-2026-07-01.md`, `src-tauri/src/services/calling_service.rs`, `src/services/calling.ts`, `src-tauri/src/p2p/behaviour.rs`, `src/pages/Feed.tsx`, `src/pages/Wall.tsx`, `src/stores/wall.ts`, `src/stores/feed.ts`, `src-tauri/src/commands/wall_sync.rs`, and `relay-server/src/board_service.rs`.

## Acceptance Criteria

- [ ] req.01: A durable spec document enumerates production-required behavior for 1:1 voice, 1:1 video, optional group video, wall host views, wall consumer views, wall sync, and release blockers.
- [ ] req.02: The spec explicitly maps each currently unfinished or placeholder behavior to a ticket in this decomposition or records a deliberate out-of-scope decision with rationale.
- [ ] req.03: README, SECURITY, and CHANGELOG references that contradict the selected release contract are identified for later correction without silently changing feature scope.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Implementing runtime calling, wall, or sync code.
- Editing release docs beyond identifying required changes.
