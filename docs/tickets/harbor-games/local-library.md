# Harbor validated local game library

Source inputs:

- `docs/tickets/harbor-games/source-spec.md`
- Neo Grounds `.harborgame` v1 contract and fixtures at commit `7607d5c`
- Neo Grounds moderated store metadata at commit `7cb320e`

## Validation and installation

Harbor's Rust implementation independently parses the checked v1 fixture. Before installation it validates:

- the bounded binary archive, canonical index, contiguous ranges, UTF-8/NFC relative paths, path ordering, duplicates, and every SHA-256;
- canonical `runtime.json`, `assets.json`, and `package.sig` JSON;
- exact v1 package, ABI, renderer, host-integration, artifact, digest, and signature metadata;
- declared files and artifact lengths/digests, with no undeclared file;
- normalized manifest and package digests;
- creator PeerId derivation and Ed25519 signature;
- exact user-approved permissions;
- for store downloads, API/package metadata agreement and the separate Ed25519 approval attestation.

The store client has a compiled exact origin, refuses redirects, and bounds streamed metadata and package responses. Harbor pins the first cryptographically valid approval public key received over the trusted HTTPS origin in the profile database. A later key change fails and requires an explicit trust-reset release procedure.

Validated bytes are copied to `<profile>/games/packages/<archive-sha256>.harborgame` with create-only writes. Harbor revalidates managed bytes before launch. The discovery folder is only a machine-visible source; files never execute in place. Discovery is non-recursive and rejects symlinks, special files, invalid extensions, and invalid packages independently.

Installation metadata, permission approval, store attestation, play count, and saves are profile-scoped. Updates replace the active database record only after complete validation and immutable copy. A failed update leaves the installed record unchanged. Uninstall removes the runnable record but intentionally preserves saves until the user separately deletes them.

## Resource bounds

Observed source artifacts on 2026-09-08 were 2,855 bytes for the canonical archive and 4,327 bytes for the largest current browser player/build sample.

- Package: 8 MiB, over 1,900 times the largest observed sample. Protects download memory, parser work, and package storage.
- Canonical index: 256 KiB. Protects JSON parsing and path metadata memory.
- Files per archive: 1,024. Protects path/hash iteration and extraction work.
- Individual file: 8 MiB. Prevents one entry from bypassing the package bound.
- Installed profile library: 1 GiB, exactly 128 maximum-size packages. Protects profile storage while allowing many measured-size games.
- Save slot: 1 MiB. The current demonstrated platform payload (`{"level":4}`) is 11 bytes; this leaves over 95,000 times that observed payload while bounding SQLite allocation and Worker/IPC transfer.

These values are declared beside the failure modes they protect. Production telemetry must justify any increase; limits are not silently relaxed.
