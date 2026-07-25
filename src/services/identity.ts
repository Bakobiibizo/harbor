import type {
  IdentityInfo,
  IdentityInitializationResult,
  CreateIdentityRequest,
  RegisterRelayNameRequest,
  RelayNameClaim,
} from '../types';
import { invokeCommand } from './command';

/** Identity service - wraps Tauri commands */
export const identityService = {
  /** Read startup state without collapsing lookup failures into identity absence. */
  async getInitializationState(): Promise<IdentityInitializationResult> {
    return invokeCommand('get_identity_initialization_state');
  },

  /** Check if an identity has been created */
  async hasIdentity(): Promise<boolean> {
    return invokeCommand('has_identity');
  },

  /** Check if the identity is currently unlocked */
  async isUnlocked(): Promise<boolean> {
    return invokeCommand('is_identity_unlocked');
  },

  /** Get identity info (public data only) */
  async getIdentityInfo(): Promise<IdentityInfo | null> {
    return invokeCommand('get_identity_info');
  },

  /** Create a new identity */
  async createIdentity(request: CreateIdentityRequest): Promise<IdentityInfo> {
    return invokeCommand('create_identity', { request });
  },

  /** Unlock the identity with passphrase */
  async unlock(passphrase: string): Promise<IdentityInfo> {
    return invokeCommand('unlock_identity', { passphrase });
  },

  /** Atomically re-encrypt the local identity under a new password. */
  async changePassword(currentPassword: string, newPassword: string): Promise<void> {
    return invokeCommand('change_identity_password', { currentPassword, newPassword });
  },

  /** Lock the identity */
  async lock(): Promise<void> {
    return invokeCommand('lock_identity');
  },

  /** Update display name */
  async updateDisplayName(displayName: string): Promise<void> {
    return invokeCommand('update_display_name', { displayName });
  },

  /** Update bio */
  async updateBio(bio: string | null): Promise<void> {
    return invokeCommand('update_bio', { bio });
  },

  async updateProfileAvatar(filePath: string | null): Promise<IdentityInfo> {
    return invokeCommand('update_profile_avatar', { filePath });
  },

  /** Update passphrase hint */
  async updatePassphraseHint(hint: string | null): Promise<void> {
    return invokeCommand('update_passphrase_hint', { hint });
  },

  /** Get the local peer ID */
  async getPeerId(): Promise<string> {
    return invokeCommand('get_peer_id');
  },
  async registerRelayName(request: RegisterRelayNameRequest): Promise<RelayNameClaim> {
    return invokeCommand('register_relay_name', { request });
  },
  async getLocalNameClaim(): Promise<RelayNameClaim | null> {
    return invokeCommand('get_local_name_claim');
  },
  async verifyNameClaim(claim: RelayNameClaim): Promise<boolean> {
    return invokeCommand('verify_name_claim', { claim });
  },
  async getIdentityEntryState(): Promise<{
    mode: 'required' | 'unverified' | 'verified';
    claim: RelayNameClaim | null;
  }> {
    return invokeCommand('get_identity_entry_state');
  },
  async getPublishingState(): Promise<'required' | 'unverified' | 'verified'> {
    return (await invokeCommand('get_identity_publishing_state')).mode;
  },
  async setPublishingMode(mode: 'unverified' | 'verified'): Promise<void> {
    return invokeCommand('set_identity_publishing_mode', { mode });
  },
};
