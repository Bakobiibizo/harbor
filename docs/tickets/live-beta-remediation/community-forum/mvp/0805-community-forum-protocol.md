---
ldgr_doc: 1
kind: ticket
id: ticket.live-0805-community-forum-protocol
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0805-community-forum-protocol]
tags: [harbor, community, forum, p2p]
---

# Implement topic, thread, and reply synchronization

## Objective

Replace the flat board-post protocol with signed topic, thread, reply, edit, and tombstone events
plus bounded cursor pagination.

```ldgr-contract yaml
title: "Implement topic, thread, and reply synchronization"
description: "Carry and verify signed forum event graphs over Harbor's request-response and relay paths."
requirements:
- id: req.01
  text: "Threads require a manifest topic, title, and body or media; replies bind a thread and optional parent reply; every parent belongs to the same community."
  evidence_required: true
- id: req.02
  text: "Create, edit, and author-delete operations verify active membership, signatures, name claims, limits, event sequence, and replay state before persistence."
  evidence_required: true
- id: req.03
  text: "Pagination uses deterministic event ordering, reports has-more/cursor state honestly, and returns tombstones needed to prevent resurrection."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not encode replies as wall comments or infer relationships from text."
tests:
- id: test.01
  scenario: "Cargo protocol/service tests cover valid graphs, missing/cross-community parents, edits, deletes, duplicate events, stale clocks, and size/rate limits."
  required: true
- id: test.02
  scenario: "Two packaged profiles exchange a thread and nested reply through direct and relayed paths."
  required: true
expected_artifacts:
- "versioned forum request-response messages"
- "client/relay services and commands"
- "graph, authorization, and replay tests"
```

## Acceptance criteria

- [ ] Topic/thread/reply identity is explicit and stable.
- [ ] Tampered or unauthorized events never advance a cursor.
- [ ] Deletes remain visible as context-preserving tombstones where replies depend on them.
