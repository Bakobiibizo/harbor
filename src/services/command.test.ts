import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { HarborError } from '../utils/errors';
import { invokeCommand } from './command';

describe('invokeCommand', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('invokes a no-argument command without manufacturing an argument object', async () => {
    vi.mocked(invoke).mockResolvedValue(true);

    await expect(invokeCommand('is_network_running')).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith('is_network_running');
  });

  it('passes the command contract arguments through unchanged', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await invokeCommand('edit_message', {
      messageId: 'message-1',
      newContent: 'updated',
      peerId: 'peer-1',
    });

    expect(invoke).toHaveBeenCalledWith('edit_message', {
      messageId: 'message-1',
      newContent: 'updated',
      peerId: 'peer-1',
    });
  });

  it('preserves a structured backend error and records the failing command', async () => {
    vi.mocked(invoke).mockRejectedValue({
      code: 'NETWORK_NOT_INITIALIZED',
      message: 'Network has not been started',
      recovery: 'Start the network and try again',
    });

    const rejection = invokeCommand('bootstrap_network');
    await expect(rejection).rejects.toMatchObject({
      name: 'HarborError',
      code: 'NETWORK_NOT_INITIALIZED',
      message: 'Network has not been started',
      recovery: 'Start the network and try again',
      command: 'bootstrap_network',
    });
  });

  it('normalizes opaque objects without exposing their fields in the public message', async () => {
    vi.mocked(invoke).mockRejectedValue({ accessToken: 'do-not-render' });

    try {
      await invokeCommand('get_contacts');
      throw new Error('expected command to reject');
    } catch (error) {
      expect(error).toBeInstanceOf(HarborError);
      const harborError = error as HarborError;
      expect(harborError.message).toBe('An unexpected error occurred');
      expect(harborError.message).not.toContain('[object Object]');
      expect(harborError.message).not.toContain('do-not-render');
      expect(harborError.command).toBe('get_contacts');
    }
  });
});
