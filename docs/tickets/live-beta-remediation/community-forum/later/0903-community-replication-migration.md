---
ldgr_doc: 1
kind: ticket
id: ticket.live-0903-community-replication-migration
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0903-community-replication-migration]
tags: [harbor, community, replication, migration, deferred]
---

# Implement community replica locators and host migration

## Objective

Allow multiple verified hosts and threshold-approved locator migration without changing the
community ID or allowing a stale/host-signed locator to redirect users silently.

```ldgr-contract yaml
title: "Implement community replica locators and host migration"
description: "Add signed replica sets, deterministic merge/cursor behavior, equivocation evidence, and quorum-approved host changes."
requirements:
- id: req.01
  text: "Replica locator records are versioned, expiring, community-bound, threshold-approved, and separate from relay namespace identity."
  evidence_required: true
- id: req.02
  text: "Clients reconcile verified events across replicas deterministically, preserve tombstones, detect conflicting bytes/equivocation, and avoid double submission."
  evidence_required: true
- id: req.03
  text: "Loss or removal of one host retains community identity, qualified address, cache, and usable remaining replicas."
  evidence_required: true
constraints:
- id: con.01
  text: "A replica cannot mint governance records or rename the community by advertising itself."
tests:
- id: test.01
  scenario: "Multi-relay tests cover outage, stale replica, conflicting response, migration, key rotation, tombstone replay, and restored host."
  required: true
expected_artifacts:
- "replica locator protocol and persistence"
- "multi-replica reconciliation worker"
- "migration runbook and validation"
```

## Acceptance criteria

- [ ] Community identity survives a locator/host change.
- [ ] Conflicting replica data fails visibly and preserves evidence.
- [ ] No host can unilaterally redirect or take ownership.
