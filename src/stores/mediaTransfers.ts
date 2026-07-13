import { create } from 'zustand';
import { mediaService } from '../services/media';
import type { EnsureMediaTransferInput, MediaTransferState } from '../types';

const MAX_TRACKED_TRANSFERS = 512;
const pendingEnsures = new Map<string, Promise<MediaTransferState>>();

interface MediaTransfersStore {
  transfers: Record<string, MediaTransferState>;
  ensure: (input: EnsureMediaTransferInput) => Promise<MediaTransferState>;
  retry: (mediaHash: string) => Promise<void>;
  apply: (state: MediaTransferState) => void;
  reset: () => void;
}

function bounded(
  transfers: Record<string, MediaTransferState>,
  next: MediaTransferState,
): Record<string, MediaTransferState> {
  const merged = { ...transfers, [next.mediaHash]: next };
  const entries = Object.entries(merged);
  if (entries.length <= MAX_TRACKED_TRANSFERS) return merged;
  entries.sort(([, left], [, right]) => right.updatedAt - left.updatedAt);
  return Object.fromEntries(entries.slice(0, MAX_TRACKED_TRANSFERS));
}

export const useMediaTransfersStore = create<MediaTransfersStore>((set, get) => ({
  transfers: {},

  ensure: async (input) => {
    const existing = get().transfers[input.mediaHash];
    if (existing) return existing;
    const inFlight = pendingEnsures.get(input.mediaHash);
    if (inFlight) return inFlight;
    const request = mediaService.ensureTransfer(input).then((state) => {
      get().apply(state);
      return state;
    });
    pendingEnsures.set(input.mediaHash, request);
    try {
      return await request;
    } finally {
      pendingEnsures.delete(input.mediaHash);
    }
  },

  retry: async (mediaHash) => {
    try {
      const state = await mediaService.retryTransfer(mediaHash);
      get().apply(state);
    } catch (error) {
      const state = await mediaService.getTransfer(mediaHash).catch(() => null);
      if (state) get().apply(state);
      throw error;
    }
  },

  apply: (state) => set((current) => ({ transfers: bounded(current.transfers, state) })),
  reset: () => {
    pendingEnsures.clear();
    set({ transfers: {} });
  },
}));
