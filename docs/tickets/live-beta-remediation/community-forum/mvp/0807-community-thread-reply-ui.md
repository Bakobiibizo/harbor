---
ldgr_doc: 1
kind: ticket
id: ticket.live-0807-community-thread-reply-ui
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0807-community-thread-reply-ui]
tags: [harbor, community, forum, compose, accessibility]
---

# Build community thread and reply workflows

## Objective

Implement thread detail, new-thread composition, one-level reply presentation, reply composition,
edits/tombstones, follow state, and durable submission/retry feedback on the forum shell.

```ldgr-contract yaml
title: "Build community thread and reply workflows"
description: "Add accessible thread/reply reading and composition with explicit durable sync states."
requirements:
- id: req.01
  text: "Thread creation requires a topic, title, and body or media; reply actions preserve the explicit thread/parent relationship and author identity."
  evidence_required: true
- id: req.02
  text: "Reply data may preserve deeper parents while MVP rendering uses at most one indentation level and keeps author tombstones when context requires them."
  evidence_required: true
- id: req.03
  text: "Compose exposes local draft, queued, submitted, confirmed, failed, retained-draft, and retry states and never offers private visibility in a public MVP community."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not reuse wall comments or flatten replies into chronological posts."
tests:
- id: test.01
  scenario: "Component/accessibility tests cover title validation, media, focus, keyboard use, one-level replies, edits/tombstones, follows, failure, and retained drafts."
  required: true
- id: test.02
  scenario: "Two packaged profiles create and reply to a thread, restart, and retain correct parent, author, and sync state."
  required: true
expected_artifacts:
- "thread detail and compose components"
- "reply/edit/tombstone UI"
- "accessible interaction and packaged-flow tests"
```

## Acceptance criteria

- [ ] A member can start a coherent titled discussion and another member can reply.
- [ ] Visual nesting remains readable without losing signed parent relationships.
- [ ] Failed or offline submissions retain recoverable drafts and honest state.
