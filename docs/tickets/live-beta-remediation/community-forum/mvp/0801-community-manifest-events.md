---
ldgr_doc: 1
kind: ticket
id: ticket.live-0801-community-manifest-events
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0801-community-manifest-events]
tags: [harbor, community, protocol, cryptography]
---

# Define community manifest and event encodings

## Objective

Implement versioned canonical-CBOR types and deterministic signing domains for community identity,
relay name attestation, locators, membership, topics, threads, replies, edits, and tombstones.

```ldgr-contract yaml
title: "Define community manifest and event encodings"
description: "Add canonical signed community identity and event schemas with strict limits and golden vectors."
requirements:
- id: req.01
  text: "Community IDs derive from the complete canonical genesis manifest, and community addresses are normalized and relay-scoped."
  evidence_required: true
- id: req.02
  text: "Every durable object has a distinct signing domain, version, field limits, parent/community binding, sequence rules, and expiry rules where applicable."
  evidence_required: true
- id: req.03
  text: "Golden vectors are shared across client and relay, including mutation, cross-domain, stale-sequence, invalid-parent, and relay-key-rotation failures."
  evidence_required: true
constraints:
- id: con.01
  text: "A relay locator or operator label cannot become the community ID or confer a role."
tests:
- id: test.01
  scenario: "Rust golden-vector and malformed-CBOR tests prove deterministic bytes and fail-closed verification."
  required: true
expected_artifacts:
- "protocol types and signing helpers"
- "golden vectors and compatibility tests"
- "version and size-limit documentation"
```

## Acceptance criteria

- [ ] Identity and every MVP event have canonical encodings.
- [ ] User-name claim digests and community IDs are bound into authored events.
- [ ] Protocol limits and replay semantics are explicit and tested.
