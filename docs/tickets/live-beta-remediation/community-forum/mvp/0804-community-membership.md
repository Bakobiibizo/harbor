---
ldgr_doc: 1
kind: ticket
id: ticket.live-0804-community-membership
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0804-community-membership]
tags: [harbor, community, membership, privacy]
---

# Implement explicit open-community membership

## Objective

Replace implicit peer registration with signed, sequenced open-community join and leave events and
enforce reader/member capabilities at every relay and client boundary.

```ldgr-contract yaml
title: "Implement explicit open-community membership"
description: "Add signed CommunityJoin and CommunityLeave records and enforce public-read/member-post behavior."
requirements:
- id: req.01
  text: "Join binds community_id, identity key, verified qualified-name claim digest, sequence, timestamp, and nonce; Leave is a strictly newer tombstone."
  evidence_required: true
- id: req.02
  text: "Only an active member may create content; public readers may sync without being added to a browsable member directory."
  evidence_required: true
- id: req.03
  text: "Leaving revokes posting, isolates pending drafts, and offers explicit keep-cache or delete-cache behavior."
  evidence_required: true
constraints:
- id: con.01
  text: "The host label is not stored or evaluated as a community capability."
tests:
- id: test.01
  scenario: "Two-profile tests cover join, duplicate join, leave, stale join replay, nonmember posting, restart, and profile switching."
  required: true
expected_artifacts:
- "membership service and protocol handling"
- "join/leave persistence and UI states"
- "authorization and privacy tests"
```

## Acceptance criteria

- [ ] Registration proves a key; membership separately authorizes posting.
- [ ] Older joins cannot override a leave.
- [ ] No API exposes a normal browsable member list.
