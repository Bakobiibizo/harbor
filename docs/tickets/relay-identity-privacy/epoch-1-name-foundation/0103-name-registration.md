---
ldgr_doc: 1
kind: ticket
id: ticket.0103-name-registration
schema: ldgr.ticket.v1
status: ready
produces: [work:0103-name-registration]
tags: [harbor, identity, relay]
---

# Implement relay name registration

## Objective

Register a unique name by countersigning a user-signed request whose key derives the supplied peer ID.

## Acceptance criteria

- [ ] Add authenticated relay registration API and client command/service.
- [ ] Verify normalization, peer-ID derivation, signature, nonce, timestamps, availability, and sequence.
- [ ] Reserve atomically and never silently reassign retired names.

## Validation

Tests cover successful registration, collisions, races, replay, forged keys, stale requests, and retired names.
