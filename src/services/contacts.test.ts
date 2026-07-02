import { describe, it, expect, vi, beforeEach } from 'vitest';
import { contactsService } from './contacts';
import { invoke } from '@tauri-apps/api/core';

describe('contactsService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('getContacts', () => {
    it('should invoke get_contacts', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await contactsService.getContacts();

      expect(invoke).toHaveBeenCalledWith('get_contacts');
    });
  });

  describe('getActiveContacts', () => {
    it('should invoke get_active_contacts', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await contactsService.getActiveContacts();

      expect(invoke).toHaveBeenCalledWith('get_active_contacts');
    });
  });

  describe('getContact', () => {
    it('should invoke get_contact with peerId', async () => {
      vi.mocked(invoke).mockResolvedValue(null);

      await contactsService.getContact('peer-alice');

      expect(invoke).toHaveBeenCalledWith('get_contact', { peerId: 'peer-alice' });
    });
  });

  describe('addContact', () => {
    it('should convert base64 keys to byte arrays and invoke add_contact', async () => {
      vi.mocked(invoke).mockResolvedValue(1);

      // "AQID" is base64 for bytes [1, 2, 3]
      await contactsService.addContact({
        peerId: 'peer-new',
        publicKey: 'AQID',
        x25519Public: 'BAUG',
        displayName: 'New User',
      });

      expect(invoke).toHaveBeenCalledWith('add_contact', {
        peerId: 'peer-new',
        publicKey: [1, 2, 3],
        x25519Public: [4, 5, 6],
        displayName: 'New User',
        avatarHash: null,
        bio: null,
      });
    });
  });

  describe('blockContact', () => {
    it('should invoke block_contact', async () => {
      vi.mocked(invoke).mockResolvedValue(true);

      const result = await contactsService.blockContact('peer-alice');

      expect(invoke).toHaveBeenCalledWith('block_contact', { peerId: 'peer-alice' });
      expect(result).toBe(true);
    });
  });

  describe('unblockContact', () => {
    it('should invoke unblock_contact', async () => {
      vi.mocked(invoke).mockResolvedValue(true);

      const result = await contactsService.unblockContact('peer-alice');

      expect(invoke).toHaveBeenCalledWith('unblock_contact', { peerId: 'peer-alice' });
      expect(result).toBe(true);
    });
  });

  describe('removeContact', () => {
    it('should invoke remove_contact', async () => {
      vi.mocked(invoke).mockResolvedValue(true);

      const result = await contactsService.removeContact('peer-alice');

      expect(invoke).toHaveBeenCalledWith('remove_contact', { peerId: 'peer-alice' });
      expect(result).toBe(true);
    });
  });

  describe('isContact', () => {
    it('should invoke is_contact', async () => {
      vi.mocked(invoke).mockResolvedValue(true);

      const result = await contactsService.isContact('peer-alice');

      expect(invoke).toHaveBeenCalledWith('is_contact', { peerId: 'peer-alice' });
      expect(result).toBe(true);
    });
  });

  describe('isBlocked', () => {
    it('should invoke is_contact_blocked', async () => {
      vi.mocked(invoke).mockResolvedValue(false);

      const result = await contactsService.isBlocked('peer-alice');

      expect(invoke).toHaveBeenCalledWith('is_contact_blocked', { peerId: 'peer-alice' });
      expect(result).toBe(false);
    });
  });

  describe('requestPeerIdentity', () => {
    it('should invoke request_peer_identity', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await contactsService.requestPeerIdentity('peer-alice');

      expect(invoke).toHaveBeenCalledWith('request_peer_identity', { peerId: 'peer-alice' });
    });
  });
});
