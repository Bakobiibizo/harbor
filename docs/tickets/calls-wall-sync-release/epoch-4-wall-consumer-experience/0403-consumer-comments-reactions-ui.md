---
ldgr_doc: 1
kind: ticket
id: ticket.0403-consumer-comments-reactions-ui
schema: ldgr.ticket.v1
status: ready
produces:
- work:0403-consumer-comments-reactions-ui
tags:
- harbor
- release-completion
- wall
- social
- consumer
---

# Implement consumer comments and reactions on feed/contact walls

**Slug:** `0403-consumer-comments-reactions-ui`

**Epoch:** Epoch 4 — Wall Consumer Experience

## Objective

Let consumers comment on and react to remote wall posts through production signed/synced paths.

```ldgr-contract yaml
title: "Implement consumer comments and reactions on feed/contact walls"
description: "Let consumers comment on and react to remote wall posts through production signed/synced paths."
requirements:
- id: req.01
  text: "Consumers can open comment threads from Feed and contact wall views, add/delete their own comments, and see accurate counts loaded from backend state."
  evidence_required: true
- id: req.02
  text: "Consumer reactions are signed, idempotent per post/user/reaction type, and can be toggled without creating duplicate likes."
  evidence_required: true
- id: req.03
  text: "Remote comments/reactions sync back to authors and other authorized consumers according to the social event visibility policy."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not allow comments/reactions on posts the user is not authorized to read."
- id: con.02
  text: "Do not accept unsigned remote social events."
- id: con.03
  text: "Do not make consumer comment/reaction UI diverge between Feed and contact wall views."
tests:
- id: test.01
  scenario: "Frontend tests cover comment/reaction actions in Feed and contact wall contexts, authorization failures, duplicate toggles, and reload."
  required: true
- id: test.02
  scenario: "Two-profile validation shows consumer social events appear for the author after sync."
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

- [ ] req.01: Consumers can open comment threads from Feed and contact wall views, add/delete their own comments, and see accurate counts loaded from backend state.
- [ ] req.02: Consumer reactions are signed, idempotent per post/user/reaction type, and can be toggled without creating duplicate likes.
- [ ] req.03: Remote comments/reactions sync back to authors and other authorized consumers according to the social event visibility policy.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Moderation beyond author/user delete/hide rules in the social event contract.
- Rich-text comments.
