import { create } from 'zustand';
import type { IdentityState, CreateIdentityRequest } from '../types';
import { identityService, networkService } from '../services';

export type IdentityClaimProgress =
  'preparing' | 'connecting' | 'waiting-for-relay' | 'registering' | 'verifying' | 'saving';

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
  /NO_ACTIVE_RELAY|no active relay|offline|unavailable|not initialized|network service|old relay|timed out/i;

async function withTimeout<T>(promise: Promise<T>, timeoutMs: number, message: string): Promise<T> {
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      promise,
      new Promise<never>((_, reject) => {
        timeout = setTimeout(() => reject(new Error(message)), timeoutMs);
      }),
    ]);
  } finally {
    if (timeout) clearTimeout(timeout);
  }
}

async function restoreVerifiedIdentity(
  identity: import('../types').IdentityInfo,
): Promise<import('../types').IdentityInfo> {
  const entry = await withTimeout(
    identityService.getIdentityEntryState(),
    10_000,
    'Harbor could not finish checking your saved name. Retry after checking your connection.',
  );
  if (!entry.claim) return identity;
  if (entry.claim.request.peerId !== identity.peerId) {
    throw new Error('The saved Harbor name does not belong to this identity.');
  }
  if (entry.mode !== 'verified') {
    await withTimeout(
      identityService.setMigrationMode('verified'),
      10_000,
      'Harbor verified your saved name but could not save its restored state. Please retry.',
    );
  }
  return { ...identity, relayNameClaim: entry.claim, relayNameVerified: true };
}

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
    onProgress?: (progress: IdentityClaimProgress) => void,
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
        let restoredIdentity = identity;
        try {
          restoredIdentity = await restoreVerifiedIdentity(identity);
        } catch (restoreError) {
          // Unlocking and legacy migration must remain available even if a saved claim is damaged
          // or temporarily unreadable. The migration gate will show the actionable recovery UI.
          set({ error: getErrorMessage(restoreError) });
        }
        set({ state: { status: 'unlocked', identity: restoredIdentity } });
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
  completeOnboarding: async (request, name, namespace, onProgress) => {
    set({ error: null });
    try {
      onProgress?.('preparing');
      let identity;
      if (await identityService.hasIdentity()) {
        identity = await identityService.getIdentityInfo();
        if (!identity) throw new Error('Harbor could not resume the local identity.');
        if (!(await identityService.isUnlocked()))
          throw new Error('Unlock this identity to resume name registration.');
      } else {
        identity = await identityService.createIdentity(request);
      }
      onProgress?.('connecting');
      await withTimeout(
        networkService.startNetwork(),
        15_000,
        'Harbor could not start networking in time. Please retry.',
      );
      await withTimeout(
        networkService.connectToPublicRelays(),
        15_000,
        'Harbor could not connect to a relay in time. Check your connection and retry.',
      );
      onProgress?.('waiting-for-relay');
      await waitForActiveRelay();
      let claim;
      for (let attempt = 0; ; attempt++) {
        try {
          onProgress?.('registering');
          claim = await withTimeout(
            identityService.registerRelayName({ name, namespace }),
            15_000,
            'Name registration timed out. Check your relay connection and retry.',
          );
          break;
        } catch (err) {
          if (attempt >= 1 || !relayRetryPattern.test(getErrorMessage(err))) throw err;
          await new Promise((r) => setTimeout(r, 300));
        }
      }
      onProgress?.('verifying');
      if (
        claim.request.peerId !== identity.peerId ||
        !(await withTimeout(
          identityService.verifyNameClaim(claim),
          10_000,
          'Harbor could not verify the relay response in time. Please retry.',
        ))
      )
        throw new Error('Harbor could not verify the relay name claim.');
      onProgress?.('saving');
      await withTimeout(
        identityService.setMigrationMode('verified'),
        10_000,
        'Harbor verified your name but could not save it in time. Please retry.',
      );
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
      let restoredIdentity = identity;
      try {
        restoredIdentity = await restoreVerifiedIdentity(identity);
      } catch (restoreError) {
        // The identity is still securely unlocked. Preserve access to the migration recovery gate.
        set({ error: getErrorMessage(restoreError) });
      }
      set({ state: { status: 'unlocked', identity: restoredIdentity } });
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
