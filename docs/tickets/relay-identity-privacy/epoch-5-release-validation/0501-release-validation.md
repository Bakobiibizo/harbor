---
ldgr_doc: 1
kind: ticket
id: ticket.0501-relay-identity-release-validation
schema: ldgr.ticket.v1
status: ready
produces: [work:0501-relay-identity-release-validation]
tags: [harbor, identity, release-gate]
---

# Validate privacy and identity release gates

## Objective

Prove the complete design with two relays and three identities before Harbor 1.0.

## Acceptance criteria

- [x] Validate registration collisions, relay loss, private introduction, approval, capability use/revocation, mention review, and restart recovery.
- [x] Attempt enumeration, replay, forged claims, relay-key substitution, wrong-recipient decryption, capability theft, and unauthorized content retrieval.
- [x] Update onboarding/operator docs and record reproducible evidence for every security invariant in the source specification.

## Validation

Frontend CI, both Rust workspaces, migration tests, multi-profile automation, and the documented adversarial test matrix all pass. This ticket cannot close on screenshots or self-certification alone.
