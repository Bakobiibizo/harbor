---
ldgr_doc: 1
kind: ticket
id: ticket.0201-relay-authentication
schema: ldgr.ticket.v1
status: ready
produces: [work:0201-relay-authentication]
tags: [harbor, relay, authentication]
---

# Implement relay challenge-response sessions

## Objective

Authenticate relay clients with existing Ed25519 identities instead of passwords or social login.

## Acceptance criteria

- [ ] Issue random signed, single-use, action-bound challenges with short expiry.
- [ ] Verify the response signature and peer-ID derivation before issuing a short-lived scoped token.
- [ ] Prevent token replay, audience confusion, privilege expansion, and use after expiry.

## Validation

Tests cover valid sessions, replay, expiry, wrong relay/action/key, token tampering, and concurrent use.
