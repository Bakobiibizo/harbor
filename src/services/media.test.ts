import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mediaService } from './media';
import { invokeCommand } from './command';
import { open } from '@tauri-apps/plugin-dialog';
import { convertFileSrc } from '@tauri-apps/api/core';

vi.mock('./command', () => ({ invokeCommand: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: vi.fn((path) => `asset:${path}`) }));

describe('media service file lifecycle', () => {
  beforeEach(() => vi.clearAllMocks());

  it('imports a selected path without reading bytes in the webview', async () => {
    vi.mocked(open).mockResolvedValue('/home/user/photo.png');
    vi.mocked(invokeCommand)
      .mockResolvedValueOnce({
        mediaHash: 'a'.repeat(64),
        mimeType: 'image/png',
        fileName: 'photo.png',
        totalBytes: 123,
      })
      .mockResolvedValueOnce({
        filePath: '/app/media/photo.png',
        mimeType: 'image/png',
        totalBytes: 123,
      });

    const selected = await mediaService.selectAndStore(['image']);

    expect(invokeCommand).toHaveBeenNthCalledWith(1, 'store_media', {
      filePath: '/home/user/photo.png',
      mimeType: undefined,
    });
    expect(convertFileSrc).toHaveBeenCalledWith('/app/media/photo.png');
    expect(selected).toMatchObject({
      mediaHash: 'a'.repeat(64),
      previewUrl: 'asset:/app/media/photo.png',
    });
    expect(JSON.stringify(vi.mocked(invokeCommand).mock.calls)).not.toContain('data');
  });

  it('preserves the selection boundary when import fails', async () => {
    vi.mocked(open).mockResolvedValue('/home/user/clip.mp4');
    vi.mocked(invokeCommand).mockRejectedValue(new Error('disk full'));

    await expect(mediaService.selectAndStore(['video'])).rejects.toThrow('disk full');
    expect(invokeCommand).toHaveBeenCalledTimes(1);
  });
});
