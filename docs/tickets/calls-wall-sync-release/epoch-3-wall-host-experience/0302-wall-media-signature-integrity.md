---
ldgr_doc: 1
kind: ticket
id: ticket.0302-wall-media-signature-integrity
schema: ldgr.ticket.v1
status: ready
produces:
- work:0302-wall-media-signature-integrity
tags:
- harbor
- release-completion
- wall
- media
- security
---

# Bind wall media to signed post integrity

**Slug:** `0302-wall-media-signature-integrity`

**Epoch:** Epoch 3 — Wall Host Experience

## Objective

Close the current media integrity gap by ensuring wall media metadata is signed, synced, and verified for images, videos, and supported audio.

```ldgr-contract yaml
title: "Bind wall media to signed post integrity"
description: "Close the current media integrity gap by ensuring wall media metadata is signed, synced, and verified for images, videos, and supported audio."
requirements:
- id: req.01
  text: "Post creation/update flow binds ordered media hashes and metadata to the signed post or a signed media attachment event before remote consumers accept/render them."
  evidence_required: true
- id: req.02
  text: "Direct P2P and relay wall sync carry image, video, and supported audio metadata consistently instead of image-only relay metadata."
  evidence_required: true
- id: req.03
  text: "Consumers reject tampered media metadata, wrong hashes, unsupported mime types, and oversized media with clear errors while preserving the text post when appropriate."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not keep signing posts with an empty `media_hashes` list when media is intended as part of the post."
- id: con.02
  text: "Do not store media blobs directly in SQLite."
- id: con.03
  text: "Do not weaken existing content-addressed storage or MIME validation."
tests:
- id: test.01
  scenario: "Cargo tests cover post/media signing and verification, tampered media rejection, and relay/direct metadata roundtrips."
  required: true
- id: test.02
  scenario: "Frontend tests cover creating posts with image/video media and rendering after reload/sync."
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

- [ ] req.01: Post creation/update flow binds ordered media hashes and metadata to the signed post or a signed media attachment event before remote consumers accept/render them.
- [ ] req.02: Direct P2P and relay wall sync carry image, video, and supported audio metadata consistently instead of image-only relay metadata.
- [ ] req.03: Consumers reject tampered media metadata, wrong hashes, unsupported mime types, and oversized media with clear errors while preserving the text post when appropriate.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Large-file chunk transfer redesign beyond what is required for post media fetching.
- Call audio/video media transport.
