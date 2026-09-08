import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invokeCommand } from './command';
import { approveGameSigningRequest, importGamePackage, installStoreGame } from './games';

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

  it('passes explicit permission approvals to package installation commands', async () => {
    vi.mocked(invokeCommand).mockResolvedValue({});
    await importGamePackage('/tmp/game.harborgame', ['save_data']);
    expect(invokeCommand).toHaveBeenCalledWith('import_game_package', {
      approvedPermissions: ['save_data'],
      filePath: '/tmp/game.harborgame',
    });
    await installStoreGame('game-1', 'version-1', ['save_data']);
    expect(invokeCommand).toHaveBeenCalledWith('install_store_game', {
      approvedPermissions: ['save_data'],
      gameId: 'game-1',
      versionId: 'version-1',
    });
  });
});
