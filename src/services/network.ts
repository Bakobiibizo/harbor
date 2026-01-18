import { invoke } from "@tauri-apps/api/core";
import type { PeerInfo, NetworkStats } from "../types";

/** Start the P2P network (requires unlocked identity) */
export async function startNetwork(): Promise<void> {
  return invoke("start_network");
}

/** Stop the P2P network */
export async function stopNetwork(): Promise<void> {
  return invoke("stop_network");
}

/** Check if the network is running */
export async function isNetworkRunning(): Promise<boolean> {
  return invoke("is_network_running");
}

/** Get list of connected peers */
export async function getConnectedPeers(): Promise<PeerInfo[]> {
  return invoke("get_connected_peers");
}

/** Get network statistics */
export async function getNetworkStats(): Promise<NetworkStats> {
  return invoke("get_network_stats");
}

/** Bootstrap the DHT */
export async function bootstrapNetwork(): Promise<void> {
  return invoke("bootstrap_network");
}

/** Get listening addresses (for sharing with remote peers) */
export async function getListeningAddresses(): Promise<string[]> {
  return invoke("get_listening_addresses");
}

/** Connect to a peer by multiaddress */
export async function connectToPeer(multiaddr: string): Promise<void> {
  return invoke("connect_to_peer", { multiaddr });
}

/** Add a bootstrap node address */
export async function addBootstrapNode(multiaddr: string): Promise<void> {
  return invoke("add_bootstrap_node", { multiaddr });
}
