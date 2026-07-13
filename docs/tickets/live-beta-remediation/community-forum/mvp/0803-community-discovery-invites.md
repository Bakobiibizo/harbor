---
ldgr_doc: 1
kind: ticket
id: ticket.live-0803-community-discovery-invites
schema: ldgr.ticket.v1
status: ready
produces: [work:live-0803-community-discovery-invites]
tags: [harbor, community, invite, identity]
---

# Implement verified community discovery and invitations

## Objective

Create, share, normalize, and verify Harbor/HTTPS community invitation bundles without a global
directory or manual scheme editing.

```ldgr-contract yaml
title: "Implement verified community discovery and invitations"
description: "Verify relay-scoped manifests and signed locators from Harbor, HTTPS, QR, and clipboard invitation paths."
requirements:
- id: req.01
  text: "Invitation verification binds qualified address, community_id, relay attestation, pinned relay key, and unexpired locator before Join is enabled."
  evidence_required: true
- id: req.02
  text: "The join confirmation explains public content, host-visible membership, signed-post participation, and absence of a public member directory."
  evidence_required: true
- id: req.03
  text: "Malformed, oversized, expired, substituted-manifest, wrong-relay, and unknown-scheme invitations fail safely with retryable human errors."
  evidence_required: true
constraints:
- id: con.01
  text: "Do not add a mandatory central directory or treat an open-community link as bearer authorization."
tests:
- id: test.01
  scenario: "Browser handoff and packaged-app tests cover Harbor/HTTPS equivalence, offline verification, tampering, and expired locators."
  required: true
expected_artifacts:
- "invitation codec and verification service"
- "web handoff and join confirmation UI"
- "negative security tests"
```

## Acceptance criteria

- [ ] Valid invitations resolve to one verified community identity.
- [ ] Invalid invites never create cached membership or dial an unverified locator.
- [ ] Users can share a stable community link without exposing private keys.
