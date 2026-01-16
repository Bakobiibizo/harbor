import { invoke } from "@tauri-apps/api/core";
import type { Capability, PermissionInfo, GrantResult } from "../types";

/** Permissions service - wraps Tauri commands */
export const permissionsService = {
  /** Grant a permission to another peer */
  async grantPermission(
    subjectPeerId: string,
    capability: Capability,
    expiresInSeconds?: number | null
  ): Promise<GrantResult> {
    return invoke<GrantResult>("grant_permission", {
      subjectPeerId,
      capability,
      expiresInSeconds,
    });
  },

  /** Revoke a permission */
  async revokePermission(grantId: string): Promise<boolean> {
    return invoke<boolean>("revoke_permission", { grantId });
  },

  /** Check if a peer has a specific capability (we granted it to them) */
  async peerHasCapability(
    peerId: string,
    capability: Capability
  ): Promise<boolean> {
    return invoke<boolean>("peer_has_capability", { peerId, capability });
  },

  /** Check if we have a specific capability from another peer */
  async weHaveCapability(
    issuerPeerId: string,
    capability: Capability
  ): Promise<boolean> {
    return invoke<boolean>("we_have_capability", { issuerPeerId, capability });
  },

  /** Get all permissions we've granted */
  async getGrantedPermissions(): Promise<PermissionInfo[]> {
    return invoke<PermissionInfo[]>("get_granted_permissions");
  },

  /** Get all permissions granted to us */
  async getReceivedPermissions(): Promise<PermissionInfo[]> {
    return invoke<PermissionInfo[]>("get_received_permissions");
  },

  /** Get all peers we can chat with */
  async getChatPeers(): Promise<string[]> {
    return invoke<string[]>("get_chat_peers");
  },

  /** Grant all standard permissions (chat, wall_read, call) to a peer */
  async grantAllPermissions(subjectPeerId: string): Promise<GrantResult[]> {
    return invoke<GrantResult[]>("grant_all_permissions", { subjectPeerId });
  },
};
