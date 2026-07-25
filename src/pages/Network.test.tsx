import { act, fireEvent, render, renderHook, screen, waitFor } from '@testing-library/react';
import toast from 'react-hot-toast';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { saveTextToDownloads } from '../services/downloads';
import * as networkService from '../services/network';
import { activateProfile, captureProfile, suspendProfile } from '../services/profileSession';
import type { RelayStatus } from '../stores/network';
import { CopyButton, DeployRelayContent, useShareableContactInvite } from './Network';

vi.mock('react-hot-toast', () => ({
  default: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('../services/downloads', () => ({
  saveTextToDownloads: vi.fn(),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

describe('Network copy actions', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    suspendProfile();
    activateProfile('network-test-profile');
  });

  it('does not report success until the clipboard write finishes', async () => {
    let finishWrite: (() => void) | undefined;
    const writeText = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finishWrite = resolve;
        }),
    );
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });

    render(<CopyButton text="harbor://contact" label="Invite copied" />);
    fireEvent.click(screen.getByTitle('Copy'));

    expect(writeText).toHaveBeenCalledWith('harbor://contact');
    expect(toast.success).not.toHaveBeenCalled();

    finishWrite?.();
    await waitFor(() => expect(toast.success).toHaveBeenCalledWith('Invite copied'));
  });

  it('reports a structured clipboard failure without showing a false success', async () => {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: vi.fn().mockRejectedValue({
          code: 'PERMISSION_DENIED',
          message: 'Clipboard permission was denied',
        }),
      },
    });

    render(<CopyButton text="harbor://contact" />);
    fireEvent.click(screen.getByTitle('Copy'));

    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith('Could not copy: Clipboard permission was denied'),
    );
    expect(toast.success).not.toHaveBeenCalled();
  });

  it('reports the clipboard rejection when native save and its fallback both fail', async () => {
    vi.mocked(saveTextToDownloads).mockRejectedValue({
      code: 'IO_ERROR',
      message: 'Downloads are unavailable',
    });
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: vi.fn().mockRejectedValue({
          code: 'PERMISSION_DENIED',
          message: 'Clipboard permission was denied',
        }),
      },
    });

    render(<DeployRelayContent />);
    fireEvent.click(screen.getByRole('button', { name: 'Download Relay Template' }));

    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(
        'Could not save the template or copy the fallback: Clipboard permission was denied',
      ),
    );
    expect(toast.success).not.toHaveBeenCalled();
  });

  it('does not restore a stale contact invite after disconnect', async () => {
    const invite = deferred<string>();
    vi.spyOn(networkService, 'getShareableContactString').mockReturnValue(invite.promise);
    const epoch = captureProfile()!.epoch;
    const { result, rerender } = renderHook(
      ({ running, status }: { running: boolean; status: RelayStatus }) =>
        useShareableContactInvite(running, status, epoch),
      {
        initialProps: { running: true, status: 'connected' as RelayStatus },
      },
    );
    await waitFor(() => expect(networkService.getShareableContactString).toHaveBeenCalledTimes(1));

    rerender({ running: false, status: 'disconnected' });
    await act(async () => {
      invite.resolve('harbor://stale-contact');
      await invite.promise;
    });

    expect(result.current).toBeNull();
  });

  it('keeps the current profile invite when the previous profile resolves last', async () => {
    const inviteA = deferred<string>();
    const inviteB = deferred<string>();
    vi.spyOn(networkService, 'getShareableContactString')
      .mockReturnValueOnce(inviteA.promise)
      .mockReturnValueOnce(inviteB.promise);
    const epochA = captureProfile()!.epoch;
    const { result, rerender } = renderHook(
      ({ epoch }) => useShareableContactInvite(true, 'connected', epoch),
      { initialProps: { epoch: epochA } },
    );
    await waitFor(() => expect(networkService.getShareableContactString).toHaveBeenCalledTimes(1));

    suspendProfile();
    const epochB = activateProfile('network-test-profile-b').epoch;
    rerender({ epoch: epochB });
    await waitFor(() => expect(networkService.getShareableContactString).toHaveBeenCalledTimes(2));
    await act(async () => {
      inviteB.resolve('harbor://current-contact');
      await inviteB.promise;
    });
    expect(result.current).toBe('harbor://current-contact');

    await act(async () => {
      inviteA.resolve('harbor://stale-contact');
      await inviteA.promise;
    });
    expect(result.current).toBe('harbor://current-contact');
  });
});
