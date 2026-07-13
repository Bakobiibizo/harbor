---
ldgr_doc: 1
kind: ticket
id: ticket.live-0802-community-event-storage
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0802-community-event-storage]
tags: [harbor, community, sqlite, migration]
---

# Persist community manifests and append-only events

## Objective

Add profile-local and relay storage for verified manifests, locator proofs, membership events,
forum events, tombstones, quarantine records, and replica cursors without mutating legacy board data.

```ldgr-contract yaml
title: "Persist community manifests and append-only events"
description: "Store verified community identity and event logs transactionally with profile isolation and replay-safe indexes."
requirements:
- id: req.01
  text: "Schema keys all content by community_id and preserves signed bytes, verification status, deterministic event order, parent IDs, and tombstones."
  evidence_required: true
- id: req.02
  text: "Cursor advancement and event insertion are atomic; duplicates are idempotent and conflicting bytes for one event ID fail closed."
  evidence_required: true
- id: req.03
  text: "Legacy relay_communities, boards, and board_posts remain untouched pending the accepted migration policy."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not rewrite old flat-board rows into verified community events."
tests:
- id: test.01
  scenario: "Migration, rollback, duplicate, conflicting-event, tombstone, cursor, and isolated-profile tests pass."
  required: true
expected_artifacts:
- "client and relay database migrations"
- "repository APIs and transactional tests"
```

## Acceptance criteria

- [ ] Verified event history survives restart and cannot be resurrected by stale replay.
- [ ] Missing-parent quarantine is bounded and queryable.
- [ ] Profile roots never share community cache or queued drafts.
