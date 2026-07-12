---
ldgr_doc: 1
kind: ticket
id: ticket.0301-introduction-transport
schema: ldgr.ticket.v1
status: ready
produces: [work:0301-introduction-transport]
tags: [harbor, privacy, relay]
---

# Add opaque introduction envelope transport

## Objective

Let an authenticated peer request delivery to a qualified name without receiving the target's peer ID or address.

## Acceptance criteria

- [ ] Add signed, encrypted, expiring introduction envelopes and bounded relay queue storage.
- [ ] Relay validates auth, work, limits, envelope metadata, and replay state but cannot decrypt the message.
- [ ] Unknown, private, offline, blocked, and forwarded cases return indistinguishable generic responses.

## Validation

Two-client relay tests cover delivery, offline queueing, expiry, replay, ciphertext privacy, and non-enumeration.
