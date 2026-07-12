---
ldgr_doc: 1
kind: ticket
id: ticket.0403-legacy-profile-migration
schema: ldgr.ticket.v1
status: ready
produces: [work:0403-legacy-profile-migration]
tags: [harbor, identity, migration]
---

# Migrate legacy profiles safely

## Objective

Require existing identities to claim a relay name without losing their peer ID, keys, contacts, or signed history.

## Acceptance criteria

- [ ] Preserve old names only as explicitly unverified local migration hints.
- [ ] Block identity-dependent publishing until registration completes or the user knowingly remains on beta compatibility mode.
- [ ] Provide collision recovery and rollback-safe database migration.

## Validation

Upgrade tests cover existing data, collisions, cancellation, offline startup, retry, and successful claim attachment.
