# Harbor games signing bridge v1

Source: `source-spec.md` and Neo Grounds `.harborgame` contract at `hydra-dynamix/neo-grounds@7607d5c`.

Harbor accepts only two versioned deep-link forms:

```text
harbor://games/auth/<base64url-json>
harbor://games/package/<base64url-json>
```

The decoded JSON is limited to 4096 bytes. The complete URL is limited to 8192 bytes. Both request objects reject unknown fields. IDs contain 1–128 ASCII letters, digits, dots, hyphens, or underscores. Requests tolerate at most 30 seconds of future clock skew.

## Authentication request

```json
{
  "accountId": "account-1",
  "audience": "https://games.social-harbor.com",
  "callbackUrl": "https://games.social-harbor.com/api/auth/harbor/challenges/challenge-1/proof",
  "challengeId": "challenge-1",
  "expiresAt": 1788825720,
  "issuedAt": 1788825600,
  "nonce": "<32 random bytes as lowercase hex>",
  "requestId": "auth-request-1",
  "version": 1
}
```

Authentication requests live for at most five minutes. Harbor signs canonical compact JSON with fields in lexical order after adding its peer ID, lowercase-hex Ed25519 public key, and domain `harbor.games.auth.v1`. It posts the resulting `harbor-game-auth-proof` object directly to the exact callback endpoint.

## Package request

```json
{
  "callbackUrl": "https://games.social-harbor.com/api/harbor-store/signing-requests/package-request-1/proof",
  "expiresAt": 1788825900,
  "gameId": "game-1",
  "issuedAt": 1788825600,
  "packageDigest": "<SHA-256 as lowercase hex>",
  "permissions": ["save_data"],
  "requestId": "package-request-1",
  "title": "Game title",
  "version": 1,
  "versionId": "version-1"
}
```

Package requests live for at most ten minutes. Permissions must be supported, unique, and sorted. Harbor signs the exact `harbor.game-package.v1` payload defined by Neo Grounds and posts the resulting `harbor-game-creator-signature` envelope directly to the exact callback endpoint.

## Approval and replay behavior

Rust validates and retains a request under a random opaque approval ID. The frontend receives only the bounded presentation and can approve only that ID; no Tauri command accepts arbitrary bytes or a caller-built payload. Harbor displays the origin/account or package title, digest, permissions, version, request ID, expiry, and selected identity before signing.

A profile-scoped SQLite row reserves each domain/request-ID pair with the exact callback and proof before network delivery. Concurrent, changed, or delivered replays fail. Failed delivery retains the deterministic public proof for an explicit retry. The HTTP client refuses redirects and uses a ten-second timeout, so a trusted endpoint cannot redirect identity proofs to another origin. Private key material never enters request, response, database, frontend, or logs.
