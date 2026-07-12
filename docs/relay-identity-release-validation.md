# Relay identity and privacy release validation

This is the reproducible release gate for relay-scoped names, private introductions, contact capabilities, and private mentions. Automated tests are necessary but do not replace the live two-relay, three-identity exercise.

## Automated gate

From the repository root on Linux or WSL:

```bash
./scripts/validate-relay-identity-release.sh 2>&1 | tee relay-identity-validation.log
```

The script tests canonical names, signed introduction and contact-card cryptography, wrong-recipient and tamper rejection, capability expiry and revocation, relay wall denial, challenge replay, uniform introduction responses, relay-key rotation, forged posts, and private mention parsing/review logic.

Retain the log with the release evidence. A command that selects zero tests is not evidence; confirm every invocation reports at least one test executed.

## Isolated topology

Use disposable data only. Never copy a production identity or relay key into this exercise.

| Process | Namespace or profile | Purpose |
| --- | --- | --- |
| Relay A | `alpha.test` | Alice and Carol namespace |
| Relay B | `beta.test` | Bob namespace and cross-relay path |
| Alice | `HARBOR_PROFILE=identity-alice` | Content owner and capability issuer |
| Bob | `HARBOR_PROFILE=identity-bob` | Approved requester |
| Carol | `HARBOR_PROFILE=identity-carol` | Unauthorized/adversarial requester |

Give every process a separate database directory and port. Record relay peer IDs, listening addresses, build commit, OS, and UTC start time. Do not record private keys, session tokens, decrypted envelopes, or contact lists.

## Required scenario

1. Start both relays with empty databases. Start Alice, Bob, and Carol with isolated profiles.
2. Register `@alice@alpha.test`, `@bob@beta.test`, and `@carol@alpha.test`. Confirm the qualified names map to the expected peer IDs.
3. Have Carol race Alice for `alice` on Relay A. Exactly one atomic assignment may succeed. Restart Relay A and confirm the result persists.
4. Bob requests a private introduction to Alice. Confirm Relay A does not expose Alice's peer ID, keys, online state, or decision to Bob.
5. Confirm Alice sees Bob's full verified `@bob@beta.test` name. Ignore once, reject once, then send a fresh request and approve it. Restart Alice between review and approval to prove local persistence.
6. Alice issues only `wall:read` and `mention:send`. Confirm Bob cannot message or call until those capabilities are separately granted.
7. Publish one public and one contacts-only post. Bob must receive both; Carol must receive only the public post through direct and relay-assisted paths.
8. Alice revokes `wall:read` while Bob is offline. Restart Bob, reconnect, and confirm no new private rows are served. Previously downloaded content may remain locally and must not be described as erased.
9. Bob mentions Alice. Alice reviews and accepts or rejects the mention locally. Carol must not be able to replace Bob's resolved peer ID by copying his text or avatar.
10. Restart all five processes. Confirm names, blocks, decisions, capability revisions, revocations, and relay-key pins recover without reusing a stale session.

## Adversarial matrix

Record pass/fail and the evidence location for every row.

| Attempt | Required result |
| --- | --- |
| Enumerate local names | Unknown, private, offline, and forwarded requests have indistinguishable public responses; no directory is returned. |
| Replay registration nonce | Rejected without changing the name assignment or sequence. |
| Concurrent name collision | One winner; no overwrite, split assignment, or retired-name reuse. |
| Forge user claim | Rejected when the key does not derive the peer ID or the user signature is altered. |
| Substitute relay key | Untrusted key ID rejected; pinned rotation required. |
| Replay authentication challenge/session | Single-use challenge rejected; expired or wrong-audience session rejected. |
| Modify introduction envelope | Rejected before display; no decision record created. |
| Decrypt as wrong recipient | AEAD authentication fails without returning partial plaintext. |
| Steal or alter capability | Wrong subject, issuer, capability, signature, expiry, or revision rejected. |
| Reorder grant and revocation | Highest valid revision wins; an older grant cannot restore access. |
| Retrieve without capability | Public rows only; private media and social events are also absent. |
| Relay restart/loss | Cached proofs remain attributable, private lookup does not become public, and stale claims are not presented as current authority. |

## Evidence checklist

- Automated log and exact Git commit
- Sanitized process topology and relay peer IDs
- Database restart observations for both relays and all three identities
- Network-path evidence for direct and relay-assisted private-wall denial
- Grant and revocation revision numbers, without signatures or secret material
- A failed-test or blocked-step record for anything not executed

Do not mark work item 0501 complete while a matrix row is untested, while the live topology used shared profile data, or when only screenshots/self-certification exist.

## Current automation boundary

The repository automates component and relay multi-profile tests. It does not yet orchestrate five packaged GUI processes end to end, simulate relay loss at the process level, or capture network traces proving response timing equivalence. Those steps remain mandatory manual release evidence until a control-surface runner covers them.
