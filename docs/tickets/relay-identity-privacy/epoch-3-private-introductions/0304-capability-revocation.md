---
ldgr_doc: 1
kind: ticket
id: ticket.0304-capability-revocation
schema: ldgr.ticket.v1
status: ready
produces: [work:0304-capability-revocation]
tags: [harbor, privacy, revocation]
---

# Implement capability revocation and expiry

## Objective

Allow issuers to narrow or revoke future access while stating honestly that disclosed content cannot be recalled.

## Acceptance criteria

- [x] Add signed monotonic grant revisions, revocations, expiries, and idempotent sync.
- [x] Enforce revocation at direct and relay content paths before serving new data.
- [x] Surface stale/offline status without claiming already downloaded data was erased.

## Validation

Multi-profile tests cover online/offline revocation, reordering, duplicate events, stale grants, and relay enforcement.
