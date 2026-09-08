import { create } from 'zustand';
import type { GameSigningRequest } from '../types';

interface GameSigningState {
  requests: GameSigningRequest[];
  enqueue: (request: GameSigningRequest) => void;
  remove: (approvalId: string) => void;
  reset: () => void;
}

export const useGameSigningStore = create<GameSigningState>((set) => ({
  requests: [],
  enqueue: (request) =>
    set((state) =>
      state.requests.some((item) => item.approvalId === request.approvalId)
        ? state
        : { requests: [...state.requests, request] },
    ),
  remove: (approvalId) =>
    set((state) => ({
      requests: state.requests.filter((request) => request.approvalId !== approvalId),
    })),
  reset: () => set({ requests: [] }),
}));
