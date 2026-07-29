import { create } from 'zustand';
import type { IdentityState, CreateIdentityRequest } from '../types';
import { identityService, networkService } from '../services';
import { suspendProfile } from '../services/profileSession';
import { getErrorMessage, HarborError } from '../utils/errors';

export type IdentityClaimProgress =
  'preparing' | 'connecting' | 'waiting-for-relay' | 'registering' | 'verifying' | 'saving';

function transportInitializationError(err: unknown) {
  return {
    status: 'recoverableError' as const,
    source: 'ipc' as const,
    error: {
      code: 'IPC_ERROR',
      message: getErrorMessage(err),
      recovery: 'Retry. If the problem continues, restart Harbor.',
    },
  };
}

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
      identityService.setPublishingMode('verified'),
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

const RETRYABLE_RELAY_CODES = new Set([
  'NETWORK_NOT_INITIALIZED',
  'NETWORK_SERVICE_UNAVAILABLE',
  'NETWORK_CONNECTION_FAILED',
  'NETWORK_PEER_UNREACHABLE',
  'NETWORK_TIMEOUT',
]);

function isRetryableRelayFailure(error: unknown): boolean {
  return RETRYABLE_RELAY_CODES.has(HarborError.fromUnknown(error).code);
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
  changePassword: (currentPassword: string, newPassword: string) => Promise<void>;
  lock: () => Promise<void>;
  updateDisplayName: (displayName: string) => Promise<void>;
  updateBio: (bio: string | null) => Promise<void>;
  updateProfileAvatar: (filePath: string | null) => Promise<void>;
  updatePassphraseHint: (hint: string | null) => Promise<void>;
  clearError: () => void;
  attachVerifiedRelayName: (claim: import('../types').RelayNameClaim) => void;
  resetRuntimeSession: () => void;
}

let lifecycleGeneration = 0;

export const useIdentityStore = create<IdentityStore>((set, get) => ({
  state: { status: 'loading' },
  error: null,

  initialize: async () => {
    const generation = lifecycleGeneration;
    set({ state: { status: 'loading' }, error: null });
    try {
      const initialization = await identityService.getInitializationState();
      if (generation !== lifecycleGeneration) return;

      if (initialization.status === 'unlocked') {
        let restoredIdentity = initialization.identity;
        try {
          restoredIdentity = await restoreVerifiedIdentity(initialization.identity);
        } catch (restoreError) {
          if (generation !== lifecycleGeneration) return;
          // Unlocking and legacy migration must remain available even if a saved claim is damaged
          // or temporarily unreadable. The migration gate will show the actionable recovery UI.
          set({ error: getErrorMessage(restoreError) });
        }
        if (generation !== lifecycleGeneration) return;
        set({ state: { status: 'unlocked', identity: restoredIdentity } });
        return;
      }

      set({
        state: initialization,
        error:
          initialization.status === 'recoverableError' || initialization.status === 'fatalError'
            ? initialization.error.message
            : null,
      });
    } catch (err) {
      if (generation !== lifecycleGeneration) return;
      const failure = transportInitializationError(err);
      set({
        state: failure,
        error: failure.error.message,
      });
    }
  },

  createIdentity: async (request: CreateIdentityRequest) => {
    const generation = lifecycleGeneration;
    try {
      set({ error: null });
      const identity = await identityService.createIdentity(request);
      if (generation === lifecycleGeneration) set({ state: { status: 'unlocked', identity } });
      return identity;
    } catch (err) {
      if (generation === lifecycleGeneration) set({ error: getErrorMessage(err) });
      throw err;
    }
  },
  completeOnboarding: async (request, name, namespace, onProgress) => {
    const generation = lifecycleGeneration;
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
      for (let attempt = 0; ; attempt += 1) {
        try {
          onProgress?.('registering');
          claim = await withTimeout(
            identityService.registerRelayName({ name, namespace }),
            15_000,
            'Name registration timed out. Check your relay connection and retry.',
          );
          break;
        } catch (error) {
          if (attempt >= 1 || !isRetryableRelayFailure(error)) throw error;
          await new Promise((resolve) => setTimeout(resolve, 300));
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
        identityService.setPublishingMode('verified'),
        10_000,
        'Harbor verified your name but could not save it in time. Please retry.',
      );
      const complete = { ...identity, relayNameClaim: claim, relayNameVerified: true };
      if (generation === lifecycleGeneration)
        set({ state: { status: 'unlocked', identity: complete } });
      return complete;
    } catch (err) {
      const message = getErrorMessage(err);
      if (generation === lifecycleGeneration) set({ error: message });
      throw err;
    }
  },

  unlock: async (passphrase: string) => {
    const generation = lifecycleGeneration;
    try {
      set({ error: null });
      const identity = await identityService.unlock(passphrase);
      if (generation !== lifecycleGeneration) return;
      let restoredIdentity = identity;
      try {
        restoredIdentity = await restoreVerifiedIdentity(identity);
      } catch (restoreError) {
        if (generation !== lifecycleGeneration) return;
        // The identity is still securely unlocked. Preserve access to the migration recovery gate.
        set({ error: getErrorMessage(restoreError) });
      }
      if (generation !== lifecycleGeneration) return;
      set({ state: { status: 'unlocked', identity: restoredIdentity } });
    } catch (err) {
      if (generation === lifecycleGeneration) set({ error: getErrorMessage(err) });
      throw err;
    }
  },

  changePassword: async (currentPassword: string, newPassword: string) => {
    const generation = lifecycleGeneration;
    try {
      set({ error: null });
      await identityService.changePassword(currentPassword, newPassword);
    } catch (err) {
      if (generation === lifecycleGeneration) set({ error: getErrorMessage(err) });
      throw err;
    }
  },

  lock: async () => {
    try {
      const current = get().state;
      const lockedIdentity =
        current.status === 'unlocked' || current.status === 'locked' ? current.identity : null;
      await identityService.lock();
      suspendProfile();
      set({
        state: lockedIdentity
          ? { status: 'locked', identity: lockedIdentity }
          : { status: 'loading' },
        error: null,
      });
    } catch (err) {
      set({ error: getErrorMessage(err) });
      throw err;
    }
  },

  updateDisplayName: async (displayName: string) => {
    const generation = lifecycleGeneration;
    try {
      await identityService.updateDisplayName(displayName);
      if (generation !== lifecycleGeneration) return;
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
      if (generation === lifecycleGeneration) set({ error: getErrorMessage(err) });
      throw err;
    }
  },

  updateBio: async (bio: string | null) => {
    const generation = lifecycleGeneration;
    try {
      await identityService.updateBio(bio);
      if (generation !== lifecycleGeneration) return;
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
      if (generation === lifecycleGeneration) set({ error: getErrorMessage(err) });
      throw err;
    }
  },

  updateProfileAvatar: async (filePath: string | null) => {
    const generation = lifecycleGeneration;
    try {
      const identity = await identityService.updateProfileAvatar(filePath);
      if (generation === lifecycleGeneration) {
        set({ state: { status: 'unlocked', identity }, error: null });
      }
    } catch (err) {
      if (generation === lifecycleGeneration) set({ error: getErrorMessage(err) });
      throw err;
    }
  },

  updatePassphraseHint: async (hint: string | null) => {
    const generation = lifecycleGeneration;
    try {
      await identityService.updatePassphraseHint(hint);
      if (generation !== lifecycleGeneration) return;
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
      if (generation === lifecycleGeneration) set({ error: getErrorMessage(err) });
      throw err;
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
  resetRuntimeSession: () => {
    lifecycleGeneration += 1;
    const { state } = get();
    set({
      state:
        state.status === 'locked' || state.status === 'unlocked'
          ? { status: 'locked', identity: state.identity }
          : { status: 'loading' },
      error: null,
    });
  },
}));
