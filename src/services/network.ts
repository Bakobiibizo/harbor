import type { AddContactResult, PeerInfo, NetworkStats } from '../types';
import { normalizeContactInvite } from '../utils/contactInvite';
import { invokeCommand } from './command';

export interface NetworkStartupConfig {
  enableMdns?: boolean;
  bootstrapNodes?: string[];
}

/** Start the P2P network (requires unlocked identity) */
export async function startNetwork(config: NetworkStartupConfig = {}): Promise<void> {
  return invokeCommand('start_network', config);
}

/** Stop the P2P network */
export async function stopNetwork(): Promise<void> {
  return invokeCommand('stop_network');
}

/** Check if the network is running */
export async function isNetworkRunning(): Promise<boolean> {
  return invokeCommand('is_network_running');
}

/** Get list of connected peers */
export async function getConnectedPeers(): Promise<PeerInfo[]> {
  return invokeCommand('get_connected_peers');
}

/** Get network statistics */
export async function getNetworkStats(): Promise<NetworkStats> {
  return invokeCommand('get_network_stats');
}

/** Bootstrap the DHT */
export async function bootstrapNetwork(): Promise<void> {
  return invokeCommand('bootstrap_network');
}

/** Get listening addresses (for sharing with remote peers) */
export async function getListeningAddresses(): Promise<string[]> {
  return invokeCommand('get_listening_addresses');
}

/** Connect to a peer by multiaddress */
export async function connectToPeer(multiaddr: string): Promise<void> {
  return invokeCommand('connect_to_peer', { multiaddr });
}

/** Add a bootstrap node address */
export async function addBootstrapNode(multiaddr: string): Promise<void> {
  return invokeCommand('add_bootstrap_node', { multiaddr });
}

/** Add a relay server by multiaddress */
export async function addRelayServer(multiaddr: string): Promise<void> {
  return invokeCommand('add_relay_server', { multiaddr });
}

/** Connect to public/default relay servers */
export async function connectToPublicRelays(): Promise<void> {
  return invokeCommand('connect_to_public_relays');
}

/** Get current NAT status */
export async function getNatStatus(): Promise<string> {
  return invokeCommand('get_nat_status');
}

/** Get shareable addresses (relay addresses that work globally) */
export async function getShareableAddresses(): Promise<string[]> {
  return invokeCommand('get_shareable_addresses');
}

/** Get canonical public discovery metadata for starting a signed contact handshake. */
export async function getShareableContactString(): Promise<string> {
  return invokeCommand('get_shareable_contact_string');
}

/** Start a signed contact request from a canonical v1 invite. */
export async function addContactFromString(contactString: string): Promise<AddContactResult> {
  return invokeCommand('add_contact_from_string', {
    contactString: normalizeContactInvite(contactString),
  });
}

export async function syncFeed(limit?: number): Promise<void> {
  return invokeCommand('sync_feed', { limit });
}
