---
ldgr_doc: 1
kind: ticket
id: ticket.live-0904-community-recovery-forks
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0904-community-recovery-forks]
tags: [harbor, community, recovery, governance, deferred]
---

# Implement governance key recovery and explicit community forks

## Objective

Define threshold-signed steward key rotation and an honest frozen/fork path when recovery quorum is
lost, without silently reassigning the community name or authority.

```ldgr-contract yaml
title: "Implement governance key recovery and explicit community forks"
description: "Add quorum key rotation and signed fork ancestry while preserving immutable identity and preventing silent takeover."
requirements:
- id: req.01
  text: "Recovery rotates steward keys only under the current threshold and a strictly newer revision, with delay/notice and replay protection."
  evidence_required: true
- id: req.02
  text: "Loss of quorum yields a verifiable governance-frozen state; it does not grant the relay, oldest member, or name claimant replacement authority."
  evidence_required: true
- id: req.03
  text: "A fork receives a new qualified name and community ID and carries a signed ancestry reference that UI never presents as continuity of the original."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not silently reuse retired names, keys, community IDs, or moderator grants."
tests:
- id: test.01
  scenario: "Lost/minority/compromised-key, stale rotation, host takeover, frozen state, and explicit fork tests pass."
  required: true
expected_artifacts:
- "accepted recovery/fork protocol extension"
- "rotation and frozen-state UI"
- "security and recovery validation"
```

## Acceptance criteria

- [ ] Recoverable quorum can rotate compromised/lost steward keys.
- [ ] Lost quorum fails safely and visibly.
- [ ] Fork ancestry is useful without enabling impersonation.
