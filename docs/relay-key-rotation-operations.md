# Relay signing-key rotation and recovery

Harbor pins the Ed25519 authority key for each relay namespace. A relay must never replace its identity key without either a successor record signed by the currently pinned key or an explicit recovery decision on every affected client.

## Planned rotation

1. Back up the current relay identity file offline. The default is `~/.config/harbor-relay/id.key`; AWS deployments keep the equivalent file on the relay instance. Never commit or transmit this file.
2. On an offline administrative machine, build the helper and create the successor key plus signed public record. If `successor.key` does not exist, the helper creates it with mode `0600` on Unix. Times are Unix seconds.

   ```bash
   cargo run --manifest-path relay-server/Cargo.toml --bin relay-key-rotation -- \
     --current-key /secure/current-id.key \
     --next-key /secure/successor.key \
     --relay relay.example.com \
     --previous-key-id relay-key-1 \
     --next-key-id relay-key-2 \
     --sequence 1 \
     --not-before 1783861200 \
     --not-after 1815397200 \
     --output relay-key-1-to-2.json
   ```

3. Inspect the JSON record. It contains the namespace, key IDs, successor 32-byte Ed25519 public key, validity window, issue time, monotonic sequence, and `previous_key_signature`. Set `--compromise-from` only when claims signed after a known incident time must be invalidated. Neither private key is written to the record.
4. Record the JSON file's SHA-256 digest. Keep the successor private key encrypted and offline until the cutover.
5. Publish the signed record through the same authenticated release channel used for the relay address. Retain its SHA-256 digest and the old/new peer IDs in the operator change record.
6. Before replacing the relay identity file, import the signed record in Harbor. The application command is `apply_relay_key_rotation` with a `signedRotation` argument. Harbor verifies the pinned key, namespace, validity window, sequence, signature, and successor key before atomically retiring the old pin.
7. Replace the relay key, update the relay multiaddress because its peer ID changes, restart the service, and verify name registration plus the two-relay identity release gate.

Harbor rejects a new key ID returned during ordinary registration while another key for that namespace remains active. A planned rotation therefore fails closed if the signed record was skipped, altered, expired, or applied out of order.

## Emergency recovery

If the current private key is unavailable or compromised and cannot sign a successor:

1. Stop the relay and preserve its database, logs, old public key, last known key ID, and incident timestamps.
2. Generate a new relay key offline and publish the new peer ID plus an incident notice over an independently authenticated channel controlled by the namespace owner.
3. Set the compromise boundary to the earliest time the old key may have been exposed. Claims at or after that boundary must be treated as untrusted.
4. Do not ask users to edit SQLite or overwrite a pin silently. Harbor intentionally has no remote emergency-override API. Each user must explicitly remove and re-add the affected relay after comparing the namespace, new peer ID, and public-key fingerprint with the independent notice.
5. Re-register affected names only under a separately reviewed recovery policy. Version 1 does not silently reassign retired names.
6. Run `scripts/validate-relay-identity-release.sh` and retain the sanitized evidence before restoring production service.

An operator who cannot provide an independently verifiable notice must deploy under a new namespace. Availability loss is preferable to making an attacker-controlled relay key authoritative.

## Secret handling

- Store current, successor, and recovery private material outside the repository and deployment logs.
- Rotation records contain public keys and signatures only.
- Never paste session tokens, private identity files, decrypted introductions, or contact lists into tickets or release evidence.
- Keep the previous public key and signed rotation record for audit; securely destroy retired private material after the rollback window closes.
