---
ldgr_doc: 1
kind: ticket
id: ticket.0201-group-call-topology-contract
schema: ldgr.ticket.v1
status: ready
produces:
- work:0201-group-call-topology-contract
tags:
- harbor
- release-completion
- calling
- architecture
---

# Select and document production group-call topology

**Slug:** `0201-group-call-topology-contract`

**Epoch:** Epoch 2 — Video and Group Calling

## Objective

Resolve the group video architecture ambiguity by committing to a production topology and compatibility contract before implementing group runtime behavior.

```ldgr-contract yaml
title: "Select and document production group-call topology"
description: "Resolve the group video architecture ambiguity by committing to a production topology and compatibility contract before implementing group runtime behavior."
requirements:
- id: req.01
  text: "The repository contains an ADR/spec choosing the first production group-call topology (for example small-group mesh, SFU, or relay-assisted hybrid) with participant limits, NAT expectations, privacy/security tradeoffs, and operational requirements."
  evidence_required: true
- id: req.02
  text: "Downstream signaling, runtime, UI, validation, and deployment requirements are updated to reference the selected topology and no longer rely on ambiguous “preferably group-capable” language."
  evidence_required: true
- id: req.03
  text: "If the selected topology requires external infrastructure, deployment/secrets/update responsibilities and release blockers are explicitly defined."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not silently assume mesh, SFU, MCU, or centralized signaling without this recorded decision."
- id: con.02
  text: "Do not weaken one-to-one call security/permission guarantees to support group calls."
- id: con.03
  text: "Do not implement a demo-only group mode that cannot be validated in production conditions."
tests:
- id: test.01
  scenario: "Manual review confirms every group-call implementation ticket references the selected topology and participant limits."
  required: true
- id: test.02
  scenario: "If infrastructure is required, an operator can identify the deployment path and credentials policy from the ADR/spec."
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

This ticket exists because group-capable video is a major architectural choice. It prevents agents from accidentally building the wrong topology.

## Acceptance Criteria

- [ ] req.01: The repository contains an ADR/spec choosing the first production group-call topology (for example small-group mesh, SFU, or relay-assisted hybrid) with participant limits, NAT expectations, privacy/security tradeoffs, and operational requirements.
- [ ] req.02: Downstream signaling, runtime, UI, validation, and deployment requirements are updated to reference the selected topology and no longer rely on ambiguous “preferably group-capable” language.
- [ ] req.03: If the selected topology requires external infrastructure, deployment/secrets/update responsibilities and release blockers are explicitly defined.

## Validation Guidance

- Prefer `dev check` and `dev ci --language typescript` after cross-stack changes.
- Use focused Vitest/Cargo tests for the touched frontend store/service and Rust service/command/protocol modules.
- For network behavior, validate with two Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) and a relay when the ticket changes relay paths.

## Out of Scope

- Implementing the chosen group-call runtime.
- Changing one-to-one voice behavior except to preserve compatibility.
