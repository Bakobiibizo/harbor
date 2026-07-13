---
ldgr_doc: 1
kind: ticket
id: ticket.0302-introduction-review
schema: ldgr.ticket.v1
status: ready
produces: [work:0302-introduction-review]
tags: [harbor, privacy, contacts]
---

# Add local introduction review and blocking

## Objective

Decrypt incoming requests locally and let the target approve, ignore, reject, or block without exposing its decision to the relay.

## Acceptance criteria

- [x] Verify requester key, peer ID, signature, freshness, and envelope binding before display.
- [x] Store decisions and blocks locally; do not upload the approved-contact list.
- [x] Present the full verified qualified requester name in every decision surface.

## Validation

Frontend and Rust tests cover every decision, invalid requests, duplicates, and persistence across restart.
