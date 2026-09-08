import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invokeCommand } from './command';
import { approveGameSigningRequest } from './games';

vi.mock('./command', () => ({ invokeCommand: vi.fn() }));

describe('game signing service', () => {
  beforeEach(() => vi.clearAllMocks());

  it('passes only the opaque approval ID to the backend', async () => {
    vi.mocked(invokeCommand).mockResolvedValue({ status: 'delivered', requestId: 'request-1' });
    await approveGameSigningRequest('approval-1');
    expect(invokeCommand).toHaveBeenCalledWith('approve_game_signing_request', {
      approvalId: 'approval-1',
    });
  });
});
