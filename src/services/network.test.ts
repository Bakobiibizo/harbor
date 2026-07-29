import { describe, it, expect, vi, beforeEach } from 'vitest';
import * as networkService from './network';
import { invoke } from '@tauri-apps/api/core';

describe('networkService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('startNetwork', () => {
    it('should invoke start_network', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await networkService.startNetwork();

      expect(invoke).toHaveBeenCalledWith('start_network', {});
    });

    it('passes persisted startup settings to the backend', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await networkService.startNetwork({
        enableMdns: false,
        bootstrapNodes: ['/ip4/127.0.0.1/tcp/4001/p2p/12D3KooWExample'],
      });

      expect(invoke).toHaveBeenCalledWith('start_network', {
        enableMdns: false,
        bootstrapNodes: ['/ip4/127.0.0.1/tcp/4001/p2p/12D3KooWExample'],
      });
    });
  });

  describe('stopNetwork', () => {
    it('should invoke stop_network', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await networkService.stopNetwork();

      expect(invoke).toHaveBeenCalledWith('stop_network');
    });
  });

  describe('isNetworkRunning', () => {
    it('should invoke is_network_running', async () => {
      vi.mocked(invoke).mockResolvedValue(true);

      const result = await networkService.isNetworkRunning();

      expect(invoke).toHaveBeenCalledWith('is_network_running');
      expect(result).toBe(true);
    });
  });

  describe('getConnectedPeers', () => {
    it('should invoke get_connected_peers', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await networkService.getConnectedPeers();

      expect(invoke).toHaveBeenCalledWith('get_connected_peers');
    });
  });

  describe('getNetworkStats', () => {
    it('should invoke get_network_stats', async () => {
      vi.mocked(invoke).mockResolvedValue({
        connectedPeers: 3,
        totalBytesIn: 1000,
        totalBytesOut: 2000,
        uptimeSeconds: 300,
        natStatus: 'public',
        relayAddresses: [],
        externalAddresses: [],
      });

      const result = await networkService.getNetworkStats();

      expect(invoke).toHaveBeenCalledWith('get_network_stats');
      expect(result.connectedPeers).toBe(3);
    });
  });

  describe('connectToPeer', () => {
    it('should invoke connect_to_peer with multiaddr', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await networkService.connectToPeer('/ip4/1.2.3.4/tcp/9000');

      expect(invoke).toHaveBeenCalledWith('connect_to_peer', {
        multiaddr: '/ip4/1.2.3.4/tcp/9000',
      });
    });
  });

  describe('addBootstrapNode', () => {
    it('should invoke add_bootstrap_node', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await networkService.addBootstrapNode('/ip4/1.2.3.4/tcp/9000');

      expect(invoke).toHaveBeenCalledWith('add_bootstrap_node', {
        multiaddr: '/ip4/1.2.3.4/tcp/9000',
      });
    });
  });

  describe('addRelayServer', () => {
    it('should invoke add_relay_server', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await networkService.addRelayServer('/ip4/1.2.3.4/tcp/9000/p2p/QmRelay');

      expect(invoke).toHaveBeenCalledWith('add_relay_server', {
        multiaddr: '/ip4/1.2.3.4/tcp/9000/p2p/QmRelay',
      });
    });
  });

  describe('getShareableContactString', () => {
    it('should invoke get_shareable_contact_string', async () => {
      vi.mocked(invoke).mockResolvedValue('harbor://contact/...');

      const result = await networkService.getShareableContactString();

      expect(invoke).toHaveBeenCalledWith('get_shareable_contact_string');
      expect(result).toBe('harbor://contact/...');
    });
  });

  describe('addContactFromString', () => {
    const peerId = '12D3KooWEKyPsvgm7g8vyRA59466ph3t9VLxEUFzNqTXHJdfY2Wq';
    const bundle = {
      version: 1,
      peerId,
      multiaddr: `/ip4/1.2.3.4/tcp/9000/p2p/${peerId}`,
      displayName: 'Alice',
      publicKey: 'QwRr/kCSs+lJlOraFdzCDYqqB7ZY/TlU644O+4vcpd4=',
      x25519Public: 'CAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAg=',
    };
    const encoded = btoa(JSON.stringify(bundle))
      .replace(/\+/g, '-')
      .replace(/\//g, '_')
      .replace(/=+$/, '');

    it('normalizes an official web invite before invoking add_contact_from_string', async () => {
      const added = {
        requestId: 'request-1',
        peerId,
        status: 'pending',
        delivery: 'offline',
      };
      vi.mocked(invoke).mockResolvedValue(added);

      const result = await networkService.addContactFromString(
        `https://social-harbor.com/add-friend/v1/${encoded}`,
      );

      expect(invoke).toHaveBeenCalledWith('add_contact_from_string', {
        contactString: `harbor://contact/v1/${encoded}`,
      });
      expect(result).toEqual(added);
    });

    it('rejects malformed and oversized invites without invoking the backend', async () => {
      await expect(
        networkService.addContactFromString('https://evil.test/add-friend/v1/x'),
      ).rejects.toThrow(/official Harbor v1/);
      expect(invoke).not.toHaveBeenCalled();
    });
  });

  describe('syncFeed', () => {
    it('should invoke sync_feed with limit', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await networkService.syncFeed(50);

      expect(invoke).toHaveBeenCalledWith('sync_feed', { limit: 50 });
    });
  });
});
