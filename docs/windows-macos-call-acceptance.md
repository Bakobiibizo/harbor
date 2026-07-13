# Packaged Windows and macOS call acceptance

This is the hardware acceptance runbook for Harbor 1:1 voice, 1:1 video, and the three-profile group-call slice. Run it against packaged release candidates on a real Windows host and a real Mac. The deterministic test harness is a preflight gate, not a substitute for this run.

Use disposable identities and redact peer IDs, IP addresses, TURN usernames, and all credential values from shared evidence. Never attach SDP, ICE candidate bodies, media captures, signatures, private keys, passwords, or TURN credentials.

## 1. Record package identity

On Windows PowerShell:

```powershell
Get-AuthenticodeSignature .\Harbor_*.msi | Format-List Status,StatusMessage,SignerCertificate
Get-FileHash .\Harbor_*.msi -Algorithm SHA256
Get-Item .\Harbor_*.msi | Select-Object Name,Length,LastWriteTimeUtc
```

On macOS Terminal:

```bash
codesign --verify --deep --strict --verbose=2 Harbor.app
spctl --assess --type execute --verbose=4 Harbor.app
shasum -a 256 Harbor_*.dmg
defaults read ./Harbor.app/Contents/Info CFBundleShortVersionString
```

Record the release tag, commit SHA, package filename, package SHA-256, app version, OS version, CPU architecture, signature result, and download URL on each host.

## 2. Launch isolated packaged profiles

Close all Harbor windows first. Do not reuse a normal Harbor data directory.

Windows PowerShell, after replacing the executable path with the installed release candidate:

```powershell
$env:HARBOR_PROFILE = "accept-win-a"
$env:HARBOR_DATA_DIR = "$env:TEMP\harbor-accept-win-a"
& "$env:LOCALAPPDATA\Programs\Harbor\Harbor.exe"
```

macOS Terminal:

```bash
HARBOR_PROFILE=accept-mac-b \
HARBOR_DATA_DIR=/tmp/harbor-accept-mac-b \
/Applications/Harbor.app/Contents/MacOS/Harbor
```

For the third group participant, use another packaged Windows or macOS host/profile:

```bash
HARBOR_PROFILE=accept-group-c \
HARBOR_DATA_DIR=/tmp/harbor-accept-group-c \
/Applications/Harbor.app/Contents/MacOS/Harbor
```

If the installed paths differ, locate them without changing the package:

```powershell
Get-ChildItem "$env:LOCALAPPDATA\Programs","$env:ProgramFiles" -Filter Harbor.exe -Recurse -ErrorAction SilentlyContinue
```

```bash
mdfind 'kMDItemCFBundleIdentifier == "io.harbor.desktop"'
```

## 3. Preflight

1. Create and unlock one disposable identity per profile.
2. Confirm each profile can reach the required contacts and has call permission.
3. In OS privacy settings, grant Harbor microphone access on both hosts and camera access for video.
4. Record whether the route is LAN/direct, STUN, or TURN. For TURN, record only server hostname, transport, and whether relay-only mode was selected. Do not record the username or credential.
5. Confirm call history contains lifecycle metadata only. Do not enable WebRTC internals or verbose logging that captures SDP or ICE candidate bodies in release evidence.

## 4. Windows to macOS voice call

1. From Windows, start a voice call to the Mac and record the ring timestamp on both hosts.
2. Reject once on the Mac. Confirm Windows leaves ringing state and reports a normal declined outcome.
3. Call again and accept on the Mac. Confirm both hosts reach connected state and audio works in both directions.
4. Mute and unmute on each host. Confirm only the selected local microphone changes.
5. Hang up from Windows. Confirm both overlays close and one terminal history row appears per profile.
6. Fully quit and relaunch both apps with the same disposable profile directories. Confirm no active-call overlay, stream, permission prompt, or stale ICE error returns.

Repeat the accepted call with macOS as caller and Windows as callee.

## 5. Windows and macOS video call

1. Start and accept a video call in each direction.
2. Confirm both audio and video are present on both hosts.
3. Disable and re-enable the camera on each host without dropping audio.
4. Switch cameras where the host has more than one camera.
5. Deny camera permission once, while retaining microphone permission. Confirm Harbor offers/continues audio-only calling with an actionable camera message.
6. Restore camera permission, relaunch, and confirm a subsequent video call negotiates video.
7. Hang up from each platform once and confirm remote cleanup.

## 6. Three-profile group call and partial failure

1. Start a group video call with Windows, macOS, and the third isolated profile.
2. Confirm each remote tile rings and connects independently.
3. Verify audio and video between every pair, then mute/unmute and disable/re-enable the camera on each profile.
4. Disconnect the third profile. Confirm its tile becomes failed/degraded while the Windows to macOS leg remains connected and usable.
5. Rejoin or start a fresh room, then reject on the third profile. Confirm rejection does not end the remaining leg.
6. Leave from the creator. Confirm every remaining leg closes and relaunch shows no stale room or media stream.
7. If TURN is configured, repeat one accepted call from networks that require relay. Record the selected route class and connected/failed state only.

## 7. Evidence record

Create one record per attempt with these exact fields:

```text
attempt_id:
scenario: win_to_mac_voice | mac_to_win_voice | win_to_mac_video | mac_to_win_video | three_profile_group | turn_relay
release_tag:
commit_sha:
windows_package_name:
windows_package_sha256:
windows_version_arch:
windows_signature_status:
macos_package_name:
macos_package_sha256:
macos_version_arch:
macos_codesign_status:
macos_gatekeeper_status:
caller_profile:
callee_profiles:
route_class: lan_direct | stun | turn
relay_only: true | false
started_at_utc:
incoming_ring_at_utc:
accepted_or_rejected_at_utc:
connected_at_utc_each_profile:
first_bidirectional_audio_at_utc:
first_bidirectional_video_at_utc:
mute_toggle_result:
camera_toggle_result:
camera_denial_audio_fallback_result:
partial_failure_result:
hangup_at_utc_each_profile:
relaunch_cleanup_result:
history_metadata_only_result:
redacted_screenshot_or_log_paths:
result: pass | fail | blocked
failure_step:
follow_up_work_item:
tester:
```

A pass requires every applicable field and real packaged hardware observations. Automated tests, synthetic media, source builds, and a single operating system cannot close this acceptance gate.
