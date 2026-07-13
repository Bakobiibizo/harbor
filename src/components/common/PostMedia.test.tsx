import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { PostMedia } from './PostMedia';
import { mediaService } from '../../services/media';
import { useMediaTransfersStore } from '../../stores/mediaTransfers';
import type { MediaTransferState } from '../../types';

vi.mock('../../services/media', () => ({
  mediaService: {
    ensureTransfer: vi.fn(),
    getMediaUrl: vi.fn(),
    retryTransfer: vi.fn(),
    getTransfer: vi.fn(),
  },
}));

const hash = 'a'.repeat(64);
const baseState: MediaTransferState = {
  mediaHash: hash,
  sourcePeerId: 'peer-alice',
  mediaType: 'image',
  mimeType: 'image/png',
  fileName: 'photo.png',
  totalBytes: 100,
  bytesReceived: 0,
  status: 'queued',
  attemptCount: 0,
  errorCode: null,
  errorMessage: null,
  updatedAt: 1,
};

describe('PostMedia lifecycle', () => {
  beforeEach(() => {
    useMediaTransfersStore.getState().reset();
    vi.clearAllMocks();
    vi.mocked(mediaService.ensureTransfer).mockResolvedValue(baseState);
    vi.mocked(mediaService.retryTransfer).mockResolvedValue({
      ...baseState,
      status: 'retrying',
      attemptCount: 1,
    });
    vi.mocked(mediaService.getTransfer).mockResolvedValue(null);
  });

  it('keeps an accessible non-empty placeholder while remote bytes are absent', async () => {
    render(
      <PostMedia
        media={[
          {
            type: 'image',
            url: hash,
            sourcePeerId: 'peer-alice',
            mimeType: 'image/png',
          },
        ]}
      />,
    );

    expect(await screen.findByRole('status', { name: /image attachment/i })).toBeTruthy();
    expect(screen.getByText(/Attachment queued|Retrying attachment transfer/i)).toBeTruthy();
  });

  it('shows bounded byte progress and resolves the player when ready', async () => {
    render(
      <PostMedia
        media={[{ type: 'image', url: hash, sourcePeerId: 'peer-alice', totalBytes: 100 }]}
      />,
    );
    await screen.findByRole('status');
    useMediaTransfersStore.getState().apply({
      ...baseState,
      status: 'transferring',
      bytesReceived: 40,
    });
    expect(await screen.findByText(/40%/)).toBeTruthy();

    vi.mocked(mediaService.getMediaUrl).mockResolvedValue('data:image/png;base64,AAAA');
    useMediaTransfersStore.getState().apply({
      ...baseState,
      status: 'ready',
      bytesReceived: 100,
    });
    await waitFor(() => expect(screen.getByRole('img')).toBeTruthy());
  });

  it('offers an explicit retry without exposing the content hash', async () => {
    useMediaTransfersStore.getState().apply({
      ...baseState,
      status: 'failed',
      errorCode: 'transport_timeout',
      errorMessage: 'The source did not respond.',
    });
    render(<PostMedia media={[{ type: 'audio', url: hash, sourcePeerId: 'peer-alice' }]} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(mediaService.retryTransfer).toHaveBeenCalledWith(hash));
    expect(screen.queryByText(hash)).toBeNull();
  });
});
