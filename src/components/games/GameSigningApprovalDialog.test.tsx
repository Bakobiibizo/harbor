import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { GameSigningApprovalDialog } from './GameSigningApprovalDialog';
import { approveGameSigningRequest } from '../../services/games';
import type { GameSigningRequest, IdentityInfo } from '../../types';

vi.mock('../../services/games', () => ({ approveGameSigningRequest: vi.fn() }));

const identity: IdentityInfo = {
  peerId: '12D3KooWCreator',
  publicKey: 'public',
  x25519Public: 'agreement',
  displayName: 'Creator',
  avatarHash: null,
  bio: null,
  passphraseHint: null,
  createdAt: 1,
  updatedAt: 1,
};

const packageRequest: GameSigningRequest = {
  kind: 'package',
  approvalId: 'approval-1',
  expiresAt: 1_900_000_000,
  gameId: 'game-1',
  packageDigest: 'a'.repeat(64),
  permissions: ['save_data'],
  requestId: 'request-1',
  title: 'Fixture Game',
  versionId: 'version-1',
};

describe('GameSigningApprovalDialog', () => {
  beforeEach(() => vi.clearAllMocks());

  it('shows the exact package identity, digest, and permissions before approval', () => {
    render(
      <GameSigningApprovalDialog identity={identity} request={packageRequest} onClose={vi.fn()} />,
    );
    expect(screen.getByText('Fixture Game')).toBeInTheDocument();
    expect(screen.getByText('a'.repeat(64))).toBeInTheDocument();
    expect(screen.getByText('save_data')).toBeInTheDocument();
    expect(screen.getByText('12D3KooWCreator')).toBeInTheDocument();
  });

  it('signs only after approval and closes only after delivery', async () => {
    const onClose = vi.fn();
    vi.mocked(approveGameSigningRequest).mockResolvedValue({
      status: 'delivered',
      requestId: 'request-1',
    });
    render(
      <GameSigningApprovalDialog identity={identity} request={packageRequest} onClose={onClose} />,
    );
    expect(approveGameSigningRequest).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Sign Package' }));
    await waitFor(() => expect(approveGameSigningRequest).toHaveBeenCalledWith('approval-1'));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('keeps a retained proof available for delivery retry', async () => {
    const onClose = vi.fn();
    vi.mocked(approveGameSigningRequest).mockResolvedValue({
      status: 'pending',
      requestId: 'request-1',
      error: 'service unavailable',
    });
    render(
      <GameSigningApprovalDialog identity={identity} request={packageRequest} onClose={onClose} />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Sign Package' }));
    await waitFor(() => expect(approveGameSigningRequest).toHaveBeenCalledOnce());
    expect(onClose).not.toHaveBeenCalled();
  });
});
