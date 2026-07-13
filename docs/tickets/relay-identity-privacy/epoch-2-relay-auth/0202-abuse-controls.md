---
ldgr_doc: 1
kind: ticket
id: ticket.0202-relay-abuse-controls
schema: ldgr.ticket.v1
status: ready
produces: [work:0202-relay-abuse-controls]
tags: [harbor, relay, abuse]
---

# Implement relay abuse limits and proof of work

## Objective

Rate-limit relay actions and require adaptive, action-bound Hashcash work for unknown introductions.

## Acceptance criteria

- [x] Implement the specified hash preimage, difficulty target, challenge signing, expiry, and single-use storage.
- [x] Apply per-peer, source-network, target, action, and global limits with contact-capability bypass.
- [x] Return generic responses and bounded timing behavior that do not confirm target existence.

## Validation

Deterministic work vectors, replay tests, limit-boundary tests, and enumeration/timing regression tests pass.
