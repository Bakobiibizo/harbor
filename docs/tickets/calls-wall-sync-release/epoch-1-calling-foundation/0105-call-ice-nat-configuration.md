---
ldgr_doc: 1
kind: ticket
id: ticket.0105-call-ice-nat-configuration
schema: ldgr.ticket.v1
status: ready
produces:
- work:0105-call-ice-nat-configuration
tags:
- harbor
- release-completion
- calling
- networking
---

# Implement ICE, STUN, and TURN configuration for calls

**Slug:** `0105-call-ice-nat-configuration`

**Epoch:** Epoch 1 — Calling Foundation and Voice

## Objective

Provide production ICE configuration and NAT failure behavior for calls, including operator-configurable STUN/TURN entries when direct/relay connections are insufficient.

```ldgr-contract yaml
title: "Implement ICE, STUN, and TURN configuration for calls"
description: "Provide production ICE configuration and NAT failure behavior for calls, including operator-configurable STUN/TURN entries when direct/relay connections are insufficient."
requirements:
- id: req.01
  text: "Settings or config exposes validated ICE server entries without storing plaintext TURN credentials beyond the selected persistence policy."
  evidence_required: true
- id: req.02
  text: "The WebRTC runtime uses configured ICE servers, surfaces ICE gathering/connection states, and produces actionable errors for strict NAT and relay-only failure modes."
  evidence_required: true
- id: req.03
  text: "Documentation and tests define the default behavior when no TURN server is configured and verify that LAN/direct/relay scenarios continue to work."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not hard-code private TURN credentials or depend on a third-party TURN service without an explicit operator contract."
- id: con.02
  text: "Do not confuse libp2p relay connectivity with WebRTC media relay; document their separate roles."
- id: con.03
  text: "Do not block LAN voice calls when optional TURN configuration is absent."
tests:
- id: test.01
  scenario: "Frontend validation tests cover ICE server parsing, redaction, persistence, and runtime consumption."
  required: true
- id: test.02
  scenario: "Manual validation records at least LAN success and a controlled ICE-failure error path."
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

- [ ] req.01: Settings or config exposes validated ICE server entries without storing plaintext TURN credentials beyond the selected persistence policy.
- [ ] req.02: The WebRTC runtime uses configured ICE servers, surfaces ICE gathering/connection states, and produces actionable errors for strict NAT and relay-only failure modes.
- [ ] req.03: Documentation and tests define the default behavior when no TURN server is configured and verify that LAN/direct/relay scenarios continue to work.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Deploying a TURN service.
- SFU/media-server operation for group calls.
