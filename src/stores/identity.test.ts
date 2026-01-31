import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useIdentityStore } from './identity';
import { identityService } from '../services';

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

  describe('unlock', () => {
    it('should unlock identity successfully', async () => {
      vi.mocked(identityService.unlock).mockResolvedValue(mockIdentity);

      await useIdentityStore.getState().unlock('test-passphrase-not-real');

      const state = useIdentityStore.getState().state;
      expect(state.status).toBe('unlocked');
    });

    it('should set error on wrong passphrase', async () => {
      vi.mocked(identityService.unlock).mockRejectedValue(new Error('Invalid passphrase'));

      await expect(useIdentityStore.getState().unlock('wrong')).rejects.toThrow(
        'Invalid passphrase',
      );

      expect(useIdentityStore.getState().error).toBe('Invalid passphrase');
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
