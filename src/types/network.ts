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

/** Network events emitted by the backend.
 *
 * Field names are snake_case to match the Rust serde output.
 * Rust uses `#[serde(tag = "type", rename_all = "snake_case")]` which
 * renames variant names but NOT struct fields.
 */
export type NetworkEvent =
  | { type: 'peer_discovered'; peer_id: string }
  | { type: 'peer_expired'; peer_id: string }
  | { type: 'peer_connected'; peer_id: string }
  | { type: 'peer_disconnected'; peer_id: string }
  | { type: 'external_address_discovered'; address: string }
  | { type: 'listening_on'; address: string }
  | { type: 'message_received'; peer_id: string; protocol: string; payload: number[] }
  | { type: 'status_changed'; status: ConnectionStatus }
  | { type: 'contact_added'; peer_id: string; display_name: string }
  | { type: 'nat_status_changed'; status: NatStatus }
  | { type: 'relay_connected'; relay_address: string }
  | { type: 'hole_punch_succeeded'; peer_id: string }
  | { type: 'content_manifest_received'; peer_id: string; post_count: number; has_more: boolean }
  | { type: 'content_fetched'; peer_id: string; post_id: string }
  | { type: 'content_sync_error'; peer_id: string; error: string }
  | { type: 'wall_post_synced'; relay_peer_id: string; post_id: string }
  | { type: 'wall_posts_received'; relay_peer_id: string; author_peer_id: string; post_count: number }
  | { type: 'wall_post_deleted_on_relay'; relay_peer_id: string; post_id: string }
  | { type: 'media_fetched'; peer_id: string; media_hash: string };
