# ADR-0001: First production group-call topology

- **Status:** Accepted
- **Date:** 2026-07-01
- **Scope:** Harbor group audio, group video, and screen sharing. One-to-one voice signaling remains compatible with the existing signed calling helpers.

## Decision

Harbor's first production group-call topology is a **relay-assisted small-group full mesh**:

- Each participant establishes a direct WebRTC `RTCPeerConnection` to every other participant.
- Harbor/libp2p request-response and configured libp2p relays carry signed signaling only. They do not relay, mix, transcode, or record media.
- WebRTC ICE may use configured STUN servers and may use operator-provided TURN relay candidates when direct media connectivity fails.
- No SFU, MCU, centralized media service, or centralized group signaling authority is part of the first production group-call release.

This choice preserves Harbor's local-first peer-to-peer model while setting an explicit scale limit that can be validated before release.

## Participant limit

The production limit is **4 total participants per group call, including the local user**.

Downstream implementation must enforce this limit in service validation, signaling validation, and UI affordances. Invites or joins that would make the roster exceed 4 participants must be rejected before SDP or ICE is emitted. A future release that supports more than 4 participants requires a new ADR because it likely changes topology, cost, security review, validation, and deployment responsibilities.

## NAT and connectivity expectations

- LAN, public-address, and permissive home/office NAT scenarios are expected to connect with direct ICE candidates.
- Existing Harbor/libp2p relay infrastructure may be used to exchange call setup messages when direct libp2p connections are unavailable.
- libp2p relay is **not** a media relay and must not be presented as a TURN/SFU substitute.
- Strict or symmetric NAT scenarios require an operator-configured TURN service for media relay candidates. Without TURN, the runtime must fail gracefully and surface a connectivity error instead of silently downgrading security or routing media through an unreviewed service.
- Public STUN/TURN services must not be hard-coded as production dependencies. ICE server configuration must be explicit and inspectable by operators/users.

## Privacy and security contract

Group calls must not weaken one-to-one call guarantees:

- Every participant must already be a known contact or satisfy the same identity verification policy required by one-to-one calls.
- The existing `Call` permission is required for every remote participant before the local client sends or accepts SDP/ICE for that participant.
- Group membership, role changes, offers, answers, ICE candidates, and hangups must be signed with the sender's Ed25519 identity and bound to a stable `group_call_id`, topology version, participant roster version, and timestamp/nonce suitable for replay rejection.
- Media remains encrypted by WebRTC DTLS-SRTP on each peer connection. Because this topology has no SFU/MCU, no Harbor-operated media server can inspect media.
- Metadata leakage is expected: peers, libp2p relays, STUN, and TURN services can observe IP addresses, timing, and connection attempts. UI and docs must not imply participant anonymity.
- TURN relays, when configured, may see encrypted packet metadata but not plaintext media. TURN credentials must be scoped and rotated; static production credentials must not be embedded in the client.

## Downstream implementation requirements

All group-call implementation tickets and code reviews must cite this ADR and the 4-participant limit.

### Signaling

- Add group-call signaling messages that explicitly name `topology = relay_assisted_mesh_v1` and `max_participants = 4`.
- Model group membership as a signed roster. A client must reject signaling from peers that are not on the accepted roster version.
- Exchange SDP offers/answers pairwise for each participant pair. Do not introduce SFU-oriented SDP routing or centralized session ownership without a replacement ADR.
- Route signaling through the existing Harbor/libp2p request-response architecture and relay paths when needed, preserving the current signing and permission checks.
- Reject invites that exceed the participant limit, omit required signatures, or include participants lacking `Call` permission.

### Runtime

- Use one `RTCPeerConnection` per remote participant, so a full 4-person call creates at most 3 peer connections per client.
- Budget CPU, encoder, decoder, and uplink bandwidth for O(n-1) local media fan-out. Runtime quality controls must prefer bitrate/resolution reduction over increasing the participant limit.
- Screen sharing is allowed within the same 4-participant roster; it does not create a separate topology.
- The runtime must expose explicit failure states for ICE failure, TURN misconfiguration, permission denial, roster mismatch, and participant-limit rejection.

### UI

- Invite controls must cap the selected roster at 4 total participants and explain that Harbor's first group-call release uses a small-group mesh.
- Roster, permission, and connectivity errors must be visible to the caller instead of becoming generic call failures.
- UI copy must not promise large rooms, webinar behavior, server-side recording, or strict-NAT reliability unless the required infrastructure and validation are present.

### Validation

Minimum release validation for group calls:

- 2, 3, and 4 Harbor profiles in one room using the mesh topology.
- A relayed-signaling scenario using the existing Harbor relay path.
- At least one WAN/NAT scenario with documented ICE server configuration.
- A strict/symmetric NAT scenario if and only if the release claims strict-NAT support; this scenario requires operator-provided TURN.
- Negative tests for over-limit rosters, missing `Call` permission, unsigned or mismatched roster versions, and ICE/TURN failure reporting.

### Deployment and operations

The selected topology does not require SFU or MCU infrastructure.

Required or conditional infrastructure responsibilities:

- **libp2p relay/bootstrap:** use Harbor's existing relay deployment path (`relay-server/` and `infrastructure/`) for signaling reachability. Operators are responsible for relay endpoint distribution, updates, and monitoring under the existing relay policy.
- **STUN:** production internet builds must expose configurable ICE server settings. Public STUN endpoints may be defaults only when documented; they are not secret-bearing.
- **TURN:** required only for releases that claim strict/symmetric NAT support or when validation shows direct ICE is insufficient for the supported environment. TURN must be operator-owned or explicitly contracted, deployed with TLS where applicable, monitored for abuse/cost, and configured with short-lived credentials or a documented credential-rotation process. Static TURN credentials must not be checked into the repository or shipped in the client binary.

Release blockers:

- Do not enable production group calls until service and UI validation enforce the 4-participant mesh contract.
- Do not claim strict-NAT support until a TURN deployment path, credentials policy, update procedure, and multi-profile validation evidence exist.
- Do not add SFU/MCU behavior, server recording, or media relay fallback without a replacement ADR and security review.

## Consequences

### Benefits

- Preserves Harbor's decentralized media path and minimizes new infrastructure.
- Keeps media end-to-end between participants.
- Allows production validation with existing desktop profiles and relay infrastructure.

### Costs and limits

- Bandwidth and CPU scale linearly per client with the number of remote participants and quadratically across the room.
- Four participants is a hard product limit for this topology.
- Connectivity behind strict NATs depends on TURN when direct ICE cannot connect.
- Larger rooms, moderation-heavy rooms, server-side recording, and low-bandwidth mobile optimization likely require an SFU-oriented ADR.

## Requirement mapping

- `req.01`: selects relay-assisted small-group full mesh, participant limit 4, NAT expectations, privacy/security tradeoffs, and operational requirements.
- `req.02`: defines downstream signaling, runtime, UI, validation, and deployment requirements with no ambiguous topology wording.
- `req.03`: identifies libp2p relay, STUN, and conditional TURN responsibilities, secrets policy, and release blockers.
