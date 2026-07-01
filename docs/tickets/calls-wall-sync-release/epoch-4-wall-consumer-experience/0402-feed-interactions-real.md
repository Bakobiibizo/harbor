---
ldgr_doc: 1
kind: ticket
id: ticket.0402-feed-interactions-real
schema: ldgr.ticket.v1
status: ready
produces:
- work:0402-feed-interactions-real
tags:
- harbor
- release-completion
- feed
- wall
- ui
---

# Replace feed placeholder interactions with durable behavior

**Slug:** `0402-feed-interactions-real`

**Epoch:** Epoch 4 — Wall Consumer Experience

## Objective

Implement real Feed like/reaction, save, hide, and snooze behavior in place of “coming soon” toasts.

```ldgr-contract yaml
title: "Replace feed placeholder interactions with durable behavior"
description: "Implement real Feed like/reaction, save, hide, and snooze behavior in place of “coming soon” toasts."
requirements:
- id: req.01
  text: "Feed reactions use the signed social event model and update counts/user state after reload and sync."
  evidence_required: true
- id: req.02
  text: "Saved posts persist locally, populate the existing Saved tab, and survive app restart without duplicating posts."
  evidence_required: true
- id: req.03
  text: "Hide post and snooze contact preferences persist locally, filter feed results deterministically, and provide reversible management controls."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not keep placeholder toasts for shipped Feed actions."
- id: con.02
  text: "Do not sync local save/hide/snooze preferences to other peers unless explicitly added to the privacy contract."
- id: con.03
  text: "Do not hide posts by deleting synced source data."
tests:
- id: test.01
  scenario: "Feed store/component tests cover reaction toggle, saved tab, hide, snooze expiry, undo/manage controls, and reload persistence."
  required: true
- id: test.02
  scenario: "Cargo/SQLite tests cover any new local preference repositories or migrations."
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

- [ ] req.01: Feed reactions use the signed social event model and update counts/user state after reload and sync.
- [ ] req.02: Saved posts persist locally, populate the existing Saved tab, and survive app restart without duplicating posts.
- [ ] req.03: Hide post and snooze contact preferences persist locally, filter feed results deterministically, and provide reversible management controls.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Cross-device syncing of saved/hidden/snoozed preferences.
- Recommendation/ranking algorithms.
