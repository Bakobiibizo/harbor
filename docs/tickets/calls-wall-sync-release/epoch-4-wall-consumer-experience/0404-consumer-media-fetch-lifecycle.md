---
ldgr_doc: 1
kind: ticket
id: ticket.0404-consumer-media-fetch-lifecycle
schema: ldgr.ticket.v1
status: ready
produces:
- work:0404-consumer-media-fetch-lifecycle
tags:
- harbor
- release-completion
- wall
- media
- consumer
---

# Harden consumer media fetching and rendering lifecycle

**Slug:** `0404-consumer-media-fetch-lifecycle`

**Epoch:** Epoch 4 — Wall Consumer Experience

## Objective

Make remote wall media loading reliable and observable for images, videos, and supported audio across feed and contact wall views.

```ldgr-contract yaml
title: "Harden consumer media fetching and rendering lifecycle"
description: "Make remote wall media loading reliable and observable for images, videos, and supported audio across feed and contact wall views."
requirements:
- id: req.01
  text: "Consumer UI distinguishes pending, fetched, missing, failed, unsupported, and blocked media states without dropping the containing post."
  evidence_required: true
- id: req.02
  text: "Media fetch requests use author/relay metadata safely, validate content hashes and MIME types, and avoid repeated uncontrolled fetch loops for missing media."
  evidence_required: true
- id: req.03
  text: "Image/video/audio rendering is shared across Feed, contact wall, and author wall components where practical to avoid inconsistent behavior."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not trust filenames or MIME types without content/hash validation."
- id: con.02
  text: "Do not block text post sync on media fetch failure."
- id: con.03
  text: "Do not fetch media from peers unrelated to the post author unless a relay/media authority contract exists."
tests:
- id: test.01
  scenario: "Frontend tests cover each media lifecycle state and shared rendering behavior."
  required: true
- id: test.02
  scenario: "Cargo tests cover media fetch validation, hash mismatch rejection, and missing-file retry limits."
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

- [ ] req.01: Consumer UI distinguishes pending, fetched, missing, failed, unsupported, and blocked media states without dropping the containing post.
- [ ] req.02: Media fetch requests use author/relay metadata safely, validate content hashes and MIME types, and avoid repeated uncontrolled fetch loops for missing media.
- [ ] req.03: Image/video/audio rendering is shared across Feed, contact wall, and author wall components where practical to avoid inconsistent behavior.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Large-file streaming UX beyond supported post media.
- Call media transport.
