---
ldgr_doc: 1
kind: ticket
id: ticket.live-0800-community-contract-approval
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0800-community-contract-approval]
tags: [harbor, community, protocol, decision]
---

# Approve the community protocol contract

## Objective

Resolve ADR-0002's four acceptance questions through stakeholder review and a short wireflow test,
then freeze the MVP/later boundary before implementation begins.

```ldgr-contract yaml
title: "Approve the community protocol contract"
description: "Resolve the community address, public MVP, governance timing, and legacy-board migration decisions and accept ADR-0002."
requirements:
- id: req.01
  text: "Record decisions for canonical address syntax, public/open MVP scope, fixed configuration versus first-release threshold governance, and legacy-board treatment."
  evidence_required: true
- id: req.02
  text: "Walk at least three representative users through discovery, join disclosure, topic/thread/reply use, host-versus-moderator meaning, and offline state using the text wireflow."
  evidence_required: true
- id: req.03
  text: "Update ADR-0002 to Accepted with the chosen answers and amend all downstream tickets before scheduling them."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not schedule protocol or product implementation while ADR-0002 is Proposed."
tests:
- id: test.01
  scenario: "Review evidence shows users can distinguish Community from Feed and correctly explain public membership metadata and host power."
  required: true
expected_artifacts:
- "Accepted ADR-0002 or a superseding ADR"
- "wireflow research notes without personal data"
- "updated community ticket dependency program"
```

## Acceptance criteria

- [ ] All four ADR questions have explicit answers.
- [ ] Wireflow feedback is summarized and reflected in the decision.
- [ ] Downstream tickets match the accepted scope and may then be scheduled.

## Out of scope

- Product or protocol implementation.
- Creating LDGR work items for downstream tickets before acceptance.
