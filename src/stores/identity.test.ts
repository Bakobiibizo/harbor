import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useIdentityStore } from './identity';
import { identityService, networkService } from '../services';

vi.mock('../services', () => ({
  identityService: {
    hasIdentity: vi.fn(),
    getIdentityInfo: vi.fn(),
    isUnlocked: vi.fn(),
    createIdentity: vi.fn(),
    unlock: vi.fn(),
    lock: vi.fn(),
    updateDisplayName: vi.fn(),
    updateBio: vi.fn(),
    updatePassphraseHint: vi.fn(),
    registerRelayName: vi.fn(),
    verifyNameClaim: vi.fn(),
    setMigrationMode: vi.fn(),
  },
  networkService: {
    startNetwork: vi.fn(),
    connectToPublicRelays: vi.fn(),
    getNetworkStats: vi.fn(),
  },
}));

const mockIdentity = {
  peerId: '12D3KooWTest123',
  publicKey: 'base64PublicKey',
  x25519Public: 'base64X25519Public',
  displayName: 'Test User',
  avatarHash: null,
  bio: 'Test bio',
  passphraseHint: null,
  createdAt: 1704067200000,
  updatedAt: 1704067200000,
};

describe('useIdentityStore', () => {
  beforeEach(() => {
    useIdentityStore.setState({
      state: { status: 'loading' },
      error: null,
    });
    vi.clearAllMocks();
    vi.mocked(networkService.getNetworkStats).mockResolvedValue({
      connectedPeers: 1,
      totalBytesIn: 0,
      totalBytesOut: 0,
      uptimeSeconds: 1,
      natStatus: 'private',
      relayAddresses: ['/p2p/relay/p2p-circuit/p2p/local'],
      externalAddresses: [],
    });
  });

  describe('initialize', () => {
    it('should set status to no_identity when no identity exists', async () => {
      vi.mocked(identityService.hasIdentity).mockResolvedValue(false);

      await useIdentityStore.getState().initialize();

      expect(useIdentityStore.getState().state.status).toBe('no_identity');
    });

    it('should set status to locked when identity exists but is locked', async () => {
      vi.mocked(identityService.hasIdentity).mockResolvedValue(true);
      vi.mocked(identityService.getIdentityInfo).mockResolvedValue(mockIdentity);
      vi.mocked(identityService.isUnlocked).mockResolvedValue(false);

      await useIdentityStore.getState().initialize();

      const state = useIdentityStore.getState().state;
      expect(state.status).toBe('locked');
      if (state.status === 'locked') {
        expect(state.identity).toEqual(mockIdentity);
      }
    });

    it('should set status to unlocked when identity is unlocked', async () => {
      vi.mocked(identityService.hasIdentity).mockResolvedValue(true);
      vi.mocked(identityService.getIdentityInfo).mockResolvedValue(mockIdentity);
      vi.mocked(identityService.isUnlocked).mockResolvedValue(true);

      await useIdentityStore.getState().initialize();

      const state = useIdentityStore.getState().state;
      expect(state.status).toBe('unlocked');
      if (state.status === 'unlocked') {
        expect(state.identity).toEqual(mockIdentity);
      }
    });

    it('should handle errors gracefully', async () => {
      vi.mocked(identityService.hasIdentity).mockRejectedValue(new Error('Test error'));

      await useIdentityStore.getState().initialize();

      expect(useIdentityStore.getState().state.status).toBe('no_identity');
      expect(useIdentityStore.getState().error).toBe('Test error');
    });
  });

  describe('createIdentity', () => {
    it('should create identity and set status to unlocked', async () => {
      vi.mocked(identityService.createIdentity).mockResolvedValue(mockIdentity);

      await useIdentityStore.getState().createIdentity({
        displayName: 'Test User',
        passphrase: 'test-passphrase-not-real',
      });

      const state = useIdentityStore.getState().state;
      expect(state.status).toBe('unlocked');
      if (state.status === 'unlocked') {
        expect(state.identity).toEqual(mockIdentity);
      }
    });

    it('should set error on failure', async () => {
      vi.mocked(identityService.createIdentity).mockRejectedValue(new Error('Creation failed'));

      await expect(
        useIdentityStore.getState().createIdentity({
          displayName: 'Test User',
          passphrase: 'test-passphrase-not-real',
        }),
      ).rejects.toThrow('Creation failed');

      expect(useIdentityStore.getState().error).toBe('Creation failed');
    });
  });

  it('resumes an unlocked half-created identity without creating it again', async () => {
    vi.mocked(identityService.hasIdentity).mockResolvedValue(true);
    vi.mocked(identityService.getIdentityInfo).mockResolvedValue(mockIdentity);
    vi.mocked(identityService.isUnlocked).mockResolvedValue(true);
    vi.mocked(networkService.startNetwork).mockResolvedValue();
    vi.mocked(networkService.connectToPublicRelays).mockResolvedValue();
    const claim = { request: { peerId: mockIdentity.peerId } } as never;
    vi.mocked(identityService.registerRelayName).mockResolvedValue(claim);
    vi.mocked(identityService.verifyNameClaim).mockResolvedValue(true);
    vi.mocked(identityService.setMigrationMode).mockResolvedValue();
    await useIdentityStore
      .getState()
      .completeOnboarding(
        { displayName: 'Test User', passphrase: 'secret-passphrase' },
        'test-user',
        'relay.test',
      );
    expect(identityService.createIdentity).not.toHaveBeenCalled();
    expect(identityService.setMigrationMode).toHaveBeenCalledWith('verified');
    expect(useIdentityStore.getState().state.status).toBe('unlocked');
  });

  it('waits for a relay reservation before requesting a name claim', async () => {
    vi.mocked(identityService.hasIdentity).mockResolvedValue(false);
    vi.mocked(identityService.createIdentity).mockResolvedValue(mockIdentity);
    vi.mocked(networkService.startNetwork).mockResolvedValue();
    vi.mocked(networkService.connectToPublicRelays).mockResolvedValue();
    vi.mocked(networkService.getNetworkStats)
      .mockResolvedValueOnce({
        connectedPeers: 1,
        totalBytesIn: 0,
        totalBytesOut: 0,
        uptimeSeconds: 1,
        natStatus: 'unknown',
        relayAddresses: [],
        externalAddresses: [],
      })
      .mockResolvedValue({
        connectedPeers: 1,
        totalBytesIn: 0,
        totalBytesOut: 0,
        uptimeSeconds: 2,
        natStatus: 'private',
        relayAddresses: ['/p2p/relay/p2p-circuit/p2p/local'],
        externalAddresses: [],
      });
    const claim = { request: { peerId: mockIdentity.peerId } } as never;
    vi.mocked(identityService.registerRelayName).mockResolvedValue(claim);
    vi.mocked(identityService.verifyNameClaim).mockResolvedValue(true);
    vi.mocked(identityService.setMigrationMode).mockResolvedValue();

    await useIdentityStore
      .getState()
      .completeOnboarding(
        { displayName: 'Test User', passphrase: 'secret-passphrase' },
        'test-user',
        'relay.test',
      );

    expect(networkService.getNetworkStats).toHaveBeenCalledTimes(2);
    expect(identityService.registerRelayName).toHaveBeenCalledAfter(
      vi.mocked(networkService.getNetworkStats),
    );
  });

  describe('unlock', () => {
    it('should unlock identity successfully', async () => {
      vi.mocked(identityService.unlock).mockResolvedValue(mockIdentity);

      await useIdentityStore.getState().unlock('test-passphrase-not-real');

      const state = useIdentityStore.getState().state;
      expect(state.status).toBe('unlocked');
    });

    it('should set error on a wrong password', async () => {
      vi.mocked(identityService.unlock).mockRejectedValue(new Error('Invalid password'));

      await expect(useIdentityStore.getState().unlock('wrong')).rejects.toThrow('Invalid password');

      expect(useIdentityStore.getState().error).toBe('Invalid password');
    });
  });

  describe('lock', () => {
    it('should lock identity', async () => {
      useIdentityStore.setState({
        state: { status: 'unlocked', identity: mockIdentity },
      });
      vi.mocked(identityService.lock).mockResolvedValue(undefined);

      await useIdentityStore.getState().lock();

      const state = useIdentityStore.getState().state;
      expect(state.status).toBe('locked');
    });
  });

  describe('clearError', () => {
    it('should clear error', () => {
      useIdentityStore.setState({ error: 'Some error' });

      useIdentityStore.getState().clearError();

      expect(useIdentityStore.getState().error).toBeNull();
    });
  });
});
