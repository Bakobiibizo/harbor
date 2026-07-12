import { create } from 'zustand';
import type { IdentityState, CreateIdentityRequest } from '../types';
import { identityService, networkService } from '../services';

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

const relayRetryPattern =
  /NO_ACTIVE_RELAY|no active relay|offline|unavailable|not initialized|network service|old relay/i;

async function waitForActiveRelay(): Promise<void> {
  let lastError: unknown;
  for (let attempt = 0; attempt < 50; attempt++) {
    try {
      const stats = await networkService.getNetworkStats();
      if (stats.relayAddresses.length > 0) return;
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  const detail = lastError ? ` ${getErrorMessage(lastError)}` : '';
  throw new Error(`Harbor connected to the network, but the relay is not ready yet.${detail}`);
}

interface IdentityStore {
  state: IdentityState;
  error: string | null;

  // Actions
  initialize: () => Promise<void>;
  createIdentity: (request: CreateIdentityRequest) => Promise<import('../types').IdentityInfo>;
  completeOnboarding: (
    request: CreateIdentityRequest,
    name: string,
    namespace: string,
  ) => Promise<import('../types').IdentityInfo>;
  unlock: (passphrase: string) => Promise<void>;
  lock: () => Promise<void>;
  updateDisplayName: (displayName: string) => Promise<void>;
  updateBio: (bio: string | null) => Promise<void>;
  updatePassphraseHint: (hint: string | null) => Promise<void>;
  clearError: () => void;
  attachVerifiedRelayName: (claim: import('../types').RelayNameClaim) => void;
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
  completeOnboarding: async (request, name, namespace) => {
    set({ error: null });
    try {
      let identity;
      if (await identityService.hasIdentity()) {
        identity = await identityService.getIdentityInfo();
        if (!identity) throw new Error('Harbor could not resume the local identity.');
        if (!(await identityService.isUnlocked()))
          throw new Error('Unlock this identity to resume name registration.');
      } else {
        identity = await identityService.createIdentity(request);
      }
      await networkService.startNetwork();
      await networkService.connectToPublicRelays();
      await waitForActiveRelay();
      let claim;
      for (let attempt = 0; ; attempt++) {
        try {
          claim = await identityService.registerRelayName({ name, namespace });
          break;
        } catch (err) {
          if (
            attempt >= 9 ||
            !relayRetryPattern.test(getErrorMessage(err))
          )
            throw err;
          await new Promise((r) => setTimeout(r, 300));
        }
      }
      if (
        claim.request.peerId !== identity.peerId ||
        !(await identityService.verifyNameClaim(claim))
      )
        throw new Error('Harbor could not verify the relay name claim.');
      await identityService.setMigrationMode('verified');
      const complete = { ...identity, relayNameClaim: claim, relayNameVerified: true };
      set({ state: { status: 'unlocked', identity: complete } });
      return complete;
    } catch (err) {
      const message = getErrorMessage(err);
      set({ error: message });
      throw new Error(message);
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
  attachVerifiedRelayName: (claim) => {
    const { state } = get();
    if (
      (state.status === 'unlocked' || state.status === 'locked') &&
      claim.request.peerId === state.identity.peerId
    ) {
      set({
        state: {
          ...state,
          identity: { ...state.identity, relayNameClaim: claim, relayNameVerified: true },
        },
      });
    }
  },
}));
