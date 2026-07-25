import { invokeCommand } from './command';

export async function exportLogs(): Promise<string> {
  return invokeCommand('export_logs');
}

export async function getLogPath(): Promise<string> {
  return invokeCommand('get_log_path');
}

export async function cleanupLogs(maxFiles: number = 5): Promise<void> {
  return invokeCommand('cleanup_logs', { maxFiles });
}

export async function downloadLogs(): Promise<void> {
  const logs = await exportLogs();
  const blob = new Blob([logs], { type: 'text/plain' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = `harbor-logs-${new Date().toISOString().split('T')[0]}.txt`;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}
