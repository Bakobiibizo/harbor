import { create } from 'zustand';
import type { IdentityState, CreateIdentityRequest } from '../types';
import { identityService } from '../services';

/** Extract error message from various error types (including Tauri errors) */
function getErrorMessage(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  if (typeof err === 'string') {
    return err;
  }
  if (err && typeof err === 'object') {
    // Tauri errors might have a message property
    if ('message' in err && typeof err.message === 'string') {
      return err.message;
    }
    // Or an error property
    if ('error' in err && typeof err.error === 'string') {
      return err.error;
    }
    // Try to stringify for debugging, but provide a fallback
    try {
      const str = JSON.stringify(err);
      if (str && str !== '{}') {
        return str;
      }
    } catch {
      // Ignore stringify errors
    }
  }
  return 'An unknown error occurred';
}

interface IdentityStore {
  state: IdentityState;
  error: string | null;

  // Actions
  initialize: () => Promise<void>;
  createIdentity: (request: CreateIdentityRequest) => Promise<import('../types').IdentityInfo>;
  unlock: (passphrase: string) => Promise<void>;
  lock: () => Promise<void>;
  updateDisplayName: (displayName: string) => Promise<void>;
  updateBio: (bio: string | null) => Promise<void>;
  updatePassphraseHint: (hint: string | null) => Promise<void>;
  clearError: () => void;
}

export const useIdentityStore = create<IdentityStore>((set, get) => ({
  state: { status: 'loading' },
  error: null,

  initialize: async () => {
    try {
      set({ state: { status: 'loading' }, error: null });

      const hasIdentity = await identityService.hasIdentity();

      if (!hasIdentity) {
        set({ state: { status: 'no_identity' } });
        return;
      }

      const identity = await identityService.getIdentityInfo();
      if (!identity) {
        set({ state: { status: 'no_identity' } });
        return;
      }

      const isUnlocked = await identityService.isUnlocked();

      if (isUnlocked) {
        set({ state: { status: 'unlocked', identity } });
      } else {
        set({ state: { status: 'locked', identity } });
      }
    } catch (err) {
      set({
        state: { status: 'no_identity' },
        error: getErrorMessage(err),
      });
    }
  },

  createIdentity: async (request: CreateIdentityRequest) => {
    try {
      set({ error: null });
      const identity = await identityService.createIdentity(request);
      set({ state: { status: 'unlocked', identity } });
      return identity;
    } catch (err) {
      set({ error: getErrorMessage(err) });
      throw err;
    }
  },

  unlock: async (passphrase: string) => {
    try {
      set({ error: null });
      const identity = await identityService.unlock(passphrase);
      set({ state: { status: 'unlocked', identity } });
    } catch (err) {
      set({ error: getErrorMessage(err) });
      throw err;
    }
  },

  lock: async () => {
    try {
      await identityService.lock();
      const { state } = get();
      if (state.status === 'unlocked') {
        set({ state: { status: 'locked', identity: state.identity } });
      }
    } catch (err) {
      set({ error: getErrorMessage(err) });
    }
  },

  updateDisplayName: async (displayName: string) => {
    try {
      await identityService.updateDisplayName(displayName);
      const { state } = get();
      if (state.status === 'unlocked' || state.status === 'locked') {
        set({
          state: {
            ...state,
            identity: { ...state.identity, displayName },
          },
        });
      }
    } catch (err) {
      set({ error: getErrorMessage(err) });
    }
  },

  updateBio: async (bio: string | null) => {
    try {
      await identityService.updateBio(bio);
      const { state } = get();
      if (state.status === 'unlocked' || state.status === 'locked') {
        set({
          state: {
            ...state,
            identity: { ...state.identity, bio },
          },
        });
      }
    } catch (err) {
      set({ error: getErrorMessage(err) });
    }
  },

  updatePassphraseHint: async (hint: string | null) => {
    try {
      await identityService.updatePassphraseHint(hint);
      const { state } = get();
      if (state.status === 'unlocked' || state.status === 'locked') {
        set({
          state: {
            ...state,
            identity: { ...state.identity, passphraseHint: hint },
          },
        });
      }
    } catch (err) {
      set({ error: getErrorMessage(err) });
    }
  },

  clearError: () => set({ error: null }),
}));
