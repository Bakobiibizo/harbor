import { describe, it, expect, vi, beforeEach } from 'vitest';
import { permissionsService } from './permissions';
import { invoke } from '@tauri-apps/api/core';

describe('permissionsService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('grantPermission', () => {
    it('should invoke grant_permission with args', async () => {
      const mockResult = { grantId: 'grant-1', success: true };
      vi.mocked(invoke).mockResolvedValue(mockResult);

      const result = await permissionsService.grantPermission('peer-alice', 'chat', 3600);

      expect(invoke).toHaveBeenCalledWith('grant_permission', {
        subjectPeerId: 'peer-alice',
        capability: 'chat',
        expiresInSeconds: 3600,
      });
      expect(result).toEqual(mockResult);
    });

    it('should handle null expiration', async () => {
      vi.mocked(invoke).mockResolvedValue({});

      await permissionsService.grantPermission('peer-alice', 'wall_read', null);

      expect(invoke).toHaveBeenCalledWith('grant_permission', {
        subjectPeerId: 'peer-alice',
        capability: 'wall_read',
        expiresInSeconds: null,
      });
    });
  });

  describe('revokePermission', () => {
    it('should invoke revoke_permission', async () => {
      vi.mocked(invoke).mockResolvedValue(true);

      const result = await permissionsService.revokePermission('grant-1');

      expect(invoke).toHaveBeenCalledWith('revoke_permission', { grantId: 'grant-1' });
      expect(result).toBe(true);
    });
  });

  describe('peerHasCapability', () => {
    it('should invoke peer_has_capability', async () => {
      vi.mocked(invoke).mockResolvedValue(true);

      const result = await permissionsService.peerHasCapability('peer-alice', 'chat');

      expect(invoke).toHaveBeenCalledWith('peer_has_capability', {
        peerId: 'peer-alice',
        capability: 'chat',
      });
      expect(result).toBe(true);
    });
  });

  describe('weHaveCapability', () => {
    it('should invoke we_have_capability', async () => {
      vi.mocked(invoke).mockResolvedValue(false);

      const result = await permissionsService.weHaveCapability('peer-alice', 'call');

      expect(invoke).toHaveBeenCalledWith('we_have_capability', {
        issuerPeerId: 'peer-alice',
        capability: 'call',
      });
      expect(result).toBe(false);
    });
  });

  describe('getGrantedPermissions', () => {
    it('should invoke get_granted_permissions', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await permissionsService.getGrantedPermissions();

      expect(invoke).toHaveBeenCalledWith('get_granted_permissions');
    });
  });

  describe('getReceivedPermissions', () => {
    it('should invoke get_received_permissions', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await permissionsService.getReceivedPermissions();

      expect(invoke).toHaveBeenCalledWith('get_received_permissions');
    });
  });

  describe('getChatPeers', () => {
    it('should invoke get_chat_peers', async () => {
      vi.mocked(invoke).mockResolvedValue(['peer-alice', 'peer-bob']);

      const result = await permissionsService.getChatPeers();

      expect(invoke).toHaveBeenCalledWith('get_chat_peers');
      expect(result).toEqual(['peer-alice', 'peer-bob']);
    });
  });

  describe('grantAllPermissions', () => {
    it('should invoke grant_all_permissions', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await permissionsService.grantAllPermissions('peer-alice');

      expect(invoke).toHaveBeenCalledWith('grant_all_permissions', {
        subjectPeerId: 'peer-alice',
      });
    });
  });
});
