# Relay Names and Private Introductions

Status: Harbor 1.0 release contract (version 1 frozen)

Target: Required identity-safety slice before Harbor 1.0

Last updated: 2026-07-12

## 1. Purpose

Harbor needs memorable names without creating a global account database, exposing private social graphs, or allowing one user to impersonate another with an arbitrary display name.

This specification defines:

- relay-scoped, unique human names;
- cryptographic binding between a name and an existing Harbor peer identity;
- private, relay-brokered introductions;
- user-controlled capability grants;
- relay authentication and abuse controls;
- mention delivery and review without exposing private network addresses; and
- revocation, recovery, caching, and minimum release requirements.

Harbor identities remain device-controlled. Relays provide naming and delivery services, but do not own identities, private keys, content permissions, or social graphs.

## 2. Design principles

1. **Peer IDs are canonical.** Names are human-readable addresses that resolve to peer IDs. Stored ownership and authorization decisions use peer IDs.
2. **There are no arbitrary display names.** A profile's primary visible name is its verified relay name.
3. **Names are relay-unique.** `alice` may be assigned once by `relay.example`. The fully qualified name is `@alice@relay.example`.
4. **Users control disclosure.** Name existence must not automatically disclose a peer ID, public keys, network addresses, profile, or content.
5. **Relays broker; users authorize.** A relay may route an introduction request. Only the target user may grant contact capabilities.
6. **Social graphs remain local.** Relays must not receive or store a user's complete approved-contact list.
7. **Private keys never leave the user's device.** Relays store only public information and opaque encrypted envelopes.
8. **Public access is explicit.** A user may publish a limited public identity card and public content capability, but this is never implied by name registration.
9. **Protocol objects are signed and replay-resistant.** Security decisions must not rely on unsigned API responses or mutable labels.
10. **Privacy failures are failures.** Unknown, private, and unavailable targets must be difficult to distinguish through relay responses.

## 3. Existing cryptographic foundation

Each Harbor identity already has:

- an Ed25519 signing keypair;
- an X25519 key-agreement keypair;
- a libp2p peer ID derived from the Ed25519 signing key; and
- locally encrypted private-key material protected by the user's passphrase.

The Ed25519 key signs protocol records. X25519 establishes encrypted communication. The peer ID is the stable account identifier.

This specification does not introduce a blockchain, cryptocurrency, global DID registry, or centralized private-key service.

## 4. Terminology

**Local name**

The normalized name unique within a relay, such as `alice`.

**Relay authority**

The DNS hostname and signing identity of the relay assigning a local name, such as `relay.example`.

**Qualified name**

The complete human-readable address, such as `@alice@relay.example`.

**Name claim**

A record countersigned by the user and relay that binds a qualified name to a peer ID.

**Introduction request**

A signed request asking a relay to deliver an opaque envelope to a qualified name.

**Contact card**

An identity and routing record encrypted specifically for an approved requester.

**Capability grant**

A signed authorization defining what a specific peer may do or read.

**Public card**

An optional, deliberately published subset of identity information available without contact approval.

## 5. Name syntax and normalization

The canonical qualified-name form is:

```text
@<local-name>@<relay-hostname>
```

Version 1 local names:

- contain 3 to 32 characters;
- use lowercase ASCII letters, digits, and single hyphens;
- begin and end with a letter or digit;
- contain no consecutive hyphens; and
- are compared after lowercase normalization.

The restricted alphabet is intentional. Unicode names require a later confusable-character and script-mixing policy. Clients may not visually substitute a separate display name for the verified name.

Relay hostnames are lowercase IDNA ASCII hostnames without a scheme, port, path, query, or fragment. Clients must display the relay hostname when the identity is unfamiliar, when resolving mentions, and in any security-sensitive confirmation.

### 5.1 Version 1 wire contract

All signed version 1 records use deterministic CBOR maps with the field names and field order defined by the Rust protocol structs. Integers use their shortest lossless CBOR representation, byte fields are CBOR byte strings, and text is UTF-8. Timestamps are signed Unix seconds. Unknown domains, versions, fields that exceed the limits below, and noncanonical qualified names must be rejected before a security decision is stored.

| Record | Signature domain |
| --- | --- |
| Name request | `harbor/name-claim-request/1` |
| Relay name claim | `harbor/name-claim/1` |
| Relay challenge | `harbor/relay-challenge/1` |
| Introduction | `harbor/introduction/1` |
| Contact card | `harbor/contact-card/1` |
| Capability grant | `harbor/capability-grant/1` |
| Capability revocation | `harbor/capability-revocation/1` |
| Private mention | `harbor/mention/1` |
| Relay-key rotation | `harbor/relay-key-rotation/1` |

The local name is 3 to 32 ASCII bytes, the relay hostname is at most 253 ASCII bytes, opaque ciphertext is at most 64 KiB, and a contact card carries at most 32 capabilities. Ed25519 and X25519 public keys are exactly 32 bytes, Ed25519 signatures are exactly 64 bytes, and nonces used for durable replay protection are at least 16 bytes.

Sequences and capability revisions are unsigned 64-bit integers. Zero is invalid where a sequence establishes authority. A receiver applies only a strictly newer verified sequence or revision; equal values are idempotent only when the signed bytes are identical, and an older grant can never override a newer revocation. Name-claim requests tolerate at most 300 seconds of clock skew, relay challenges live for no more than five minutes, introduction envelopes live for no more than 24 hours, and the signed expiry is always checked by the receiving authority.

The normative name-request golden vector is asserted in `models::relay_identity::tests::name_claim_request_has_deterministic_bytes`. Mutation tests must also prove that changing the domain changes the signed bytes and that tampering, wrong-recipient decryption, reordered revisions, and substituted keys fail closed.

## 6. Name registration

### 6.1 User request

The client creates a `NameClaimRequest`:

```json
{
  "version": 1,
  "localName": "alice",
  "relay": "relay.example",
  "peerId": "12D3KooW...",
  "ed25519PublicKey": "base64...",
  "x25519PublicKey": "base64...",
  "sequence": 1,
  "issuedAt": 1783861200,
  "nonce": "base64...",
  "userSignature": "base64..."
}
```

The user signature covers every preceding field using Harbor's canonical signing encoding.

### 6.2 Relay validation

Before assignment, the relay must:

1. validate syntax and canonical normalization;
2. verify that the Ed25519 key derives the supplied peer ID;
3. verify the user signature;
4. confirm that the name is not active, reserved, retired, or pending;
5. apply registration rate limits and abuse policy; and
6. atomically reserve the name before returning success.

### 6.3 Countersigned claim

The relay returns a `NameClaim` containing the request fields plus:

```json
{
  "status": "active",
  "notBefore": 1783861200,
  "notAfter": 1815397200,
  "relayKeyId": "2026-01",
  "relaySignature": "base64..."
}
```

The relay signature covers the complete user-signed request and the relay fields. A claim is valid only when both signatures verify.

Name claims are public proofs only when voluntarily presented by the user. The relay is not required to expose a browsable directory.

### 6.4 Reassignment

Version 1 names must not be silently reassigned. Deleted or abandoned names enter a retired state. Reuse requires a future, explicitly specified recovery policy. This prevents old mentions and cached cards from resolving to a different person.

## 7. Relay authentication

Relay sessions use challenge-response authentication:

1. Client sends its peer ID and requests a challenge.
2. Relay returns a random, single-use challenge containing the relay hostname, action audience, issue time, expiry, and nonce.
3. Client signs the challenge with its Ed25519 key.
4. Relay verifies the signature and peer-ID derivation.
5. Relay issues a short-lived, audience-bound session token.

Session tokens must:

- expire quickly;
- be bound to the authenticated peer ID and relay;
- identify permitted relay API actions;
- be invalid after relay signing-key rotation rules require it; and
- never replace signatures on durable protocol records.

No email address, password, or social-login account is required for relay authentication.

## 8. Private name resolution

A normal name lookup must not return the target's peer ID, Ed25519 signing key, routing addresses, profile, or existence flag. To seal an introduction without already knowing the target, the relay-signed work challenge carries a 32-byte X25519 delivery key. For an active target this is the delivery key from the countersigned claim; for every other case it is a deterministic relay-derived decoy with the same wire shape. Clients keep this key only in a short-lived in-memory cache and never present it as identity authority.

The relay response must use uniform status and timing envelopes for:

- an unknown name;
- a private name;
- an offline target;
- a request accepted for forwarding; and
- a request suppressed by target policy.

The standard response is equivalent to:

```json
{
  "status": "accepted-for-processing",
  "requestId": "opaque-random-id",
  "retryAfter": 3600
}
```

This response does not confirm that the target exists. Relays should add bounded timing jitter and must not expose directory-listing or prefix-search endpoints.

The delivery key is an encryption input, not a resolved identity. The sender leaves the recipient peer-ID field empty and signs the qualified target name. A recipient accepts that form only after AEAD decryption succeeds and the signed qualified name matches its own current verified claim. Once two users later approve a relationship and exchange identity material, they may be able to recognize that an earlier delivery key belonged to the same user; version 1 prevents unauthenticated directory enumeration, not all post-contact correlation.

## 9. Introduction requests

An `IntroductionRequest` contains:

```json
{
  "version": 1,
  "requestId": "uuid",
  "target": "@alice@relay.example",
  "requesterPeerId": "12D3KooW...",
  "requesterSigningKey": "base64...",
  "requesterEphemeralX25519Key": "base64...",
  "purpose": "contact",
  "messageCiphertext": "base64...",
  "issuedAt": 1783861200,
  "expiresAt": 1783947600,
  "challengeId": "opaque-id",
  "workNonce": 1847291,
  "signature": "base64..."
}
```

The signature covers the canonical record. The encrypted message must not expose free-form text to the relay. The target name is visible because the relay needs it for routing.

Relays queue introduction envelopes for bounded periods. They must delete expired, delivered, or rejected envelopes according to a documented retention policy.

## 10. Abuse resistance and proof of work

Proof of work supplements authentication; it does not replace it.

The relay issues an action-bound challenge. The requester finds a nonce satisfying:

```text
SHA-256(
  protocol-version || relay || challenge-id || requester-peer-id ||
  target-qualified-name || action || expiry || nonce
) < relay-provided-target
```

Challenges must be:

- signed by the relay;
- random and single-use;
- bound to one requester, target, action, and relay;
- valid for no more than five minutes; and
- rejected after successful use.

Difficulty is adaptive. Recommended policy:

- approved contact capability: no proof of work;
- healthy authenticated relay member: rate limit only or minimal work;
- unknown requester: modest interactive work;
- suspicious or burst traffic: increased work and stricter limits;
- blocked peer: reject without forwarding, while preserving a generic external response.

Relays must also apply per-peer, per-network-origin, per-target, and global rate limits. Proof of work alone is not sufficient against specialized hardware or distributed abuse.

## 11. Target review and approval

The target client decrypts the introduction envelope and presents the verified qualified name and peer ID. The target may:

- approve with selected capabilities;
- approve a one-time interaction;
- ignore;
- reject; or
- block the requester locally.

The relay must not learn which capabilities were granted. A rejection should not provide the requester with a reliable target-existence oracle.

## 12. Contact cards and capability grants

On approval, the target creates a contact card encrypted to the requester's ephemeral or long-term X25519 key. It may include:

```json
{
  "version": 1,
  "nameClaim": {},
  "peerId": "12D3KooW...",
  "ed25519PublicKey": "base64...",
  "x25519PublicKey": "base64...",
  "routing": [],
  "capabilities": [],
  "issuedAt": 1783861200,
  "expiresAt": 1786453200,
  "revision": 7,
  "revocationId": "random-opaque-id",
  "signature": "base64..."
}
```

Capabilities are explicit and independently revocable. Examples include:

- `wall.read.contacts`;
- `message.send`;
- `call.audio.request`;
- `call.video.request`;
- `profile.media.read`; and
- `mention.deliver`.

The card and grants are user-signed before encryption. The relay carries only ciphertext.

An approved-contact list remains on the user's devices. Synchronization between a user's own devices must be end-to-end encrypted and is outside relay directory storage.

## 13. Public identities and content

Public visibility is a separate, explicit capability. A user enabling public wall or RSS access may publish a `PublicIdentityCard` containing only the fields necessary to verify and retrieve that public content.

Public cards must not include:

- private addresses not intended for public reachability;
- contact lists;
- private capabilities;
- private profile fields; or
- X25519 private material.

Disabling public visibility stops future relay resolution and publication. Previously downloaded public content cannot be cryptographically recalled from other devices.

## 14. Mentions and tags

Mentions are structured protocol fields, not decorative `@name` text. A post stores:

- the qualified name typed by the author;
- the resolved target peer ID when disclosure is authorized;
- the target name-claim digest;
- delivery state; and
- the author's signature over the mention and post.

For a private or unresolved target, the relay forwards an encrypted mention envelope using the introduction mechanism. It does not reveal the target's peer ID to the author.

The target may accept the notification without granting broader contact access. Reposting a tagged bug report onto a service identity's wall remains an explicit action by that identity.

## 15. Caching and offline behavior

Clients may cache:

- name claims voluntarily received from their owner;
- contact cards encrypted for the local identity;
- relay public signing keys and rotation proofs;
- capability grants; and
- negative or generic delivery responses for rate-control purposes.

Clients must not treat an expired relay claim as authority for a new relationship. Existing signed content remains attributable to the peer ID even if its name claim expires or its relay disappears.

Offline targets may receive queued introductions when they reconnect. Queue expiry must be visible to the requester without revealing whether the target was offline or nonexistent.

## 16. Revocation and rotation

Capability revocations are signed by the issuing user and identify the grant or revocation ID. Relays may transport revocations but are not their authority.

Relay signing keys require an offline recovery key and signed rotation chain. Clients pin the first trusted relay key and accept a successor only when authorized by the previous key or a documented recovery procedure.

Compromise of a relay signing key can produce fraudulent name claims but cannot sign as a user, decrypt private content, or derive user private keys. Clients must surface claims issued during a known compromise window and support relay-wide invalidation metadata.

User signing-key rotation and peer-ID continuity are deferred until separately specified. Version 1 must include version and sequence fields so rotation can be added without ambiguous records.

## 17. Privacy and threat model

### 17.1 Relay can observe

- its registered local names;
- authenticated peer IDs connecting to it;
- requested target names;
- request time, size, source network metadata, and delivery state;
- public identity cards deliberately published through it; and
- encrypted envelope ciphertext.

### 17.2 Relay must not possess

- user private keys;
- plaintext private messages or introductions;
- complete approved-contact lists;
- decrypted capability grants; or
- authority to publish content as a user.

### 17.3 This protocol mitigates

- copied profile names used for direct impersonation;
- unauthorized content access;
- bulk unauthenticated introduction spam;
- relay forgery of user content;
- replay of registration and introduction requests; and
- passive disclosure of contact capabilities from relay storage.

### 17.4 This protocol does not fully prevent

- a relay correlating connection and routing metadata;
- traffic analysis;
- denial of service by a malicious relay;
- a malicious relay censoring or refusing name requests;
- compromise of an unlocked user device;
- recipients copying content they were legitimately allowed to view; or
- coordinated abuse using many valid peer identities.

Users requiring stronger metadata privacy need onion routing, private information retrieval, or multi-relay delivery. Those are future protocol layers.

## 18. Required release slice

Before Harbor 1.0, implementation must include:

1. relay-unique ASCII names and qualified-name UI;
2. removal of arbitrary primary display names;
3. user-and-relay-countersigned name claims;
4. Ed25519 relay challenge-response authentication;
5. private introduction envelopes;
6. local approval and blocking;
7. encrypted contact cards using existing X25519 keys;
8. explicit capability grants using peer IDs;
9. structured mentions with private delivery;
10. replay protection, expiry, rate limiting, and adaptive proof of work;
11. generic non-enumerating relay responses;
12. relay-key pinning and rotation metadata;
13. migration of existing local display names into unverified legacy labels that are never presented as verified names; and
14. integration tests with at least two relays and three independent identities.

## 19. Deferred work

- custom-domain names and DNS TXT resolution;
- cross-relay name migration and redirects;
- Unicode local names;
- multiple aliases;
- private information retrieval;
- onion-routed introductions;
- replicated global directories;
- peer-ID-preserving user key rotation;
- relay consortium governance; and
- automated moderation or bug-report acceptance.

## 20. Security invariants

Implementations must preserve these invariants:

1. A relay cannot create a valid user-signed post.
2. A user cannot create a valid relay-verified name without relay approval.
3. Possession of a name claim grants no content capability.
4. Possession of a public key grants no private-content access.
5. A capability for one peer cannot be exercised by another peer.
6. A relay database breach reveals no user private key or plaintext contact card.
7. A private lookup does not disclose whether a target exists.
8. A name is never silently rebound to a different peer ID.
9. Content attribution survives relay loss because it is anchored to the peer ID and user signature.
10. Revocation cannot erase content already disclosed to an authorized recipient.
