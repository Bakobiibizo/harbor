import { invoke } from '@tauri-apps/api/core';
import type { IdentityInfo, CreateIdentityRequest } from '../types';

/** Identity service - wraps Tauri commands */
export const identityService = {
  /** Check if an identity has been created */
  async hasIdentity(): Promise<boolean> {
    return invoke<boolean>('has_identity');
  },

  /** Check if the identity is currently unlocked */
  async isUnlocked(): Promise<boolean> {
    return invoke<boolean>('is_identity_unlocked');
  },

  /** Get identity info (public data only) */
  async getIdentityInfo(): Promise<IdentityInfo | null> {
    return invoke<IdentityInfo | null>('get_identity_info');
  },

  /** Create a new identity */
  async createIdentity(request: CreateIdentityRequest): Promise<IdentityInfo> {
    return invoke<IdentityInfo>('create_identity', { request });
  },

  /** Unlock the identity with passphrase */
  async unlock(passphrase: string): Promise<IdentityInfo> {
    return invoke<IdentityInfo>('unlock_identity', { passphrase });
  },

  /** Lock the identity */
  async lock(): Promise<void> {
    return invoke('lock_identity');
  },

  /** Update display name */
  async updateDisplayName(displayName: string): Promise<void> {
    return invoke('update_display_name', { displayName });
  },

  /** Update bio */
  async updateBio(bio: string | null): Promise<void> {
    return invoke('update_bio', { bio });
  },

  /** Get the local peer ID */
  async getPeerId(): Promise<string> {
    return invoke<string>('get_peer_id');
  },
};
