# Harbor games deployment status

Observed: 2026-09-08

Inputs:

- Harbor `games` at `c799dfe`
- Neo Grounds `harbor-games` at `a15d01b`
- work item `games-0500-deploy-store`, run 88

## Deployed and verified

The designated host runs Neo Grounds from an immutable release directory as a dedicated non-login service account. The system service and daily persistent backup timer are enabled.

Verified state:

- origin listener: `127.0.0.1:8090` only;
- local health: durable API and store report healthy;
- static store: restrictive CSP, frame ancestors, capability policy, and `nosniff` present;
- durable directories: separate metadata, pending, approved, key, and backup paths under `/mnt/nas/neo-grounds`;
- backup: create-only production backup completed;
- systemd hardening: `systemd-analyze security` rated the service `OK`;
- tunnel: games ingress validates and targets the loopback origin;
- regression: both unrelated pre-existing tunnel sites returned HTTP 200 before and after tunnel restart;
- Neo Grounds Node 24 checks: formatting, typecheck, lint, 218 tests, and build passed on the host.

No service or infrastructure credential was written to a repository, command output, deployment report, or application log.

## Blocking external state

`games.social-harbor.com` is authoritative under an externally managed DNS zone and still has its prior CNAME to the existing web provider. The deployment host has neither an origin certificate nor scoped authoritative-DNS API authority to replace that record. Public HTTPS therefore does not reach the deployed service and cannot pass acceptance.

The previously exposed infrastructure credential has not been confirmed rotated. Rotation remains a required acceptance condition; this report does not reinterpret it as a documented limitation.

Follow-up `games-0501-public-dns-tls` is held until the operator:

1. confirms rotation of the previously exposed credential; and
2. either performs the authoritative DNS/TLS cutover or grants a narrowly scoped mechanism that can update only the games hostname.

After that change, validation must prove valid HTTPS store/API responses, exact framing and CORS headers, loopback-only origin binding, preserved unrelated sites, restart persistence, and rollback readiness. Only then may `games-0600-end-to-end-validation` start.
