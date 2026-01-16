import { create } from "zustand";
import type { PeerInfo, NetworkStats, ConnectionStatus } from "../types";
import * as networkService from "../services/network";

interface NetworkState {
  // State
  isRunning: boolean;
  status: ConnectionStatus;
  connectedPeers: PeerInfo[];
  stats: NetworkStats;
  error: string | null;
  isLoading: boolean;

  // Actions
  startNetwork: () => Promise<void>;
  stopNetwork: () => Promise<void>;
  refreshPeers: () => Promise<void>;
  refreshStats: () => Promise<void>;
  checkStatus: () => Promise<void>;
}

const initialStats: NetworkStats = {
  connectedPeers: 0,
  totalBytesIn: 0,
  totalBytesOut: 0,
  uptimeSeconds: 0,
};

export const useNetworkStore = create<NetworkState>((set, get) => ({
  // Initial state
  isRunning: false,
  status: "disconnected",
  connectedPeers: [],
  stats: initialStats,
  error: null,
  isLoading: false,

  // Start the network
  startNetwork: async () => {
    set({ isLoading: true, error: null, status: "connecting" });
    try {
      await networkService.startNetwork();
      set({ isRunning: true, status: "connected", isLoading: false });
      // Refresh peers and stats after starting
      await get().refreshPeers();
      await get().refreshStats();
    } catch (error) {
      set({
        error: String(error),
        isLoading: false,
        status: "disconnected",
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
        status: "disconnected",
        connectedPeers: [],
        stats: initialStats,
        isLoading: false,
      });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  // Check if network is running
  checkStatus: async () => {
    try {
      const isRunning = await networkService.isNetworkRunning();
      set({
        isRunning,
        status: isRunning ? "connected" : "disconnected",
      });
    } catch (error) {
      set({ error: String(error) });
    }
  },

  // Refresh connected peers list
  refreshPeers: async () => {
    try {
      const peers = await networkService.getConnectedPeers();
      set({ connectedPeers: peers });
    } catch (error) {
      set({ error: String(error) });
    }
  },

  // Refresh network statistics
  refreshStats: async () => {
    try {
      const stats = await networkService.getNetworkStats();
      set({ stats });
    } catch (error) {
      set({ error: String(error) });
    }
  },
}));
