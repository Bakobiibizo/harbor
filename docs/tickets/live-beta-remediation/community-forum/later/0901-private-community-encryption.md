---
ldgr_doc: 1
kind: ticket
id: ticket.live-0901-private-community-encryption
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0901-private-community-encryption]
tags: [harbor, community, privacy, encryption, deferred]
---

# Design private community encryption and key epochs

## Objective

Produce a replacement/extension ADR for invite-only membership and encrypted community content,
including metadata disclosure, history access, removal, epoch rotation, recovery, and multi-device
behavior before any Private label is exposed.

```ldgr-contract yaml
title: "Design private community encryption and key epochs"
description: "Specify and implement scoped invites plus auditable group-content key epochs after the public MVP is validated."
requirements:
- id: req.01
  text: "The design defines invitation authority, group key distribution, epoch rotation, removed-member behavior, forward/backward secrecy goals, and bounded history sharing."
  evidence_required: true
- id: req.02
  text: "Relay and peer metadata leakage, encrypted-cache deletion, backup/recovery, and multi-device key synchronization are threat-modeled and user-visible."
  evidence_required: true
- id: req.03
  text: "Cryptographic test vectors and multi-profile removal/rejoin tests fail closed before Private appears in product copy."
  evidence_required: true
constraints:
- id: con.01
  text: "A secret invitation URL alone is not private-content encryption."
tests:
- id: test.01
  scenario: "Removed and unauthorized profiles cannot decrypt new epochs; replayed or substituted key packages fail."
  required: true
expected_artifacts:
- "accepted private-community ADR"
- "key protocol implementation and vectors"
- "privacy UX and validation evidence"
```

## Acceptance criteria

- [ ] Private has a precise cryptographic and metadata meaning.
- [ ] Membership removal rotates access without deleting verifiable history.
- [ ] Recovery cannot silently grant a host or old member new access.
