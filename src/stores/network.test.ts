import { describe, it, expect, vi, beforeEach } from "vitest";
import { useNetworkStore } from "./network";
import * as networkService from "../services/network";

vi.mock("../services/network", () => ({
  startNetwork: vi.fn(),
  stopNetwork: vi.fn(),
  isNetworkRunning: vi.fn(),
  getConnectedPeers: vi.fn(),
  getNetworkStats: vi.fn(),
  getListeningAddresses: vi.fn(),
  connectToPeer: vi.fn(),
  addBootstrapNode: vi.fn(),
}));

const mockStats = {
  connectedPeers: 5,
  totalBytesIn: 1000,
  totalBytesOut: 2000,
  uptimeSeconds: 3600,
  natStatus: "public" as const,
  relayAddresses: [],
  externalAddresses: [],
};

const mockPeers = [
  {
    peerId: "12D3KooWPeer1",
    addresses: ["/ip4/192.168.1.1/tcp/9000"],
    protocolVersion: "harbor/1.0.0",
    agentVersion: "harbor/0.1.0",
    isConnected: true,
    lastSeen: Date.now(),
  },
];

describe("useNetworkStore", () => {
  beforeEach(() => {
    useNetworkStore.setState({
      isRunning: false,
      status: "disconnected",
      connectedPeers: [],
      stats: {
        connectedPeers: 0,
        totalBytesIn: 0,
        totalBytesOut: 0,
        uptimeSeconds: 0,
        natStatus: "unknown",
        relayAddresses: [],
        externalAddresses: [],
      },
      listeningAddresses: [],
      error: null,
      isLoading: false,
    });
    vi.clearAllMocks();
  });

  describe("startNetwork", () => {
    it("should start network and update state", async () => {
      vi.mocked(networkService.startNetwork).mockResolvedValue(undefined);
      vi.mocked(networkService.getConnectedPeers).mockResolvedValue(mockPeers);
      vi.mocked(networkService.getNetworkStats).mockResolvedValue(mockStats);
      vi.mocked(networkService.getListeningAddresses).mockResolvedValue([
        "/ip4/0.0.0.0/tcp/9000",
      ]);

      await useNetworkStore.getState().startNetwork();

      expect(useNetworkStore.getState().isRunning).toBe(true);
      expect(useNetworkStore.getState().status).toBe("connected");
      expect(useNetworkStore.getState().connectedPeers).toEqual(mockPeers);
    });

    it("should handle start failure", async () => {
      vi.mocked(networkService.startNetwork).mockRejectedValue(
        new Error("Network error")
      );

      await useNetworkStore.getState().startNetwork();

      expect(useNetworkStore.getState().isRunning).toBe(false);
      expect(useNetworkStore.getState().status).toBe("disconnected");
      expect(useNetworkStore.getState().error).toBe("Network error");
    });
  });

  describe("stopNetwork", () => {
    it("should stop network and reset state", async () => {
      useNetworkStore.setState({
        isRunning: true,
        status: "connected",
        connectedPeers: mockPeers,
      });
      vi.mocked(networkService.stopNetwork).mockResolvedValue(undefined);

      await useNetworkStore.getState().stopNetwork();

      expect(useNetworkStore.getState().isRunning).toBe(false);
      expect(useNetworkStore.getState().status).toBe("disconnected");
      expect(useNetworkStore.getState().connectedPeers).toEqual([]);
    });
  });

  describe("checkStatus", () => {
    it("should update status based on network state", async () => {
      vi.mocked(networkService.isNetworkRunning).mockResolvedValue(true);

      await useNetworkStore.getState().checkStatus();

      expect(useNetworkStore.getState().isRunning).toBe(true);
      expect(useNetworkStore.getState().status).toBe("connected");
    });

    it("should handle offline state", async () => {
      vi.mocked(networkService.isNetworkRunning).mockResolvedValue(false);

      await useNetworkStore.getState().checkStatus();

      expect(useNetworkStore.getState().isRunning).toBe(false);
      expect(useNetworkStore.getState().status).toBe("disconnected");
    });
  });

  describe("connectToPeer", () => {
    it("should connect to peer and refresh list", async () => {
      vi.mocked(networkService.connectToPeer).mockResolvedValue(undefined);
      vi.mocked(networkService.getConnectedPeers).mockResolvedValue(mockPeers);

      await useNetworkStore.getState().connectToPeer("/ip4/1.2.3.4/tcp/9000/p2p/12D3KooW");

      expect(networkService.connectToPeer).toHaveBeenCalledWith(
        "/ip4/1.2.3.4/tcp/9000/p2p/12D3KooW"
      );
      expect(useNetworkStore.getState().connectedPeers).toEqual(mockPeers);
    });

    it("should throw on connection failure", async () => {
      vi.mocked(networkService.connectToPeer).mockRejectedValue(
        new Error("Connection failed")
      );

      await expect(
        useNetworkStore.getState().connectToPeer("/ip4/1.2.3.4/tcp/9000")
      ).rejects.toThrow("Connection failed");

      expect(useNetworkStore.getState().error).toBe("Connection failed");
    });
  });

  describe("setNatStatus", () => {
    it("should update NAT status", () => {
      useNetworkStore.getState().setNatStatus("public");

      expect(useNetworkStore.getState().stats.natStatus).toBe("public");
    });
  });

  describe("addRelayAddress", () => {
    it("should add relay address without duplicates", () => {
      useNetworkStore.getState().addRelayAddress("/p2p-circuit/relay1");
      useNetworkStore.getState().addRelayAddress("/p2p-circuit/relay1");
      useNetworkStore.getState().addRelayAddress("/p2p-circuit/relay2");

      const state = useNetworkStore.getState();
      expect(state.stats.relayAddresses).toHaveLength(2);
      expect(state.stats.relayAddresses).toContain("/p2p-circuit/relay1");
      expect(state.stats.relayAddresses).toContain("/p2p-circuit/relay2");
    });
  });
});
