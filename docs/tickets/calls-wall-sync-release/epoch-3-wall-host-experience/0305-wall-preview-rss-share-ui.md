---
ldgr_doc: 1
kind: ticket
id: ticket.0305-wall-preview-rss-share-ui
schema: ldgr.ticket.v1
status: ready
produces:
- work:0305-wall-preview-rss-share-ui
tags:
- harbor
- release-completion
- wall
- rss
- ui
---

# Expose wall preview, RSS, and share surfaces

**Slug:** `0305-wall-preview-rss-share-ui`

**Epoch:** Epoch 3 — Wall Host Experience

## Objective

Wire existing wall preview/RSS backend capabilities into production UI and sharing flows for wall authors.

```ldgr-contract yaml
title: "Expose wall preview, RSS, and share surfaces"
description: "Wire existing wall preview/RSS backend capabilities into production UI and sharing flows for wall authors."
requirements:
- id: req.01
  text: "Author UI provides guest/contact/owner wall preview using production backend commands and explains exactly which posts each perspective can see."
  evidence_required: true
- id: req.02
  text: "Public RSS feed generation and shareable feed/contact links are available through UI actions with copy/export feedback and no contacts-only leakage."
  evidence_required: true
- id: req.03
  text: "Preview/RSS/share paths are documented and covered by tests so docs no longer reference hidden backend-only features."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not expose private key material or encrypted backups through share/export actions."
- id: con.02
  text: "Do not represent RSS as a network-hosted URL unless a real serving path exists."
- id: con.03
  text: "Do not duplicate wall rendering logic in a way that diverges from feed/wall components."
tests:
- id: test.01
  scenario: "Frontend tests cover preview mode selection, RSS generation/copy, and public-only filtering."
  required: true
- id: test.02
  scenario: "Manual validation confirms generated RSS contains only public posts."
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

- [ ] req.01: Author UI provides guest/contact/owner wall preview using production backend commands and explains exactly which posts each perspective can see.
- [ ] req.02: Public RSS feed generation and shareable feed/contact links are available through UI actions with copy/export feedback and no contacts-only leakage.
- [ ] req.03: Preview/RSS/share paths are documented and covered by tests so docs no longer reference hidden backend-only features.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Hosting RSS over HTTP.
- Importing external RSS feeds.
