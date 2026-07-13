import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mediaService } from '../services/media';
import { useMediaTransfersStore } from './mediaTransfers';
import type { EnsureMediaTransferInput, MediaTransferState } from '../types';

vi.mock('../services/media', () => ({
  mediaService: {
    ensureTransfer: vi.fn(),
    retryTransfer: vi.fn(),
    getTransfer: vi.fn(),
  },
}));

const input: EnsureMediaTransferInput = {
  mediaHash: 'c'.repeat(64),
  sourcePeerId: 'peer-alice',
  mediaType: 'video',
};

function state(overrides: Partial<MediaTransferState> = {}): MediaTransferState {
  return {
    mediaHash: input.mediaHash,
    sourcePeerId: 'peer-alice',
    mediaType: 'video',
    mimeType: 'video/mp4',
    fileName: null,
    totalBytes: null,
    bytesReceived: 0,
    status: 'queued',
    attemptCount: 0,
    errorCode: null,
    errorMessage: null,
    updatedAt: 1,
    ...overrides,
  };
}

describe('media transfer store', () => {
  beforeEach(() => {
    useMediaTransfersStore.getState().reset();
    vi.clearAllMocks();
  });

  it('deduplicates concurrent discovery and duplicate lifecycle events', async () => {
    vi.mocked(mediaService.ensureTransfer).mockResolvedValue(state());
    await Promise.all([
      useMediaTransfersStore.getState().ensure(input),
      useMediaTransfersStore.getState().ensure(input),
    ]);
    expect(mediaService.ensureTransfer).toHaveBeenCalledTimes(1);

    const ready = state({ status: 'ready', updatedAt: 2 });
    useMediaTransfersStore.getState().apply(ready);
    useMediaTransfersStore.getState().apply(ready);
    expect(Object.keys(useMediaTransfersStore.getState().transfers)).toEqual([input.mediaHash]);
    expect(useMediaTransfersStore.getState().transfers[input.mediaHash].status).toBe('ready');
  });

  it('deduplicates concurrent retries for the same attachment', async () => {
    let release!: (value: MediaTransferState) => void;
    vi.mocked(mediaService.retryTransfer).mockReturnValue(
      new Promise((resolve) => {
        release = resolve;
      }),
    );
    const first = useMediaTransfersStore.getState().retry(input.mediaHash);
    const second = useMediaTransfersStore.getState().retry(input.mediaHash);
    expect(mediaService.retryTransfer).toHaveBeenCalledTimes(1);
    release(state({ status: 'retrying' }));
    await Promise.all([first, second]);
  });

  it('does not repopulate transfer state from a promise completed after reset', async () => {
    let release!: (value: MediaTransferState) => void;
    vi.mocked(mediaService.ensureTransfer).mockReturnValue(
      new Promise((resolve) => {
        release = resolve;
      }),
    );
    const pending = useMediaTransfersStore.getState().ensure(input);
    useMediaTransfersStore.getState().reset();
    release(state());
    await pending;
    expect(useMediaTransfersStore.getState().transfers).toEqual({});
  });

  it('bounds lifecycle memory to the most recent 512 attachments', () => {
    for (let index = 0; index < 520; index += 1) {
      useMediaTransfersStore
        .getState()
        .apply(state({ mediaHash: index.toString(16).padStart(64, '0'), updatedAt: index }));
    }
    const transfers = useMediaTransfersStore.getState().transfers;
    expect(Object.keys(transfers)).toHaveLength(512);
    expect(transfers['0'.repeat(64)]).toBeUndefined();
  });
});
