---
ldgr_doc: 1
kind: ticket
id: ticket.live-0811-community-release-validation
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0811-community-release-validation]
tags: [harbor, community, validation, release-gate]
---

# Validate community migration, privacy, and multi-profile behavior

## Objective

Run the accepted legacy treatment and a clean packaged multi-profile community scenario covering
identity, membership, forum use, abuse controls, offline replay, privacy boundaries, and host loss.

```ldgr-contract yaml
title: "Validate community migration, privacy, and multi-profile behavior"
description: "Prove the community forum MVP across packaged profiles, a community relay, restart/offline paths, and negative security cases."
requirements:
- id: req.01
  text: "At least three isolated profiles verify invite/join, qualified names, nonmember read/member post, thread/replies, edits/deletes, filters, local block, and restart."
  evidence_required: true
- id: req.02
  text: "Relay outage/recovery proves cached reading, queued drafts, replay convergence, tombstone safety, and sanitized diagnostics."
  evidence_required: true
- id: req.03
  text: "The accepted legacy policy is exercised without presenting unsigned flat boards as verified communities."
  evidence_required: true
- id: req.04
  text: "Negative tests reject tampered manifests/invites/events, stale membership, unauthorized posts, cross-community parents, replay, and profile leakage."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not claim private communities, moderator governance, replication, or migration beyond the accepted MVP evidence."
tests:
- id: test.01
  scenario: "Automated frontend/Rust/relay suites and a recorded packaged three-profile run pass with sanitized evidence."
  required: true
expected_artifacts:
- "community multi-profile validation checklist and evidence"
- "legacy migration/read-only evidence"
- "release capability contract update"
```

## Acceptance criteria

- [ ] Every MVP claim has automated or packaged-profile evidence.
- [ ] Public membership metadata and host power are accurately documented.
- [ ] Any failed security/privacy scenario blocks release of Communities.
