import { describe, it, expect, vi, beforeEach } from 'vitest';
import * as loggingService from './logging';
import { invoke } from '@tauri-apps/api/core';

describe('loggingService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('exportLogs', () => {
    it('should invoke export_logs', async () => {
      vi.mocked(invoke).mockResolvedValue('log line 1\nlog line 2');

      const result = await loggingService.exportLogs();

      expect(invoke).toHaveBeenCalledWith('export_logs');
      expect(result).toBe('log line 1\nlog line 2');
    });
  });

  describe('getLogPath', () => {
    it('should invoke get_log_path', async () => {
      vi.mocked(invoke).mockResolvedValue('/path/to/logs');

      const result = await loggingService.getLogPath();

      expect(invoke).toHaveBeenCalledWith('get_log_path');
      expect(result).toBe('/path/to/logs');
    });
  });

  describe('cleanupLogs', () => {
    it('should invoke cleanup_logs with default maxFiles', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await loggingService.cleanupLogs();

      expect(invoke).toHaveBeenCalledWith('cleanup_logs', { maxFiles: 5 });
    });

    it('should invoke cleanup_logs with custom maxFiles', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await loggingService.cleanupLogs(10);

      expect(invoke).toHaveBeenCalledWith('cleanup_logs', { maxFiles: 10 });
    });
  });

  describe('downloadLogs', () => {
    it('should create and click a download link', async () => {
      vi.mocked(invoke).mockResolvedValue('log content here');

      // Mock URL and DOM APIs
      const mockUrl = 'blob:http://localhost/mock-url';
      const createObjectURL = vi.fn().mockReturnValue(mockUrl);
      const revokeObjectURL = vi.fn();
      global.URL.createObjectURL = createObjectURL;
      global.URL.revokeObjectURL = revokeObjectURL;

      const mockLink = {
        href: '',
        download: '',
        click: vi.fn(),
      };
      const createElementSpy = vi
        .spyOn(document, 'createElement')
        .mockReturnValue(mockLink as unknown as HTMLAnchorElement);
      const appendChildSpy = vi
        .spyOn(document.body, 'appendChild')
        .mockReturnValue(mockLink as unknown as HTMLAnchorElement);
      const removeChildSpy = vi
        .spyOn(document.body, 'removeChild')
        .mockReturnValue(mockLink as unknown as HTMLAnchorElement);

      await loggingService.downloadLogs();

      expect(invoke).toHaveBeenCalledWith('export_logs');
      expect(createElementSpy).toHaveBeenCalledWith('a');
      expect(mockLink.href).toBe(mockUrl);
      expect(mockLink.download).toMatch(/^harbor-logs-\d{4}-\d{2}-\d{2}\.txt$/);
      expect(mockLink.click).toHaveBeenCalled();
      expect(appendChildSpy).toHaveBeenCalled();
      expect(removeChildSpy).toHaveBeenCalled();
      expect(revokeObjectURL).toHaveBeenCalledWith(mockUrl);

      createElementSpy.mockRestore();
      appendChildSpy.mockRestore();
      removeChildSpy.mockRestore();
    });
  });
});
