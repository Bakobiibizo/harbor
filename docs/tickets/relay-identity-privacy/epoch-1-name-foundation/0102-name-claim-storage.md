---
ldgr_doc: 1
kind: ticket
id: ticket.0102-name-claim-storage
schema: ldgr.ticket.v1
status: ready
produces: [work:0102-name-claim-storage]
tags: [harbor, identity, sqlite]
---

# Persist relay name claims and relay keys

## Objective

Add client and relay persistence for versioned claims, sequence state, relay trust keys, retirement, and expiry.

## Acceptance criteria

- [ ] Add forward-only SQLite migrations and repositories on both client and relay.
- [ ] Enforce one active claim per relay/local-name and monotonic sequences atomically.
- [ ] Store no private user keys or approved-contact lists in relay tables.

## Validation

Migration, uniqueness, rollback, expiry, retirement, and concurrent-registration tests pass.
