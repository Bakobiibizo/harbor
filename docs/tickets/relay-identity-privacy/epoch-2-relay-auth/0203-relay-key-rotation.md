---
ldgr_doc: 1
kind: ticket
id: ticket.0203-relay-key-rotation
schema: ldgr.ticket.v1
status: ready
produces: [work:0203-relay-key-rotation]
tags: [harbor, relay, key-rotation]
---

# Implement relay signing-key pinning and rotation

## Objective

Pin relay authority keys and accept replacements only through a verifiable rotation or recovery chain.

## Acceptance criteria

- [ ] Define first-use/predistributed trust, active key IDs, signed successor records, and compromise windows.
- [ ] Persist pins locally and require explicit user action for an unverifiable replacement.
- [ ] Publish operator rotation and emergency recovery procedures without committing private keys.

## Validation

Tests cover ordinary rotation, rollback, unknown replacement, expired keys, and compromise invalidation.
