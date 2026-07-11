# Transitive dependency risk disposition — 2026-07-11

The release lock refresh upgraded Tauri to 2.11.5, `rustls-webpki` to 0.103.13,
`tar` to 0.4.46, and the current yamux line to 0.13.10. Seven GitHub alerts
remain in dependencies that Harbor cannot safely override independently.

| Dependency | Alerts | Ownership and demo disposition |
| --- | ---: | --- |
| `hickory-proto` 0.25.2 | 4 | Pulled by `libp2p-mdns` in latest `libp2p` 0.56. The NSEC3 and name-compression issues affect DNS parsing/encoding; Harbor uses this path for local mDNS discovery, not an authoritative or recursive DNS service. Accept for the bounded demo; prefer the configured relay and trusted demo LAN. Upgrade when libp2p moves to Hickory 0.26.1 or newer. |
| `yamux` 0.12.1 | 2 | A compatibility line pulled by latest `libp2p-yamux`; the active 0.13 line is fixed at 0.13.10. Malformed remote frames can panic a connection/process, so demo peers and relay should remain controlled. Accept for the bounded demo and upgrade with the next libp2p release that removes 0.12.1. |
| `glib` 0.18.5 | 1 | Pulled by Tauri's Linux WebKitGTK stack. The advisory concerns iterator unsoundness in an API Harbor does not call directly. A forced GLib major override is ABI-risky. Accept for the demo; update when Tauri/WebKitGTK moves to GLib 0.20 or newer. |

These are not declared resolved. They are time-bounded demo risk acceptances, not
general production waivers. Recheck Dependabot and upstream releases before the
next public release. Do not expose demo peers or relay ports beyond what the
documented deployment requires.
