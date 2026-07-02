import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  safeInvoke,
  showErrorToast,
  showSuccessToast,
  handleError,
  HarborError,
} from './errorHandler';
import { invoke } from '@tauri-apps/api/core';
import toast from 'react-hot-toast';

vi.mock('./logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

describe('safeInvoke', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should call invoke with command and args, returning the result', async () => {
    vi.mocked(invoke).mockResolvedValue({ data: 'test' });

    const result = await safeInvoke<{ data: string }>('test_command', { key: 'value' });

    expect(invoke).toHaveBeenCalledWith('test_command', { key: 'value' });
    expect(result).toEqual({ data: 'test' });
  });

  it('should throw HarborError on failure and show toast by default', async () => {
    vi.mocked(invoke).mockRejectedValue(new Error('Command failed'));

    await expect(safeInvoke('failing_command')).rejects.toThrow();
    expect(toast.error).toHaveBeenCalled();
  });

  it('should not show toast when showToast is false', async () => {
    vi.mocked(invoke).mockRejectedValue(new Error('Command failed'));

    await expect(safeInvoke('failing_command', undefined, { showToast: false })).rejects.toThrow();
    expect(toast.error).not.toHaveBeenCalled();
  });

  it('should retry on recoverable errors when retryCount > 0', async () => {
    vi.mocked(invoke)
      .mockRejectedValueOnce(new HarborError({ code: 'NETWORK_TIMEOUT', message: 'Timeout' }))
      .mockResolvedValueOnce('success');

    const result = await safeInvoke<string>('retry_command', undefined, {
      retryCount: 1,
      retryDelay: 10,
    });

    expect(result).toBe('success');
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it('should throw after all retries are exhausted', async () => {
    const error = new HarborError({
      code: 'NETWORK_TIMEOUT',
      message: 'Timeout',
    });
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(
      safeInvoke('retry_command', undefined, {
        retryCount: 1,
        retryDelay: 10,
      }),
    ).rejects.toThrow();
    expect(invoke).toHaveBeenCalledTimes(2);
  });
});

describe('showErrorToast', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should show error toast for HarborError', () => {
    const error = new HarborError({
      code: 'NETWORK_ERROR',
      message: 'Connection lost',
    });

    showErrorToast(error);

    expect(toast.error).toHaveBeenCalledWith('Connection lost', expect.any(Object));
  });

  it('should include recovery message when available', () => {
    const error = new HarborError({
      code: 'NETWORK_ERROR',
      message: 'Connection lost',
      recovery: 'Check your network settings',
    });

    showErrorToast(error);

    expect(toast.error).toHaveBeenCalledWith(
      'Connection lost\nCheck your network settings',
      expect.any(Object),
    );
  });

  it('should convert unknown errors to HarborError before displaying', () => {
    showErrorToast(new Error('Generic error'));

    expect(toast.error).toHaveBeenCalledWith('Generic error', expect.any(Object));
  });

  it('should use longer duration for critical errors', () => {
    const criticalError = new HarborError({
      code: 'DATABASE_ERROR',
      message: 'DB crashed',
    });

    showErrorToast(criticalError);

    expect(toast.error).toHaveBeenCalledWith('DB crashed', { duration: 6000 });
  });

  it('should use standard duration for non-critical errors', () => {
    const error = new HarborError({
      code: 'VALIDATION_ERROR',
      message: 'Invalid input',
    });

    showErrorToast(error);

    expect(toast.error).toHaveBeenCalledWith('Invalid input', { duration: 4000 });
  });
});

describe('showSuccessToast', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should show success toast with message', () => {
    showSuccessToast('Operation completed');

    expect(toast.success).toHaveBeenCalledWith('Operation completed', { duration: 3000 });
  });
});

describe('handleError', () => {
  it('should return a HarborError from unknown input', () => {
    const result = handleError(new Error('test error'));

    expect(result).toBeInstanceOf(HarborError);
    expect(result.message).toBe('test error');
  });

  it('should preserve existing HarborError', () => {
    const original = new HarborError({
      code: 'NOT_FOUND',
      message: 'Resource not found',
    });

    const result = handleError(original);

    expect(result).toBe(original);
  });

  it('should work with context parameter', () => {
    const result = handleError(new Error('fail'), 'loading user');

    expect(result).toBeInstanceOf(HarborError);
    expect(result.message).toBe('fail');
  });
});
