---
ldgr_doc: 1
kind: ticket
id: ticket.0401-verified-name-ui
schema: ldgr.ticket.v1
status: ready
produces: [work:0401-verified-name-ui]
tags: [harbor, identity, ux]
---

# Replace display names with verified relay names

## Objective

Use the single verified relay name throughout onboarding, profiles, contacts, posts, calls, communities, and settings.

## Acceptance criteria

- [x] Onboarding registers a relay-unique name and explains `@name@relay` plainly.
- [x] Security-sensitive and unfamiliar-user views show the full qualified name; local shortening is unambiguous.
- [x] Arbitrary self-authored labels can never appear with verified-name styling.

## Validation

Component tests cover all identity surfaces, collisions, offline registration, expired claims, and untrusted relays.
