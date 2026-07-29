---
ldgr_doc: 1
kind: ticket_index_readme
id: ticket_readme.live-community-forum.v1
schema: ldgr.readme.v1
status: ready
tags: [harbor, community, forum, protocol]
---

# Community forum implementation program

Source decision: [`../../../architecture/adr-0002-community-forum-identity.md`](../../../architecture/adr-0002-community-forum-identity.md)

Wireflow: [`../../../architecture/community-forum-wireflow.md`](../../../architecture/community-forum-wireflow.md)

ADR-0002 remains **Proposed**. These files are an implementation-ready decomposition, not authority
to schedule product code. Only `live-0800-community-contract-approval` may begin before the four ADR
acceptance questions are resolved and the ADR status becomes Accepted. No work item has been added
to `.ldgr` by this documentation slice.

## MVP sequence

| ID | Ticket | Depends on |
| --- | --- | --- |
| 0800 | [Approve the community protocol contract](mvp/0800-community-contract-approval.md) | live-0729 |
| 0801 | [Define community manifest and event encodings](mvp/0801-community-manifest-events.md) | 0800 |
| 0802 | [Persist community manifests and append-only events](mvp/0802-community-event-storage.md) | 0801 |
| 0803 | [Implement verified discovery and invitation handoff](mvp/0803-community-discovery-invites.md) | 0801 |
| 0804 | [Implement explicit open-community membership](mvp/0804-community-membership.md) | 0801, 0802 |
| 0805 | [Implement topic, thread, and reply synchronization](mvp/0805-community-forum-protocol.md) | 0802, 0804 |
| 0806 | [Build community identity, navigation, and topic discovery UI](mvp/0806-community-forum-ui.md) | 0803, 0805 |
| 0807 | [Build community thread and reply workflows](mvp/0807-community-thread-reply-ui.md) | 0805, 0806 |
| 0808 | [Persist per-community landing and modality preferences](mvp/0808-community-preferences.md) | 0806 |
| 0809 | [Implement offline queue and deterministic replay](mvp/0809-community-offline-replay.md) | 0805, 0807 |
| 0810 | [Add local abuse controls and transparent host actions](mvp/0810-community-abuse-controls.md) | 0804, 0805 |
| 0811 | [Validate migration, privacy, and multi-profile behavior](mvp/0811-community-release-validation.md) | 0803 through 0810 |

## Later protocol programs

These are not MVP scope and must not be pulled forward implicitly.

| ID | Ticket | Depends on |
| --- | --- | --- |
| 0901 | [Design private community encryption and key epochs](later/0901-private-community-encryption.md) | MVP validated |
| 0902 | [Implement threshold roles and governance](later/0902-threshold-community-governance.md) | MVP validated |
| 0903 | [Implement replica locators and host migration](later/0903-community-replication-migration.md) | 0902 |
| 0904 | [Implement governance key recovery and explicit forks](later/0904-community-recovery-forks.md) | 0902, 0903 |

## Dependency graph

```text
0729 ── 0800 ── 0801 ── 0802 ── 0805 ── 0806 ── 0807
                    │       │       └────── 0808
                    │       └─ 0804 ─────── 0810
                    │               0807 ── 0809
                    └─ 0803 ───────────────────┐
0803 + 0804 + 0805 + 0806 + 0807 + 0808 + 0809 + 0810 ── 0811

validated MVP ── 0901
validated MVP ── 0902 ── 0903 ── 0904
```

## Shared constraints

- Extend Harbor's Ed25519 identity, relay-name claims, canonical CBOR signing, libp2p
  request-response, SQLite, and local-first stores. Do not add a blockchain or global registry.
- A relay peer ID/multiaddress is a locator, not community identity.
- Normal UI uses verified qualified user and community names, not public keys or raw peer IDs.
- MVP content is public. Do not describe link possession or host registration as private content.
- Host abuse actions and community moderator actions are different signed domains and UI labels.
- Completion of network work requires isolated packaged profiles and negative authorization/replay
  tests, not component mocks alone.
