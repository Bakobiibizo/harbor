---
ldgr_doc: 1
kind: ticket
id: ticket.0001-freeze-relay-identity-protocol
schema: ldgr.ticket.v1
status: ready
produces: [work:0001-freeze-relay-identity-protocol]
tags: [harbor, identity, protocol]
---

# Freeze relay identity protocol encodings

## Objective

Turn the source specification into versioned canonical-CBOR schemas and protocol identifiers before persistence or transport lands.

## Acceptance criteria

- [ ] Define signed schemas for name requests/claims, challenges, introductions, contact cards, grants, revocations, mentions, and relay-key rotations.
- [ ] Specify signature domains, field limits, time units, sequence rules, and deterministic test vectors.
- [ ] Add the Harbor 1.0 release-gate contract and explicitly record deferred features.

## Validation

Rust tests decode and verify shared golden vectors; malformed, reordered, oversized, and cross-domain records fail.
