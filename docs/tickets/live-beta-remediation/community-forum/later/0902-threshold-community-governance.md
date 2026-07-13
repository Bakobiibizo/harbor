---
ldgr_doc: 1
kind: ticket
id: ticket.live-0902-threshold-community-governance
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0902-threshold-community-governance]
tags: [harbor, community, governance, capabilities, deferred]
---

# Implement threshold community roles and governance

## Objective

Add scoped, signed, expiring moderator/steward capabilities and threshold-authorized policy,
topic, role, moderation, and appeal events without an implicit relay-owner role.

```ldgr-contract yaml
title: "Implement threshold community roles and governance"
description: "Introduce revisioned capability grants and quorum governance after the ownerless MVP contract is validated."
requirements:
- id: req.01
  text: "The manifest defines steward keys and threshold; role/policy/steward-set changes require that active threshold and strictly newer revisions."
  evidence_required: true
- id: req.02
  text: "Moderator grants are scoped by action/topic/time, revocable, independently verifiable, and displayed separately from host actions."
  evidence_required: true
- id: req.03
  text: "Moderation records preserve signed source events, support reason/appeal linkage, and converge under reordered replay."
  evidence_required: true
constraints:
- id: con.01
  text: "Relay operation, community creation, or earliest timestamp does not confer governance authority."
tests:
- id: test.01
  scenario: "Threshold, stale revision, revoked role, insufficient quorum, host impersonation, reordered moderation, and appeal tests pass."
  required: true
expected_artifacts:
- "accepted governance protocol extension"
- "capability/quorum services and UI"
- "governance security and convergence tests"
```

## Acceptance criteria

- [ ] No single undeclared owner can mutate community-wide policy.
- [ ] Users can verify why a moderator action applies.
- [ ] Host and governance authority remain cryptographically and visually distinct.
