---
ldgr_doc: 1
kind: ticket
id: ticket.live-0806-community-forum-ui
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0806-community-forum-ui]
tags: [harbor, community, forum, ui, accessibility]
---

# Build community identity, navigation, and topic discovery UI

## Objective

Implement community cards, verified identity/trust details, topic navigation, thread discovery lists,
host state, and empty/loading/error states without implementing thread composition or replies.

```ldgr-contract yaml
title: "Build community identity, navigation, and topic discovery UI"
description: "Render community identity, trust, topics, and thread discovery as a forum shell rather than another chronological feed."
requirements:
- id: req.01
  text: "Normal surfaces show title plus qualified community and user names; raw peer IDs/keys require an explicit diagnostic action."
  evidence_required: true
- id: req.02
  text: "Topic navigation and thread discovery show titles, reply counts, unread state, stable links, and sync/host state that make the model distinct from Feed."
  evidence_required: true
- id: req.03
  text: "Community information explains manifest verification, relay hosting limits, public membership disclosure, export/leave actions, and diagnostic-ID access."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not show private visibility controls in an MVP public community or label host actions as moderation."
tests:
- id: test.01
  scenario: "Component and accessibility tests cover navigation, names, identity/trust copy, keyboard use, stable links, and empty/loading/error states."
  required: true
- id: test.02
  scenario: "A packaged usability pass confirms participants can find a topic/thread and explain public/host state without diagnostic keys."
  required: true
expected_artifacts:
- "community/topic navigation and trust components"
- "accessible state and interaction tests"
- "community discovery and trust documentation"
```

## Acceptance criteria

- [ ] A user can identify a community, navigate topics, find a thread, and return to unread activity.
- [ ] Host-offline and verification state do not obscure cached community identity.
- [ ] Offline/cached/host-unavailable states are unambiguous.

## Out of scope

- Thread/reply composition and detail UI, covered by `live-0807-community-thread-reply-ui`.
- Per-community defaults, covered by `live-0808-community-preferences`.
