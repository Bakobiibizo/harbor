# Community forum low-fidelity wireflow

This wireflow illustrates [ADR-0002](adr-0002-community-forum-identity.md). Labels are product copy,
not final visual styling. Raw peer IDs and keys are absent from normal surfaces.

## 1. Community home and discovery

```text
┌ Communities ──────────────────────────────────────────────────────────┐
│ [Join community]                                      [My activity]  │
│                                                                     │
│ Harbor Community                                                    │
│ community:harbor@harbor.social      3 unread threads   Synced 2m ago│
│ General discussion and help                                         │
│ [Open]                                                              │
│                                                                     │
│ Substrate Builders                                                  │
│ community:substrate@relay.example   Cached             Host offline │
│ [Open cached copy]                                                   │
└─────────────────────────────────────────────────────────────────────┘
```

The title aids scanning. The qualified community address carries identity. A transport problem says
`Host offline`, not `Community invalid`.

## 2. Join from a link

```text
Browser / clipboard / QR
        │
        ▼
Validate HTTPS/harbor URI → relay proof → manifest digest → locator expiry
        │
        ├─ invalid ──> “This invitation cannot be verified” [Details] [Cancel]
        │
        ▼
┌ Join Harbor Community? ──────────────────────────────────────────────┐
│ community:harbor@harbor.social                                      │
│ Public community · Open membership                                  │
│                                                                     │
│ Anyone can read posts. The host will know that you joined, and your │
│ signed posts reveal your participation. No public member directory  │
│ is provided by Harbor.                                              │
│                                                                     │
│ Rules digest verified · Host identity verified                      │
│ [Review rules]                                [Cancel] [Join]        │
└─────────────────────────────────────────────────────────────────────┘
```

After Join, the client signs `CommunityJoin`, stores the manifest, and begins incremental replay.
Failure leaves the verified invite available for retry.

## 3. Distinct forum landing page

```text
┌ Harbor Community ────────────────────────────────────────────────────┐
│ community:harbor@harbor.social  [Following ▾] [Share] [Community ▾] │
│ Host: reachable · Updated just now                                  │
├ Topics ───────────────┬──────────────────────────────────────────────┤
│ General            12 │ [New thread]                                 │
│ Help                31 │                                             │
│ Show and tell        8 │ Filter: [All] Images Video Audio            │
│                      │ Sort: [Latest activity ▾]                     │
│ My defaults          │                                             │
│ Topic: Help          │ “How do I move a profile?”        6 replies  │
│ Filter: All          │ Last reply by @alice@relay.example   5m      │
│                      │                                             │
│                      │ “Relay deployment checklist”      14 replies │
│                      │ Last reply by @sam@harbor.social     1h      │
└──────────────────────┴──────────────────────────────────────────────┘
```

The topic/thread hierarchy, reply counts, unread state, and stable titles distinguish this from the
time-ordered Feed. The chosen topic, sort, and filter persist for this `community_id` only.

## 4. Create a thread

```text
┌ New thread in Help ──────────────────────────────────────────────────┐
│ Title *  [How do I move a profile?_______________________________]  │
│ Body     [_______________________________________________________]  │
│          [_______________________________________________________]  │
│                                                                     │
│ [Add image] [Add video] [Add audio]   Posting publicly              │
│                                                                     │
│ Status: Local draft                                  [Cancel] [Post]│
└─────────────────────────────────────────────────────────────────────┘

Post → Queued offline → Submitted to host → Confirmed in community log
               └──────── failure ───────> Draft retained [Retry]
```

The derived modality is stored in the signed event. The UI does not offer Contacts-only visibility
inside an MVP public community.

## 5. Thread and replies

```text
┌ How do I move a profile? ────────────────────────────────────────────┐
│ Help · started by @richard@harbor.social · Confirmed                │
│ [opening post and media]                              [Follow] [...] │
├ 6 replies ───────────────────────────────────────────────────────────┤
│ @alice@relay.example · 5m                                           │
│ Export the profile first…                              [Reply] [...] │
│   ↳ @richard@harbor.social · 2m                                     │
│     That worked, thanks.                               [Reply] [...] │
│                                                                     │
│ [Write a reply…__________________________________________________]  │
│                                                    [Add media] [Send]│
└─────────────────────────────────────────────────────────────────────┘
```

Deeper reply parents remain preserved in data but render at one indentation level in MVP. Author
deletes become visible tombstones when needed to preserve reply context.

## 6. Community information and trust

```text
┌ Community information ──────────────────────────────────────────────┐
│ Harbor Community                                                    │
│ community:harbor@harbor.social                                      │
│ Community ID: verified [Copy diagnostic ID]                         │
│ Manifest: verified · Rules revision: genesis                        │
│ Host: harbor.social · Locator expires in 4d                         │
│                                                                     │
│ This host provides storage and replay. It is not automatically a    │
│ community moderator. It can still refuse or stop hosting data.      │
│                                                                     │
│ [Export signed archive] [Leave community]                            │
└─────────────────────────────────────────────────────────────────────┘
```

Diagnostic IDs require an explicit action. Leaving offers `Keep cached copy` or `Delete local copy`.

## 7. Abuse actions

```text
Post menu
├─ Hide this post                 local only
├─ Mute @name@relay               local only
├─ Block @name@relay              local only; block list stays local
├─ Report to host                 sends selected post/event reference + reason
└─ Copy signed reference

Host quarantine marker
┌ Host action ─────────────────────────────────────────────────────────┐
│ This host stopped serving an event under its published abuse policy.│
│ The action does not alter signatures or remove copies from another  │
│ replica. [Policy] [Technical details]                                │
└─────────────────────────────────────────────────────────────────────┘
```

Later moderator actions use a different `Community moderator action` label and show the verified
capability/quorum that authorized them.

## 8. Offline, replay, and recovery states

```text
Open community
  ├─ cache present + host offline → browse cache; compose queues locally
  ├─ cache absent + host offline  → verified identity only; Retry
  ├─ cursor stale                 → replay signed events in bounded pages
  ├─ missing parent              → quarantine event; request parent; never render as root
  ├─ invalid signature/replay    → reject; sanitized warning; no cursor advancement
  └─ host gone                   → cached read-only + export; later replica/migration flow
```

## 9. Later private and governance flows

These controls must not appear in MVP:

```text
Private invitation → verify scoped invite → join approval → receive content-key epoch
Member removed     → signed revocation → rotate epoch → no new content for removed key

Policy proposal → collect steward threshold signatures → append governance event
Moderator action → verify scoped capability + revision → apply policy presentation
Lost steward key → threshold-signed rotation; no quorum → governance-frozen or explicit fork
```

## Acceptance walkthrough

User research should confirm that a participant can answer, without viewing diagnostic keys:

1. Which community am I in?
2. Who is speaking, using their relay-qualified name?
3. How do I start a discussion versus reply to one?
4. What is public, and what membership metadata reaches the host?
5. Is an action local, a host action, or a later community moderator action?
6. What remains usable when the host is offline?
7. Why would I use this instead of Feed?
