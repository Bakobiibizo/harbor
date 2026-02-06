/** Network connection status */
export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected';

/** NAT status detected by AutoNAT */
export type NatStatus = 'unknown' | 'public' | 'private' | 'behind_nat';

/** Information about a peer */
export interface PeerInfo {
  peerId: string;
  addresses: string[];
  protocolVersion: string | null;
  agentVersion: string | null;
  isConnected: boolean;
  lastSeen: number | null;
}

/** Network statistics */
export interface NetworkStats {
  connectedPeers: number;
  totalBytesIn: number;
  totalBytesOut: number;
  uptimeSeconds: number;
  /** Current NAT status */
  natStatus: NatStatus;
  /** Relay addresses we can be reached at (via relay) */
  relayAddresses: string[];
  /** External addresses discovered via AutoNAT */
  externalAddresses: string[];
}

/** Network events emitted by the backend */
export type NetworkEvent =
  | { type: 'peer_discovered'; peerId: string }
  | { type: 'peer_expired'; peerId: string }
  | { type: 'peer_connected'; peerId: string }
  | { type: 'peer_disconnected'; peerId: string }
  | { type: 'external_address_discovered'; address: string }
  | { type: 'listening_on'; address: string }
  | { type: 'message_received'; peerId: string; protocol: string; payload: number[] }
  | { type: 'status_changed'; status: ConnectionStatus }
  | { type: 'contact_added'; peerId: string; displayName: string }
  | { type: 'nat_status_changed'; status: NatStatus }
  | { type: 'relay_connected'; relayAddress: string }
  | { type: 'hole_punch_succeeded'; peerId: string }
  | { type: 'content_manifest_received'; peerId: string; postCount: number; hasMore: boolean }
  | { type: 'content_fetched'; peerId: string; postId: string }
  | { type: 'content_sync_error'; peerId: string; error: string };
