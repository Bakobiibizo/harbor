---
ldgr_doc: 1
kind: ticket
id: ticket.0402-private-mentions
schema: ldgr.ticket.v1
status: ready
produces: [work:0402-private-mentions]
tags: [harbor, mentions, privacy]
---

# Implement structured private mentions

## Objective

Represent tags as signed structured mentions and privately deliver unresolved mentions through introductions.

## Acceptance criteria

- [ ] Composer resolves known contacts locally and sends private relay envelopes for qualified unknown names.
- [ ] Posts sign typed mention records, claim digests, and authorized peer IDs instead of trusting parsed text.
- [ ] Recipients may accept a notification or repost request without granting broader contact access.

## Validation

Tests cover known/private/unknown targets, tampering, name changes, blocked mentions, bug-report review, and repost consent.
