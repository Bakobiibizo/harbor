---
ldgr_doc: 1
kind: ticket
id: ticket.live-0809-community-offline-replay
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0809-community-offline-replay]
tags: [harbor, community, offline, sync]
---

# Implement offline queue and deterministic community replay

## Objective

Make cached forums useful offline and reconcile queued signed drafts and remote event pages without
duplicates, resurrection, or cross-profile leakage.

```ldgr-contract yaml
title: "Implement offline queue and deterministic community replay"
description: "Add profile-scoped drafts, submission states, cursor replay, parent recovery, backoff, and tombstone convergence."
requirements:
- id: req.01
  text: "Offline users can browse verified cache and queue drafts whose local, queued, submitted, confirmed, failed, and retry states survive restart."
  evidence_required: true
- id: req.02
  text: "Reconnect replay is incremental, idempotent, cancellable on lock/profile switch, bounded for missing parents, and retains revocations/tombstones."
  evidence_required: true
- id: req.03
  text: "Queued events are revalidated against current membership and manifest state; rejection retains an editable draft."
  evidence_required: true
constraints:
- id: con.01
  text: "A local timestamp alone cannot define authoritative replay order or cursor advancement."
tests:
- id: test.01
  scenario: "Multi-profile tests cover host outage, queued compose, stale cursor, duplicate pages, missing parents, tombstone replay, lock, and profile switch."
  required: true
expected_artifacts:
- "community sync worker and durable queue"
- "reconciliation/status events"
- "offline and stale-replica tests"
```

## Acceptance criteria

- [ ] Host loss yields cached read-only/queued behavior, not data disappearance.
- [ ] Invalid pages do not advance state.
- [ ] Retry never silently duplicates a thread or reply.
