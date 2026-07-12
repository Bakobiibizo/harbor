---
ldgr_doc: 1
kind: ticket_index_readme
id: ticket_readme.relay-identity-privacy.v1
schema: ldgr.readme.v1
status: ready
tags: [harbor, identity, privacy, release-gate]
---

# Relay Identity and Private Introductions

Source specification: [`../../architecture/relay-names-and-private-introductions.md`](../../architecture/relay-names-and-private-introductions.md).

This program is a Harbor 1.0 identity-safety release gate. It adds relay-unique names without giving relays private keys, plaintext introductions, content authority, or users' contact lists.

## Work items

| ID | Work item | Depends on |
| --- | --- | --- |
| 0001 | [Freeze protocol encodings and release contract](epoch-0-contract/0001-freeze-protocol-contract.md) | - |
| 0101 | [Add canonical relay-name types and normalization](epoch-1-name-foundation/0101-relay-name-types.md) | 0001 |
| 0102 | [Persist relay name claims and relay keys](epoch-1-name-foundation/0102-name-claim-storage.md) | 0101 |
| 0103 | [Implement relay name registration](epoch-1-name-foundation/0103-name-registration.md) | 0101, 0102 |
| 0104 | [Implement client name-claim verification](epoch-1-name-foundation/0104-name-claim-verification.md) | 0101, 0102 |
| 0201 | [Implement relay challenge-response sessions](epoch-2-relay-auth/0201-relay-authentication.md) | 0001 |
| 0202 | [Implement relay abuse limits and proof of work](epoch-2-relay-auth/0202-abuse-controls.md) | 0201 |
| 0203 | [Implement relay signing-key pinning and rotation](epoch-2-relay-auth/0203-relay-key-rotation.md) | 0102, 0201 |
| 0301 | [Add opaque introduction envelope transport](epoch-3-private-introductions/0301-introduction-transport.md) | 0103, 0202 |
| 0302 | [Add local introduction review and blocking](epoch-3-private-introductions/0302-introduction-review.md) | 0301, 0104 |
| 0303 | [Issue encrypted contact cards and capabilities](epoch-3-private-introductions/0303-contact-card-capabilities.md) | 0302 |
| 0304 | [Implement capability revocation and expiry](epoch-3-private-introductions/0304-capability-revocation.md) | 0303 |
| 0401 | [Replace display names with verified relay names](epoch-4-client-experience/0401-verified-name-ui.md) | 0103, 0104 |
| 0402 | [Implement structured private mentions](epoch-4-client-experience/0402-private-mentions.md) | 0301, 0303, 0401 |
| 0403 | [Migrate legacy profiles safely](epoch-4-client-experience/0403-legacy-profile-migration.md) | 0401 |
| 0501 | [Validate privacy and identity release gates](epoch-5-release-validation/0501-release-validation.md) | all implementation items |

No ticket permits a display-name fallback to appear verified, a public username directory, relay storage of contact graphs, or mock-only completion.

