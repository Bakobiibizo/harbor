---
ldgr_doc: 1
kind: ticket
id: ticket.live-0808-community-preferences
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0808-community-preferences]
tags: [harbor, community, preferences, filters]
---

# Persist per-community landing and modality preferences

## Objective

Persist a preferred landing topic, thread sort, and `All/Images/Video/Audio` discovery filter for
each community ID while keeping Feed/personal-wall and other communities independent.

```ldgr-contract yaml
title: "Persist per-community landing and modality preferences"
description: "Add profile-scoped per-community defaults using Harbor's canonical modality classification."
requirements:
- id: req.01
  text: "Preferences key by community_id, survive restart, and never follow a reused relay locator or collide across profiles."
  evidence_required: true
- id: req.02
  text: "Filtering uses canonical primary modality for thread discovery while a selected thread retains text replies needed for context."
  evidence_required: true
- id: req.03
  text: "Missing or removed preferred topics fall back deterministically to the manifest default without erasing other community preferences."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not reuse the linked Feed/personal-wall socialView as community state."
tests:
- id: test.01
  scenario: "Store/UI tests cover two communities, profile isolation, restart, mixed media, removed-topic fallback, and Feed independence."
  required: true
expected_artifacts:
- "versioned preference schema and migration"
- "forum filter/default controls and tests"
```

## Acceptance criteria

- [ ] Each community opens in that identity's preferred forum view.
- [ ] Community changes never alter Feed/personal-wall selection.
- [ ] Mixed-media threads appear in exactly one media filter plus All.
