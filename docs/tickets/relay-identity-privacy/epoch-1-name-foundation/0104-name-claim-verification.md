---
ldgr_doc: 1
kind: ticket
id: ticket.0104-name-claim-verification
schema: ldgr.ticket.v1
status: ready
produces: [work:0104-name-claim-verification]
tags: [harbor, identity, crypto]
---

# Implement client name-claim verification

## Objective

Verify both user and pinned-relay signatures before any name is presented as verified.

## Acceptance criteria

- [ ] Verify canonical form, peer-ID derivation, both signatures, relay audience, validity window, sequence, and key ID.
- [ ] Cache only verified claims and mark expired, superseded, or untrusted claims distinctly.
- [ ] Expose a typed verification result; callers cannot treat plain strings as verified identities.

## Validation

Golden-vector and mutation tests reject each independently corrupted field and signature.
