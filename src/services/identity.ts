import { invoke } from '@tauri-apps/api/core';
import type {
  IdentityInfo,
  CreateIdentityRequest,
  RegisterRelayNameRequest,
  RelayNameClaim,
} from '../types';

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

  /** Update passphrase hint */
  async updatePassphraseHint(hint: string | null): Promise<void> {
    return invoke('update_passphrase_hint', { hint });
  },

  /** Get the local peer ID */
  async getPeerId(): Promise<string> {
    return invoke<string>('get_peer_id');
  },
  async registerRelayName(request: RegisterRelayNameRequest): Promise<RelayNameClaim> {
    return invoke<RelayNameClaim>('register_relay_name', { request });
  },
  async getLocalNameClaim(): Promise<RelayNameClaim | null> {
    return invoke<RelayNameClaim | null>('get_local_name_claim');
  },
  async verifyNameClaim(claim: RelayNameClaim): Promise<boolean> {
    return invoke<boolean>('verify_name_claim', { claim });
  },
  async getIdentityEntryState(): Promise<{
    mode: 'required' | 'compatibility' | 'verified';
    claim: RelayNameClaim | null;
  }> {
    return invoke('get_identity_entry_state');
  },
  async getMigrationState(): Promise<'required' | 'compatibility' | 'verified'> {
    return (
      await invoke<{ mode: 'required' | 'compatibility' | 'verified' }>(
        'get_identity_migration_state',
      )
    ).mode;
  },
  async setMigrationMode(mode: 'compatibility' | 'verified'): Promise<void> {
    return invoke('set_identity_migration_mode', { mode });
  },
};
