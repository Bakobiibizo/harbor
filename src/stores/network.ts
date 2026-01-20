import { create } from 'zustand';
import type { PeerInfo, NetworkStats, ConnectionStatus, NatStatus } from '../types';
import * as networkService from '../services/network';

interface NetworkState {
  // State
  isRunning: boolean;
  status: ConnectionStatus;
  connectedPeers: PeerInfo[];
  stats: NetworkStats;
  listeningAddresses: string[];
  error: string | null;
  isLoading: boolean;

  // Actions
  startNetwork: () => Promise<void>;
  stopNetwork: () => Promise<void>;
  refreshPeers: () => Promise<void>;
  refreshStats: () => Promise<void>;
  refreshAddresses: () => Promise<void>;
  checkStatus: () => Promise<void>;
  connectToPeer: (multiaddr: string) => Promise<void>;
  addBootstrapNode: (multiaddr: string) => Promise<void>;
  // NAT status update (called by event handler)
  setNatStatus: (status: NatStatus) => void;
  addRelayAddress: (address: string) => void;
}

const initialStats: NetworkStats = {
  connectedPeers: 0,
  totalBytesIn: 0,
  totalBytesOut: 0,
  uptimeSeconds: 0,
  natStatus: 'unknown',
  relayAddresses: [],
  externalAddresses: [],
};

export const useNetworkStore = create<NetworkState>((set, get) => ({
  // Initial state
  isRunning: false,
  status: 'disconnected',
  connectedPeers: [],
  stats: initialStats,
  listeningAddresses: [],
  error: null,
  isLoading: false,

  // Start the network
  startNetwork: async () => {
    set({ isLoading: true, error: null, status: 'connecting' });
    try {
      await networkService.startNetwork();
      set({ isRunning: true, status: 'connected', isLoading: false });
      // Refresh peers, stats, and addresses after starting
      await get().refreshPeers();
      await get().refreshStats();
      await get().refreshAddresses();
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error),
        isLoading: false,
        status: 'disconnected',
      });
    }
  },

  // Stop the network
  stopNetwork: async () => {
    set({ isLoading: true, error: null });
    try {
      await networkService.stopNetwork();
      set({
        isRunning: false,
        status: 'disconnected',
        connectedPeers: [],
        stats: initialStats,
        listeningAddresses: [],
        isLoading: false,
      });
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error), isLoading: false });
    }
  },

  // Check if network is running
  checkStatus: async () => {
    try {
      const isRunning = await networkService.isNetworkRunning();
      set({
        isRunning,
        status: isRunning ? 'connected' : 'disconnected',
      });
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
    }
  },

  // Refresh connected peers list
  refreshPeers: async () => {
    try {
      const peers = await networkService.getConnectedPeers();
      set({ connectedPeers: peers });
    } catch (error) {
      // Don't show error for refresh failures - just log it
      console.error('Failed to refresh peers:', error);
    }
  },

  // Refresh network statistics
  refreshStats: async () => {
    try {
      const stats = await networkService.getNetworkStats();
      set({ stats });
    } catch (error) {
      // Don't show error for refresh failures - just log it
      console.error('Failed to refresh stats:', error);
    }
  },

  // Refresh listening addresses
  refreshAddresses: async () => {
    try {
      const addresses = await networkService.getListeningAddresses();
      set({ listeningAddresses: addresses });
    } catch (error) {
      console.error('Failed to refresh addresses:', error);
    }
  },

  // Connect to a peer by multiaddress
  connectToPeer: async (multiaddr: string) => {
    set({ isLoading: true, error: null });
    try {
      await networkService.connectToPeer(multiaddr);
      set({ isLoading: false });
      // Refresh peers after connecting
      await get().refreshPeers();
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error), isLoading: false });
      throw error;
    }
  },

  // Add a bootstrap node
  addBootstrapNode: async (multiaddr: string) => {
    set({ isLoading: true, error: null });
    try {
      await networkService.addBootstrapNode(multiaddr);
      set({ isLoading: false });
      // Refresh peers after adding bootstrap
      await get().refreshPeers();
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error), isLoading: false });
      throw error;
    }
  },

  // Update NAT status (called by event handler)
  setNatStatus: (status: NatStatus) => {
    set((state) => ({
      stats: { ...state.stats, natStatus: status },
    }));
  },

  // Add a relay address (called by event handler)
  addRelayAddress: (address: string) => {
    set((state) => {
      if (state.stats.relayAddresses.includes(address)) {
        return state;
      }
      return {
        stats: {
          ...state.stats,
          relayAddresses: [...state.stats.relayAddresses, address],
        },
        // Also add to listening addresses (relay addresses should be first)
        listeningAddresses: state.listeningAddresses.includes(address)
          ? state.listeningAddresses
          : [address, ...state.listeningAddresses],
      };
    });
  },
}));
