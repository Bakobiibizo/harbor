---
ldgr_doc: 1
kind: ticket
id: ticket.0303-contact-card-capabilities
schema: ldgr.ticket.v1
status: ready
produces: [work:0303-contact-card-capabilities]
tags: [harbor, privacy, capabilities]
---

# Issue encrypted contact cards and capabilities

## Objective

On approval, issue a user-signed contact card and least-privilege grants encrypted only for the requester.

## Acceptance criteria

- [x] Bind the verified claim, peer ID, public keys, routing, capabilities, revision, expiry, and revocation ID.
- [x] Encrypt with X25519-derived authenticated encryption and reject wrong-recipient or modified cards.
- [x] Map wall, messaging, call, media, and mention permissions onto existing capability enforcement.

## Validation

Cross-profile tests prove approved access works and unapproved, expired, altered, or wrong-peer use fails.
