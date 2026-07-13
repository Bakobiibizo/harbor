# Cross-platform live beta acceptance

This is the final operator gate for `live-0730-cross-platform-acceptance`. Run it with packaged
builds on Windows and macOS, two real users, and a third isolated profile. Automated and component
tests are prerequisites, not substitutes for this session.

## Before the session

1. Install the exact release candidate that the public download will serve on Windows and macOS.
2. Use three isolated test profiles: `alpha` on Windows, `bravo` on macOS, and `charlie` on either
   platform. Do not reuse a personal Harbor profile.
3. Record the OS versions and architectures, package signing/notarization state, Harbor version and
   commit, relay artifact, and relay namespace.
4. Confirm microphone and camera access can be granted on both primary platforms. Have a second
   network or hotspot available for relay/NAT behavior.
5. Create an ignored evidence directory and initialize the manifest:

   ```bash
   pnpm acceptance:live-beta init
   ```

6. Populate every metadata field. For example:

   ```bash
   pnpm acceptance:live-beta metadata commit "$(git rev-parse HEAD)"
   pnpm acceptance:live-beta metadata version "1.4.1-beta.7"
   pnpm acceptance:live-beta metadata windowsVersion "Windows 11 24H2 build ..."
   pnpm acceptance:live-beta metadata windowsArchitecture "x86_64"
   pnpm acceptance:live-beta metadata macosVersion "macOS ..."
   pnpm acceptance:live-beta metadata macosArchitecture "arm64"
   pnpm acceptance:live-beta metadata macosPackage "signed and notarized universal DMG"
   pnpm acceptance:live-beta metadata thirdProfilePlatform "Windows x86_64"
   pnpm acceptance:live-beta metadata relayArtifact "..."
   pnpm acceptance:live-beta metadata relayNamespace "harbor.social"
   pnpm acceptance:live-beta metadata operator "initials"
   ```

## Run order

Use the detailed expected behavior in
[the reproduction matrix](live-beta-reproduction-matrix-2026-07-12.md). Run in this order so later
scenarios reuse only state established by earlier passing scenarios.

1. `R01` and `R02`: create, lock, unlock, quit, relaunch, and recover both primary identities.
2. `R03` and `R04`: open the HTTPS contact handoff in a clean browser, then add the same link inside
   Harbor without editing its scheme.
3. `R05`: request, review, accept, reject, retry, restart, and verify durable contact state and
   notifications in both directions.
4. `R06`: publish public and contacts-only content. Verify the accepted contact can read it and
   `charlie` cannot. Revoke access and confirm stale cached authorization is not accepted.
5. `R07` through `R10`: publish, edit, message, reconnect, transfer slow media, prefetch offline
   changes, background the app, and confirm bounded refresh and notifications without manual reload.
6. `R11A`, `R11B`, and `R11C`: follow
   [the Windows/macOS call checklist](windows-macos-call-acceptance.md). Run both call directions,
   permission denial/recovery, hangup/relaunch, relay/NAT behavior, and a three-profile group call.
7. `R12` through `R15`: validate safe link cards, provider consent, composer and filters, onboarding,
   bug-report navigation, keyboard/pointer/reduced-motion interaction, and the absence of raw keys on
   normal user surfaces.

For every scenario, save at least one sanitized screenshot, short recording, or log excerpt and
record the result:

```bash
pnpm acceptance:live-beta record R01 pass "Both packaged profiles resumed their verified names after relaunch" artifacts/live-beta-acceptance/r01.png
```

Use `fail` for a reproducible product failure and `blocked` when the environment prevents the step.
Neither outcome permits publication.

## Evidence hygiene

Evidence may include qualified test names, timestamps, version metadata, sanitized transport state,
and UI screenshots made with test content. It must not include passwords, recovery material, private
keys, authorization tokens, raw private-message bodies, personal contact graphs, or private media.
Crop unrelated desktop content and inspect logs before attaching them.

## Gate decision

Run:

```bash
pnpm acceptance:live-beta check
```

The command exits nonzero until every required metadata field is populated, every P0 scenario is
recorded as `pass`, every note is present, and every referenced evidence file exists. A failed or
blocked scenario returns to its owning work item. Do not publish the broad beta or mark `live-0730`
done while this command is nonzero.
