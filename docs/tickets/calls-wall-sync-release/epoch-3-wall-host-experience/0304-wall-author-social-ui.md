---
ldgr_doc: 1
kind: ticket
id: ticket.0304-wall-author-social-ui
schema: ldgr.ticket.v1
status: ready
produces:
- work:0304-wall-author-social-ui
tags:
- harbor
- release-completion
- wall
- social
- ui
---

# Show real comments and reactions on author wall

**Slug:** `0304-wall-author-social-ui`

**Epoch:** Epoch 3 — Wall Host Experience

## Objective

Replace author-wall local-only like counts and “Comments coming soon” behavior with real comment/reaction UI backed by signed social events.

```ldgr-contract yaml
title: "Show real comments and reactions on author wall"
description: "Replace author-wall local-only like counts and “Comments coming soon” behavior with real comment/reaction UI backed by signed social events."
requirements:
- id: req.01
  text: "Author wall posts display accurate like/reaction and comment counts loaded from backend state after app reload."
  evidence_required: true
- id: req.02
  text: "Authors can open comment threads on their own posts, add/delete their own comments, and see remote consumer comments/reactions after sync."
  evidence_required: true
- id: req.03
  text: "The UI removes placeholder toasts and local-only counters for comments and likes on the author wall."
  evidence_required: true
constraints:
- id: con.01
  text: "Depend on the signed social event model; do not invent a second local-only comment/reaction store."
- id: con.02
  text: "Do not allow authors to forge comments/reactions as other peers."
- id: con.03
  text: "Keep existing post creation/edit/delete behavior intact."
tests:
- id: test.01
  scenario: "Wall store/component tests cover loading counts, opening threads, adding/deleting comments, toggling reactions, and reload persistence."
  required: true
- id: test.02
  scenario: "Two-profile validation shows a consumer comment/reaction appears on the host wall after sync."
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

- [ ] req.01: Author wall posts display accurate like/reaction and comment counts loaded from backend state after app reload.
- [ ] req.02: Authors can open comment threads on their own posts, add/delete their own comments, and see remote consumer comments/reactions after sync.
- [ ] req.03: The UI removes placeholder toasts and local-only counters for comments and likes on the author wall.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Advanced moderation workflows beyond author delete/hide if not in release contract.
- Community board comments.
