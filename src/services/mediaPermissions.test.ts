import { describe, expect, it, vi } from 'vitest';
import { requestCallMediaAccess } from './mediaPermissions';

function stream() {
  const track = { stop: vi.fn() };
  return {
    getTracks: () => [track],
  } as unknown as MediaStream;
}

describe('requestCallMediaAccess', () => {
  it('distinguishes a missing API from denied permissions', async () => {
    expect(await requestCallMediaAccess({} as never)).toMatchObject({
      microphone: 'missing_media_api',
      camera: 'missing_media_api',
    });

    const getUserMedia = vi
      .fn()
      .mockRejectedValueOnce(new DOMException('denied', 'NotAllowedError'))
      .mockResolvedValueOnce(stream());
    const result = await requestCallMediaAccess({ getUserMedia });
    expect(result.microphone).toBe('permission_denied');
    expect(result.camera).toBe('ready');
  });

  it('requests audio and video separately, releases tracks, and enumerates safely', async () => {
    const audio = stream();
    const video = stream();
    const getUserMedia = vi.fn().mockResolvedValueOnce(audio).mockResolvedValueOnce(video);
    const enumerateDevices = vi.fn().mockResolvedValue([
      { kind: 'audioinput' },
      { kind: 'videoinput' },
      { kind: 'audiooutput' },
    ]);

    const result = await requestCallMediaAccess({ getUserMedia, enumerateDevices });

    expect(getUserMedia).toHaveBeenNthCalledWith(1, { audio: true, video: false });
    expect(getUserMedia).toHaveBeenNthCalledWith(2, { audio: false, video: true });
    expect(result).toEqual({
      microphone: 'ready',
      camera: 'ready',
      audioInputCount: 1,
      videoInputCount: 1,
    });
    expect(audio.getTracks()[0].stop).toHaveBeenCalled();
    expect(video.getTracks()[0].stop).toHaveBeenCalled();
  });

  it('does not fail access checks when WebKit blocks enumeration', async () => {
    const getUserMedia = vi.fn().mockResolvedValue(stream());
    const result = await requestCallMediaAccess({
      getUserMedia,
      enumerateDevices: vi.fn().mockRejectedValue(new Error('labels unavailable')),
    });
    expect(result).toMatchObject({
      microphone: 'ready',
      camera: 'ready',
      audioInputCount: null,
      videoInputCount: null,
    });
  });
});
