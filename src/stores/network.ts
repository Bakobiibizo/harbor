import { create } from 'zustand';
import type { PeerInfo, NetworkStats, ConnectionStatus, NatStatus } from '../types';
import * as networkService from '../services/network';
import { getErrorMessage } from '../utils/errors';
import { useSettingsStore } from './settings';

export type RelayStatus = 'disconnected' | 'connecting' | 'connected';

interface NetworkState {
  // State
  isRunning: boolean;
  status: ConnectionStatus;
  connectedPeers: PeerInfo[];
  stats: NetworkStats;
  listeningAddresses: string[];
  shareableAddresses: string[];
  relayStatus: RelayStatus;
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
  connectToRelay: (multiaddr: string) => Promise<void>;
  connectToPublicRelays: () => Promise<void>;
  refreshShareableAddresses: () => Promise<void>;
  setRelayStatus: (status: RelayStatus) => void;
  // NAT status update (called by event handler)
  setNatStatus: (status: NatStatus) => void;
  addRelayAddress: (address: string) => void;
  // Deep-link contact pending user confirmation
  pendingDeepLinkContact: string | null;
  setPendingDeepLinkContact: (contact: string | null) => void;
  reset: () => void;
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

let lifecycleGeneration = 0;

const resetState = {
  isRunning: false,
  status: 'disconnected' as const,
  connectedPeers: [] as PeerInfo[],
  stats: initialStats,
  listeningAddresses: [] as string[],
  shareableAddresses: [] as string[],
  relayStatus: 'disconnected' as const,
  error: null,
  isLoading: false,
  pendingDeepLinkContact: null,
};

export const useNetworkStore = create<NetworkState>((set, get) => ({
  ...resetState,

  // Start the network
  startNetwork: async () => {
    const generation = lifecycleGeneration;
    set({ isLoading: true, error: null, status: 'connecting' });
    try {
      const { localDiscovery, bootstrapNodes } = useSettingsStore.getState();
      await networkService.startNetwork({ enableMdns: localDiscovery, bootstrapNodes });
      if (generation !== lifecycleGeneration) return;
      // Backend auto-connects to relay, set UI to show connecting state
      set({ isRunning: true, status: 'connected', isLoading: false, relayStatus: 'connecting' });
      // Refresh peers, stats, and addresses after starting
      await get().refreshPeers();
      await get().refreshStats();
      await get().refreshAddresses();
    } catch (error) {
      if (generation === lifecycleGeneration)
        set({
          error: getErrorMessage(error),
          isLoading: false,
          status: 'disconnected',
        });
      throw error;
    }
  },

  // Stop the network
  stopNetwork: async () => {
    const generation = lifecycleGeneration;
    set({ isLoading: true, error: null });
    try {
      await networkService.stopNetwork();
      if (generation !== lifecycleGeneration) return;
      set({
        isRunning: false,
        status: 'disconnected',
        connectedPeers: [],
        stats: initialStats,
        listeningAddresses: [],
        shareableAddresses: [],
        relayStatus: 'disconnected',
        isLoading: false,
      });
    } catch (error) {
      if (generation === lifecycleGeneration)
        set({ error: getErrorMessage(error), isLoading: false });
      throw error;
    }
  },

  // Check if network is running
  checkStatus: async () => {
    const generation = lifecycleGeneration;
    try {
      const isRunning = await networkService.isNetworkRunning();
      if (generation !== lifecycleGeneration) return;
      set({
        isRunning,
        status: isRunning ? 'connected' : 'disconnected',
      });
    } catch (error) {
      if (generation === lifecycleGeneration)
        set({ error: getErrorMessage(error) });
    }
  },

  // Refresh connected peers list
  refreshPeers: async () => {
    const generation = lifecycleGeneration;
    try {
      const peers = await networkService.getConnectedPeers();
      if (generation === lifecycleGeneration) set({ connectedPeers: peers });
    } catch (error) {
      // Don't show error for refresh failures - just log it
      console.error('Failed to refresh peers:', error);
    }
  },

  // Refresh network statistics
  refreshStats: async () => {
    const generation = lifecycleGeneration;
    try {
      const stats = await networkService.getNetworkStats();
      if (generation === lifecycleGeneration) set({ stats });
    } catch (error) {
      // Don't show error for refresh failures - just log it
      console.error('Failed to refresh stats:', error);
    }
  },

  // Refresh listening addresses
  refreshAddresses: async () => {
    const generation = lifecycleGeneration;
    try {
      const addresses = await networkService.getListeningAddresses();
      if (generation === lifecycleGeneration) set({ listeningAddresses: addresses });
    } catch (error) {
      console.error('Failed to refresh addresses:', error);
    }
  },

  // Connect to a peer by multiaddress
  connectToPeer: async (multiaddr: string) => {
    const generation = lifecycleGeneration;
    set({ isLoading: true, error: null });
    try {
      await networkService.connectToPeer(multiaddr);
      if (generation !== lifecycleGeneration) return;
      set({ isLoading: false });
      // Refresh peers after connecting
      await get().refreshPeers();
    } catch (error) {
      if (generation === lifecycleGeneration)
        set({ error: getErrorMessage(error), isLoading: false });
      throw error;
    }
  },

  // Add a bootstrap node
  addBootstrapNode: async (multiaddr: string) => {
    const generation = lifecycleGeneration;
    set({ isLoading: true, error: null });
    try {
      await networkService.addBootstrapNode(multiaddr);
      if (generation !== lifecycleGeneration) return;
      set({ isLoading: false });
      // Refresh peers after adding bootstrap
      await get().refreshPeers();
    } catch (error) {
      if (generation === lifecycleGeneration)
        set({ error: getErrorMessage(error), isLoading: false });
      throw error;
    }
  },

  // Connect to a specific relay server
  connectToRelay: async (multiaddr: string) => {
    const generation = lifecycleGeneration;
    set({ relayStatus: 'connecting', error: null });
    try {
      await networkService.addRelayServer(multiaddr);
      if (generation !== lifecycleGeneration) return;
      // Relay status will be set to 'connected' by the relay_connected event handler
      await get().refreshAddresses();
      await get().refreshShareableAddresses();
    } catch (error) {
      if (generation === lifecycleGeneration)
        set({
          relayStatus: 'disconnected',
          error: getErrorMessage(error),
        });
      throw error;
    }
  },

  // Connect to public/default Harbor relays
  connectToPublicRelays: async () => {
    const generation = lifecycleGeneration;
    set({ relayStatus: 'connecting', error: null });
    try {
      await networkService.connectToPublicRelays();
      if (generation !== lifecycleGeneration) return;
      // Relay status will be set to 'connected' by the relay_connected event handler
      await get().refreshAddresses();
      await get().refreshShareableAddresses();
    } catch (error) {
      if (generation === lifecycleGeneration)
        set({
          relayStatus: 'disconnected',
          error: getErrorMessage(error),
        });
      throw error;
    }
  },

  // Refresh shareable addresses (relay addresses usable by remote peers)
  refreshShareableAddresses: async () => {
    const generation = lifecycleGeneration;
    try {
      const addresses = await networkService.getShareableAddresses();
      if (generation === lifecycleGeneration) set({ shareableAddresses: addresses });
    } catch (error) {
      console.error('Failed to refresh shareable addresses:', error);
    }
  },

  // Set relay status (called by event handler)
  setRelayStatus: (status: RelayStatus) => {
    set({ relayStatus: status });
  },

  // Update NAT status (called by event handler)
  setNatStatus: (status: NatStatus) => {
    set((state) => ({
      stats: { ...state.stats, natStatus: status },
    }));
  },

  // Set or clear the contact string awaiting confirmation from a deep link
  setPendingDeepLinkContact: (contact) => set({ pendingDeepLinkContact: contact }),

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
  reset: () => {
    lifecycleGeneration += 1;
    set({ ...resetState, stats: { ...initialStats }, connectedPeers: [] });
  },
}));
