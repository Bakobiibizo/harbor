---
ldgr_doc: 1
kind: ticket
id: ticket.0401-contact-wall-view
schema: ldgr.ticket.v1
status: ready
produces:
- work:0401-contact-wall-view
tags:
- harbor
- release-completion
- wall
- consumer
- ui
---

# Implement contact wall consumer view

**Slug:** `0401-contact-wall-view`

**Epoch:** Epoch 4 — Wall Consumer Experience

## Objective

Add a production route/component for viewing a specific contact’s wall with permissions, relay/direct sync, media, and pagination.

```ldgr-contract yaml
title: "Implement contact wall consumer view"
description: "Add a production route/component for viewing a specific contact’s wall with permissions, relay/direct sync, media, and pagination."
requirements:
- id: req.01
  text: "Users can open a contact wall from feed, chat, network/contact surfaces, or a deep/share link and see that author’s posts with display name, media, visibility-safe metadata, and pagination."
  evidence_required: true
- id: req.02
  text: "The view triggers direct or relay sync as appropriate, handles missing WallRead grants/public-only access, and surfaces actionable errors without showing unauthorized contacts-only posts."
  evidence_required: true
- id: req.03
  text: "The same underlying backend state powers contact wall and feed views so host/consumer displays converge after sync."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not use mock peer walls for production contact wall rendering."
- id: con.02
  text: "Do not bypass `get_wall`/sync authorization checks in the UI."
- id: con.03
  text: "Do not fetch all contacts from relay on every single contact wall open when a targeted cursor is available."
tests:
- id: test.01
  scenario: "Frontend route/store tests cover loading, pagination, permission denied, relay sync success/failure, and media rendering."
  required: true
- id: test.02
  scenario: "Two-profile validation confirms a consumer can view an authorized host wall and cannot view contacts-only posts without grant."
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

- [ ] req.01: Users can open a contact wall from feed, chat, network/contact surfaces, or a deep/share link and see that author’s posts with display name, media, visibility-safe metadata, and pagination.
- [ ] req.02: The view triggers direct or relay sync as appropriate, handles missing WallRead grants/public-only access, and surfaces actionable errors without showing unauthorized contacts-only posts.
- [ ] req.03: The same underlying backend state powers contact wall and feed views so host/consumer displays converge after sync.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Editing another user’s posts.
- Community board wall replacement.
