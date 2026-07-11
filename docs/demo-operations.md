# Demo relay and TURN operations

Harbor uses two distinct relay layers. The libp2p relay carries discovery,
signaling, messages, and optional wall sync. A TURN server relays WebRTC media
when direct ICE connectivity fails. A Harbor relay is not a TURN server.

## Sunday demo configuration

1. In **Settings → Network**, use the Harbor community relay multiaddr from
   AWS SSM parameter `/harbor/harbor-community-relay/relay-address`. Confirm the
   complete value includes `/p2p/<peer-id>` before saving it.
2. For devices on the same LAN, leave call ICE servers empty and keep transport
   policy `all`. Host candidates provide the simplest demo path.
3. For devices behind different or strict NATs, enter an operator-owned
   `turn:` or `turns:` URL in **Settings → Calls**, plus its username and
   credential. Prefer short-lived credentials. Do not embed credentials in the
   URL or commit them. Select **This session only** on shared demo devices.
4. Before presenting, verify both devices show the expected peer and relay
   connection. Place a short test call and confirm connected media, then hang up.

## Observability and recovery

- Relay host: `systemctl status harbor-community-relay` and
  `journalctl -u harbor-community-relay --since '15 minutes ago'`.
- Relay address: `aws ssm get-parameter --name
  /harbor/harbor-community-relay/relay-address --query Parameter.Value --output
  text`.
- Harbor: use the Network status and call-state UI. Logs may show peer IDs,
  signaling state, ICE state, and relay connection state; they must not print
  TURN credentials or private message/media bodies.
- If LAN calling works but a strict-NAT call fails, verify the TURN URL,
  credential lifetime, UDP/TCP reachability, and that TURN is selected as an
  ICE candidate. Changing the Harbor libp2p relay cannot repair media ICE.
- If peer signaling fails, verify the libp2p relay multiaddr and peer ID first;
  TURN does not carry Harbor signaling.

Rotate TURN credentials after the demo. For a long-lived deployment, use a TURN
server that issues time-limited credentials and monitor allocation failures,
bandwidth, and credential expiry without logging credential material.
