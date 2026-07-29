---
ldgr_doc: 1
kind: ticket
id: ticket.live-0810-community-abuse-controls
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0810-community-abuse-controls]
tags: [harbor, community, abuse, moderation]
---

# Add local abuse controls and transparent host actions

## Objective

Add local hide/mute/block/report behavior and separately signed host rate-limit/quarantine actions
without inventing an MVP community moderator or uploading private block lists.

```ldgr-contract yaml
title: "Add local abuse controls and transparent host actions"
description: "Protect users and host infrastructure while preserving the boundary between local choices, hosting policy, and later governance."
requirements:
- id: req.01
  text: "Hide, mute, and block remain profile-local; reports disclose the selected destination/data and minimize copied content."
  evidence_required: true
- id: req.02
  text: "Relay authentication, replay rejection, payload limits, rate limits, and bounded work challenges apply before expensive storage or fanout."
  evidence_required: true
- id: req.03
  text: "Host actions have a distinct signing domain, policy reason/reference, expiry or revision, and UI label and cannot be interpreted as community moderator authority."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not publish block lists, erase signed history on other replicas, or claim appeals/governance exist in MVP."
tests:
- id: test.01
  scenario: "Tests cover local-only controls, report minimization, host-action tampering, expiry, rate-limit bypass attempts, and UI labeling."
  required: true
expected_artifacts:
- "local safety controls"
- "relay host-policy event handling"
- "abuse and privacy tests"
```

## Acceptance criteria

- [ ] A user can stop seeing an abusive participant without disclosing their block graph.
- [ ] Host enforcement is verifiable and honestly labelled.
- [ ] No MVP control implies community-wide ownership or moderation.
