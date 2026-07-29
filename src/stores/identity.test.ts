import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useIdentityStore } from './identity';
import { identityService, networkService } from '../services';
import { suspendProfile } from '../services/profileSession';

vi.mock('../services', () => ({
  identityService: {
    getInitializationState: vi.fn(),
    hasIdentity: vi.fn(),
    getIdentityInfo: vi.fn(),
    isUnlocked: vi.fn(),
    createIdentity: vi.fn(),
    unlock: vi.fn(),
    changePassword: vi.fn(),
    lock: vi.fn(),
    updateDisplayName: vi.fn(),
    updateBio: vi.fn(),
    updatePassphraseHint: vi.fn(),
    getIdentityEntryState: vi.fn(),
    registerRelayName: vi.fn(),
    verifyNameClaim: vi.fn(),
    setPublishingMode: vi.fn(),
  },
  networkService: {
    startNetwork: vi.fn(),
    connectToPublicRelays: vi.fn(),
    getNetworkStats: vi.fn(),
  },
}));

vi.mock('../services/profileSession', () => ({ suspendProfile: vi.fn() }));

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
    vi.mocked(identityService.getIdentityEntryState).mockResolvedValue({
      mode: 'required',
      claim: null,
    });
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
    it('offers creation only after authoritative identity absence', async () => {
      vi.mocked(identityService.getInitializationState).mockResolvedValue({ status: 'absent' });

      await useIdentityStore.getState().initialize();

      expect(useIdentityStore.getState().state.status).toBe('absent');
    });

    it('uses the authoritative locked identity state', async () => {
      vi.mocked(identityService.getInitializationState).mockResolvedValue({
        status: 'locked',
        identity: mockIdentity,
      });

      await useIdentityStore.getState().initialize();

      const state = useIdentityStore.getState().state;
      expect(state.status).toBe('locked');
      if (state.status === 'locked') {
        expect(state.identity).toEqual(mockIdentity);
      }
    });

    it('uses the authoritative unlocked identity state', async () => {
      vi.mocked(identityService.getInitializationState).mockResolvedValue({
        status: 'unlocked',
        identity: mockIdentity,
      });

      await useIdentityStore.getState().initialize();

      const state = useIdentityStore.getState().state;
      expect(state.status).toBe('unlocked');
      if (state.status === 'unlocked') {
        expect(state.identity).toEqual(mockIdentity);
      }
    });

    it('turns an IPC rejection into a retryable transport failure, never absence', async () => {
      vi.mocked(identityService.getInitializationState).mockRejectedValue(
        new Error('Desktop IPC unavailable'),
      );

      await useIdentityStore.getState().initialize();

      expect(useIdentityStore.getState().state).toEqual({
        status: 'recoverableError',
        source: 'ipc',
        error: {
          code: 'IPC_ERROR',
          message: 'Desktop IPC unavailable',
          recovery: 'Retry. If the problem continues, restart Harbor.',
        },
      });
      expect(useIdentityStore.getState().error).toBe('Desktop IPC unavailable');
    });

    it.each([
      {
        label: 'database permission',
        state: {
          status: 'recoverableError' as const,
          source: 'identityDatabase' as const,
          error: { code: 'PERMISSION_DENIED', message: 'Database access denied' },
        },
      },
      {
        label: 'account registry',
        state: {
          status: 'recoverableError' as const,
          source: 'accountRegistry' as const,
          error: { code: 'IO_ERROR', message: 'Registry is temporarily unavailable' },
        },
      },
    ])('preserves the typed $label recoverable state', async ({ state }) => {
      vi.mocked(identityService.getInitializationState).mockResolvedValue(state);

      await useIdentityStore.getState().initialize();

      expect(useIdentityStore.getState().state).toEqual(state);
      expect(useIdentityStore.getState().state.status).not.toBe('absent');
    });

    it('preserves a fatal identity-corruption state', async () => {
      const corruption = {
        status: 'fatalError' as const,
        source: 'identityCorruption' as const,
        error: {
          code: 'INVALID_DATA',
          message: 'Saved identity keys are inconsistent',
          recovery: 'Restore this account from a trusted backup.',
        },
      };
      vi.mocked(identityService.getInitializationState).mockResolvedValue(corruption);

      await useIdentityStore.getState().initialize();

      expect(useIdentityStore.getState().state).toEqual(corruption);
      expect(useIdentityStore.getState().state.status).not.toBe('absent');
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
    vi.mocked(identityService.setPublishingMode).mockResolvedValue();
    await useIdentityStore
      .getState()
      .completeOnboarding(
        { displayName: 'Test User', passphrase: 'secret-passphrase' },
        'test-user',
        'relay.test',
      );
    expect(identityService.createIdentity).not.toHaveBeenCalled();
    expect(identityService.setPublishingMode).toHaveBeenCalledWith('verified');
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
    vi.mocked(identityService.setPublishingMode).mockResolvedValue();

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

  it('retries relay registration once only for an explicit transient network code', async () => {
    vi.mocked(identityService.hasIdentity).mockResolvedValue(false);
    vi.mocked(identityService.createIdentity).mockResolvedValue(mockIdentity);
    vi.mocked(networkService.startNetwork).mockResolvedValue();
    vi.mocked(networkService.connectToPublicRelays).mockResolvedValue();
    const claim = { request: { peerId: mockIdentity.peerId } } as never;
    vi.mocked(identityService.registerRelayName)
      .mockRejectedValueOnce({
        code: 'NETWORK_SERVICE_UNAVAILABLE',
        message: 'Relay actor is restarting',
      })
      .mockResolvedValueOnce(claim);
    vi.mocked(identityService.verifyNameClaim).mockResolvedValue(true);
    vi.mocked(identityService.setPublishingMode).mockResolvedValue();

    await useIdentityStore
      .getState()
      .completeOnboarding(
        { displayName: 'Test User', passphrase: 'secret-passphrase' },
        'test-user',
        'relay.test',
      );

    expect(identityService.registerRelayName).toHaveBeenCalledTimes(2);
  });

  it('does not retry relay registration for a non-network structured failure', async () => {
    vi.mocked(identityService.hasIdentity).mockResolvedValue(false);
    vi.mocked(identityService.createIdentity).mockResolvedValue(mockIdentity);
    vi.mocked(networkService.startNetwork).mockResolvedValue();
    vi.mocked(networkService.connectToPublicRelays).mockResolvedValue();
    vi.mocked(identityService.registerRelayName).mockRejectedValue({
      code: 'ALREADY_EXISTS',
      message: 'That name is already claimed',
    });

    await expect(
      useIdentityStore
        .getState()
        .completeOnboarding(
          { displayName: 'Test User', passphrase: 'secret-passphrase' },
          'test-user',
          'relay.test',
        ),
    ).rejects.toMatchObject({ code: 'ALREADY_EXISTS' });

    expect(identityService.registerRelayName).toHaveBeenCalledOnce();
    expect(useIdentityStore.getState().error).toBe('That name is already claimed');
  });

  describe('unlock', () => {
    it('should unlock identity successfully', async () => {
      vi.mocked(identityService.unlock).mockResolvedValue(mockIdentity);

      await useIdentityStore.getState().unlock('test-passphrase-not-real');

      const state = useIdentityStore.getState().state;
      expect(state.status).toBe('unlocked');
    });

    it('restores a cryptographically verified persisted claim while unlocking', async () => {
      const claim = { request: { peerId: mockIdentity.peerId } } as never;
      vi.mocked(identityService.unlock).mockResolvedValue(mockIdentity);
      vi.mocked(identityService.getIdentityEntryState).mockResolvedValue({
        mode: 'verified',
        claim,
      });

      await useIdentityStore.getState().unlock('test-passphrase-not-real');

      const state = useIdentityStore.getState().state;
      expect(state.status).toBe('unlocked');
      if (state.status === 'unlocked') {
        expect(state.identity.relayNameClaim).toBe(claim);
        expect(state.identity.relayNameVerified).toBe(true);
      }
      expect(identityService.setPublishingMode).not.toHaveBeenCalled();
    });

    it('should set error on a wrong password', async () => {
      vi.mocked(identityService.unlock).mockRejectedValue(new Error('Invalid password'));

      await expect(useIdentityStore.getState().unlock('wrong')).rejects.toThrow('Invalid password');

      expect(useIdentityStore.getState().error).toBe('Invalid password');
    });
  });

  describe('changePassword', () => {
    it('calls the authoritative service and preserves unlocked state', async () => {
      useIdentityStore.setState({
        state: { status: 'unlocked', identity: mockIdentity },
      });
      vi.mocked(identityService.changePassword).mockResolvedValue(undefined);

      await useIdentityStore.getState().changePassword('current-password', 'new-password');

      expect(identityService.changePassword).toHaveBeenCalledWith(
        'current-password',
        'new-password',
      );
      expect(useIdentityStore.getState().state.status).toBe('unlocked');
      expect(useIdentityStore.getState().error).toBeNull();
    });

    it('surfaces backend rejection without changing identity state', async () => {
      useIdentityStore.setState({
        state: { status: 'unlocked', identity: mockIdentity },
      });
      vi.mocked(identityService.changePassword).mockRejectedValue(new Error('Invalid password'));

      await expect(
        useIdentityStore.getState().changePassword('wrong-password', 'new-password'),
      ).rejects.toThrow('Invalid password');

      expect(useIdentityStore.getState().state.status).toBe('unlocked');
      expect(useIdentityStore.getState().error).toBe('Invalid password');
    });
  });

  it('bounds an interrupted relay claim and permits another attempt', async () => {
    vi.useFakeTimers();
    try {
      vi.mocked(identityService.hasIdentity).mockResolvedValue(true);
      vi.mocked(identityService.getIdentityInfo).mockResolvedValue(mockIdentity);
      vi.mocked(identityService.isUnlocked).mockResolvedValue(true);
      vi.mocked(networkService.startNetwork).mockResolvedValue();
      vi.mocked(networkService.connectToPublicRelays).mockResolvedValue();
      vi.mocked(identityService.registerRelayName).mockImplementation(
        () => new Promise(() => undefined),
      );

      const attempt = useIdentityStore
        .getState()
        .completeOnboarding(
          { displayName: 'Test User', passphrase: 'secret-passphrase' },
          'test-user',
          'relay.test',
        );
      const rejected = expect(attempt).rejects.toThrow('Name registration timed out');
      await vi.advanceTimersByTimeAsync(31_000);
      await rejected;

      expect(identityService.registerRelayName).toHaveBeenCalledOnce();
      expect(useIdentityStore.getState().error).toContain('timed out');
    } finally {
      vi.useRealTimers();
    }
  });

  describe('lock', () => {
    it('suspends the active profile only after authoritative lock succeeds', async () => {
      useIdentityStore.setState({
        state: { status: 'unlocked', identity: mockIdentity },
      });
      vi.mocked(identityService.lock).mockResolvedValue(undefined);

      await useIdentityStore.getState().lock();

      const state = useIdentityStore.getState().state;
      expect(state.status).toBe('locked');
      expect(suspendProfile).toHaveBeenCalledOnce();
      expect(vi.mocked(identityService.lock).mock.invocationCallOrder[0]).toBeLessThan(
        vi.mocked(suspendProfile).mock.invocationCallOrder[0],
      );
    });

    it('does not tear down frontend state when backend lock fails', async () => {
      useIdentityStore.setState({
        state: { status: 'unlocked', identity: mockIdentity },
      });
      vi.mocked(identityService.lock).mockRejectedValue(new Error('Backend refused lock'));

      await expect(useIdentityStore.getState().lock()).rejects.toThrow('Backend refused lock');

      expect(useIdentityStore.getState().state.status).toBe('unlocked');
      expect(suspendProfile).not.toHaveBeenCalled();
    });
  });

  it.each([
    ['updateDisplayName', 'displayName', 'Changed name'],
    ['updateBio', 'bio', 'Changed bio'],
    ['updatePassphraseHint', 'passphraseHint', 'Changed hint'],
  ] as const)(
    '%s rejects a structured failure and preserves the last confirmed identity',
    async (action, _field, value) => {
      useIdentityStore.setState({
        state: { status: 'unlocked', identity: mockIdentity },
      });
      vi.mocked(identityService[action]).mockRejectedValue({
        code: 'DATABASE_ERROR',
        message: 'Could not save the profile change',
        recovery: 'Retry in a moment.',
      });

      await expect(useIdentityStore.getState()[action](value)).rejects.toMatchObject({
        code: 'DATABASE_ERROR',
      });

      expect(useIdentityStore.getState().state).toEqual({
        status: 'unlocked',
        identity: mockIdentity,
      });
      expect(useIdentityStore.getState().error).toBe('Could not save the profile change');
    },
  );

  describe('clearError', () => {
    it('should clear error', () => {
      useIdentityStore.setState({ error: 'Some error' });

      useIdentityStore.getState().clearError();

      expect(useIdentityStore.getState().error).toBeNull();
    });
  });
});
