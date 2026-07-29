import type { Capability, PermissionInfo, GrantResult } from '../types';
import { invokeCommand } from './command';

/** Permissions service - wraps Tauri commands */
export const permissionsService = {
  /** Grant a permission to another peer */
  async grantPermission(
    subjectPeerId: string,
    capability: Capability,
    expiresInSeconds?: number | null,
  ): Promise<GrantResult> {
    return invokeCommand('grant_permission', {
      subjectPeerId,
      capability,
      expiresInSeconds,
    });
  },

  /** Revoke a permission */
  async revokePermission(grantId: string): Promise<boolean> {
    return invokeCommand('revoke_permission', { grantId });
  },

  /** Check if a peer has a specific capability (we granted it to them) */
  async peerHasCapability(peerId: string, capability: Capability): Promise<boolean> {
    return invokeCommand('peer_has_capability', { peerId, capability });
  },

  /** Check if we have a specific capability from another peer */
  async weHaveCapability(issuerPeerId: string, capability: Capability): Promise<boolean> {
    return invokeCommand('we_have_capability', { issuerPeerId, capability });
  },

  /** Get all permissions we've granted */
  async getGrantedPermissions(): Promise<PermissionInfo[]> {
    return invokeCommand('get_granted_permissions');
  },

  /** Get all permissions granted to us */
  async getReceivedPermissions(): Promise<PermissionInfo[]> {
    return invokeCommand('get_received_permissions');
  },

  /** Get all peers we can chat with */
  async getChatPeers(): Promise<string[]> {
    return invokeCommand('get_chat_peers');
  },

  /** Grant all standard permissions (chat, wall_read, call) to a peer */
  async grantAllPermissions(subjectPeerId: string): Promise<GrantResult[]> {
    return invokeCommand('grant_all_permissions', { subjectPeerId });
  },
};
