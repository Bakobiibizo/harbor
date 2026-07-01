import { create } from 'zustand';
import type { CallSession, NetworkEvent } from '../types';
import { callingService } from '../services/calling';

interface CallingState {
  activeCalls: CallSession[];
  callHistory: CallSession[];
  isLoading: boolean;
  error: string | null;
  lastEventPeerId: string | null;

  hydrateCalls: () => Promise<void>;
  refreshActiveCalls: () => Promise<void>;
  refreshCallHistory: (limit?: number) => Promise<void>;
  handleBackendEvent: (event: NetworkEvent) => Promise<void>;
  reset: () => void;
}

const initialState = {
  activeCalls: [] as CallSession[],
  callHistory: [] as CallSession[],
  isLoading: false,
  error: null as string | null,
  lastEventPeerId: null as string | null,
};

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export const useCallingStore = create<CallingState>((set, get) => ({
  ...initialState,

  hydrateCalls: async () => {
    set({ isLoading: true, error: null });
    try {
      const [activeCalls, callHistory] = await Promise.all([
        callingService.getActiveCalls(),
        callingService.getCallHistory(),
      ]);
      set({ activeCalls, callHistory, isLoading: false });
    } catch (error) {
      set({ error: toErrorMessage(error), isLoading: false });
    }
  },

  refreshActiveCalls: async () => {
    try {
      const activeCalls = await callingService.getActiveCalls();
      set({ activeCalls, error: null });
    } catch (error) {
      set({ error: toErrorMessage(error) });
    }
  },

  refreshCallHistory: async (limit = 100) => {
    try {
      const callHistory = await callingService.getCallHistory(limit);
      set({ callHistory, error: null });
    } catch (error) {
      set({ error: toErrorMessage(error) });
    }
  },

  handleBackendEvent: async (event: NetworkEvent) => {
    if (event.type !== 'call_signaling_received') {
      return;
    }

    set({ lastEventPeerId: event.peer_id });
    await get().hydrateCalls();
  },

  reset: () => set(initialState),
}));
