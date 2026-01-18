/** Network connection status */
export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected';

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
  | { type: 'contact_added'; peerId: string; displayName: string };
