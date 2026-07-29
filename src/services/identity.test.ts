import { describe, it, expect, vi, beforeEach } from 'vitest';
import { identityService } from './identity';
import { invoke } from '@tauri-apps/api/core';

describe('identityService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('getInitializationState', () => {
    it('requests the authoritative typed initialization state', async () => {
      const state = { status: 'absent' } as const;
      vi.mocked(invoke).mockResolvedValue(state);

      await expect(identityService.getInitializationState()).resolves.toEqual(state);

      expect(invoke).toHaveBeenCalledWith('get_identity_initialization_state');
    });
  });

  describe('hasIdentity', () => {
    it('should invoke has_identity', async () => {
      vi.mocked(invoke).mockResolvedValue(true);

      const result = await identityService.hasIdentity();

      expect(invoke).toHaveBeenCalledWith('has_identity');
      expect(result).toBe(true);
    });
  });

  describe('isUnlocked', () => {
    it('should invoke is_identity_unlocked', async () => {
      vi.mocked(invoke).mockResolvedValue(false);

      const result = await identityService.isUnlocked();

      expect(invoke).toHaveBeenCalledWith('is_identity_unlocked');
      expect(result).toBe(false);
    });
  });

  describe('getIdentityInfo', () => {
    it('should invoke get_identity_info', async () => {
      const mockInfo = {
        peerId: '12D3KooWTest',
        publicKey: 'base64key',
        x25519Public: 'base64x25519',
        displayName: 'Test User',
        avatarHash: null,
        bio: 'Hello',
        passphraseHint: null,
        createdAt: 1700000000,
        updatedAt: 1700000000,
      };
      vi.mocked(invoke).mockResolvedValue(mockInfo);

      const result = await identityService.getIdentityInfo();

      expect(invoke).toHaveBeenCalledWith('get_identity_info');
      expect(result).toEqual(mockInfo);
    });

    it('should return null when no identity exists', async () => {
      vi.mocked(invoke).mockResolvedValue(null);

      const result = await identityService.getIdentityInfo();

      expect(result).toBeNull();
    });
  });

  describe('createIdentity', () => {
    it('should invoke create_identity with request', async () => {
      const request = { displayName: 'New User', passphrase: 'test-pass-not-real' };
      const mockResult = {
        peerId: '12D3KooWNew',
        publicKey: 'key',
        x25519Public: 'x25519',
        displayName: 'New User',
        avatarHash: null,
        bio: null,
        passphraseHint: null,
        createdAt: 1700000000,
        updatedAt: 1700000000,
      };
      vi.mocked(invoke).mockResolvedValue(mockResult);

      const result = await identityService.createIdentity(request);

      expect(invoke).toHaveBeenCalledWith('create_identity', { request });
      expect(result).toEqual(mockResult);
    });
  });

  describe('updateProfileAvatar', () => {
    it('sends only the selected native path and returns authoritative identity metadata', async () => {
      const updated = {
        peerId: 'peer-avatar',
        publicKey: 'key',
        x25519Public: 'x25519',
        displayName: 'Avatar User',
        avatarHash: 'a'.repeat(64),
        bio: null,
        passphraseHint: null,
        createdAt: 1,
        updatedAt: 2,
      };
      vi.mocked(invoke).mockResolvedValue(updated);

      await expect(identityService.updateProfileAvatar('/native/avatar.png')).resolves.toEqual(
        updated,
      );
      expect(invoke).toHaveBeenCalledWith('update_profile_avatar', {
        filePath: '/native/avatar.png',
      });
    });
  });

  describe('unlock', () => {
    it('should invoke unlock_identity with passphrase', async () => {
      vi.mocked(invoke).mockResolvedValue({});

      await identityService.unlock('test-passphrase-not-real');

      expect(invoke).toHaveBeenCalledWith('unlock_identity', {
        passphrase: 'test-passphrase-not-real',
      });
    });
  });

  describe('changePassword', () => {
    it('should invoke change_identity_password with both passwords', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await identityService.changePassword('current-password', 'new-password');

      expect(invoke).toHaveBeenCalledWith('change_identity_password', {
        currentPassword: 'current-password',
        newPassword: 'new-password',
      });
    });
  });

  describe('lock', () => {
    it('should invoke lock_identity', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await identityService.lock();

      expect(invoke).toHaveBeenCalledWith('lock_identity');
    });
  });

  describe('updateDisplayName', () => {
    it('should invoke update_display_name', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await identityService.updateDisplayName('New Name');

      expect(invoke).toHaveBeenCalledWith('update_display_name', { displayName: 'New Name' });
    });
  });

  describe('updateBio', () => {
    it('should invoke update_bio with string', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await identityService.updateBio('New bio');

      expect(invoke).toHaveBeenCalledWith('update_bio', { bio: 'New bio' });
    });

    it('should invoke update_bio with null to clear', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await identityService.updateBio(null);

      expect(invoke).toHaveBeenCalledWith('update_bio', { bio: null });
    });
  });

  describe('updatePassphraseHint', () => {
    it('should invoke update_passphrase_hint', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await identityService.updatePassphraseHint('my hint');

      expect(invoke).toHaveBeenCalledWith('update_passphrase_hint', { hint: 'my hint' });
    });
  });

  describe('getPeerId', () => {
    it('should invoke get_peer_id', async () => {
      vi.mocked(invoke).mockResolvedValue('12D3KooWTest');

      const result = await identityService.getPeerId();

      expect(invoke).toHaveBeenCalledWith('get_peer_id');
      expect(result).toBe('12D3KooWTest');
    });
  });

  describe('getIdentityEntryState', () => {
    it('requests the atomic backend-verified returning identity state', async () => {
      const entry = { mode: 'verified', claim: null } as const;
      vi.mocked(invoke).mockResolvedValue(entry);

      await expect(identityService.getIdentityEntryState()).resolves.toEqual(entry);

      expect(invoke).toHaveBeenCalledWith('get_identity_entry_state');
    });
  });
});
