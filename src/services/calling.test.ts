import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { callingService } from './calling';

describe('callingService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('hydrates active calls from backend persistence', async () => {
    vi.mocked(invoke).mockResolvedValue([
      {
        callId: 'call-1',
        peerId: 'peer-alice',
        callerPeerId: 'peer-local',
        calleePeerId: 'peer-alice',
        direction: 'outgoing',
        mediaKind: 'audio',
        state: 'ringing',
        startedAt: 100,
        endedAt: null,
        durationSeconds: null,
        terminalReason: null,
      },
    ]);

    const active = await callingService.getActiveCalls();

    expect(invoke).toHaveBeenCalledWith('get_active_calls');
    expect(active).toHaveLength(1);
    expect(active[0].callId).toBe('call-1');
  });

  it('loads call history with an explicit limit', async () => {
    vi.mocked(invoke).mockResolvedValue([]);

    await callingService.getCallHistory(25);

    expect(invoke).toHaveBeenCalledWith('get_call_history', { limit: 25 });
  });
});
