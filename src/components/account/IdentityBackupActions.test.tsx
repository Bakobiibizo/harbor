import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';
import toast from 'react-hot-toast';
import { IdentityBackupActions } from './IdentityBackupActions';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('IdentityBackupActions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(relaunch).mockResolvedValue(undefined);
  });

  it('treats canceling the native recovery picker as a no-op', async () => {
    vi.mocked(open).mockResolvedValue(null);

    render(<IdentityBackupActions />);
    fireEvent.click(screen.getByRole('button', { name: 'Recover from Backup' }));

    await waitFor(() => expect(open).toHaveBeenCalledOnce());
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalled();
    expect(toast.success).not.toHaveBeenCalled();
  });

  it('reports a native recovery-picker failure without invoking restore', async () => {
    vi.mocked(open).mockRejectedValue(new Error('picker unavailable'));

    render(<IdentityBackupActions />);
    fireEvent.click(screen.getByRole('button', { name: 'Recover from Backup' }));

    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(
        'Could not open the backup picker: picker unavailable',
      ),
    );
    expect(invoke).not.toHaveBeenCalled();
  });

  it('uses the native save path and does not report export success before Rust completes', async () => {
    const completion = deferred<{
      peerId: string;
      path: string;
      createdAt: number;
      bytesWritten: number;
    }>();
    vi.mocked(save).mockResolvedValue('C:\\Backups\\identity.harbor-backup');
    vi.mocked(invoke).mockReturnValue(completion.promise);

    render(<IdentityBackupActions allowExport identityLabel="Alice Example" />);
    fireEvent.click(screen.getByRole('button', { name: 'Export Backup' }));
    fireEvent.change(screen.getByLabelText('Current password'), {
      target: { value: 'backup password' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Choose Save Location' }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('export_identity_backup', {
        path: 'C:\\Backups\\identity.harbor-backup',
        password: 'backup password',
      }),
    );
    expect(toast.success).not.toHaveBeenCalled();

    completion.resolve({
      peerId: 'peer-1',
      path: 'C:\\Backups\\identity.harbor-backup',
      createdAt: 1,
      bytesWritten: 4096,
    });
    await waitFor(() => expect(toast.success).toHaveBeenCalledOnce());
  });

  it('does not invoke export or report success when the native save picker is canceled', async () => {
    vi.mocked(save).mockResolvedValue(null);

    render(<IdentityBackupActions allowExport />);
    fireEvent.click(screen.getByRole('button', { name: 'Export Backup' }));
    fireEvent.change(screen.getByLabelText('Current password'), {
      target: { value: 'backup password' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Choose Save Location' }));

    await waitFor(() => expect(save).toHaveBeenCalledOnce());
    expect(invoke).not.toHaveBeenCalled();
    expect(toast.success).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  it('retains the recovery form and password when the backend rejects the backup', async () => {
    vi.mocked(open).mockResolvedValue('/backups/identity.harbor-backup');
    vi.mocked(invoke).mockRejectedValue({
      code: 'INVALID_PASSWORD',
      message: 'The backup password is incorrect',
    });

    render(<IdentityBackupActions />);
    fireEvent.click(screen.getByRole('button', { name: 'Recover from Backup' }));
    await screen.findByRole('dialog');
    fireEvent.change(screen.getByLabelText('Backup password'), {
      target: { value: 'wrong password' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Recover Account' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('The backup password is incorrect');
    expect(screen.getByLabelText('Backup password')).toHaveValue('wrong password');
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(toast.success).not.toHaveBeenCalled();
    expect(relaunch).not.toHaveBeenCalled();
  });

  it('relaunches only after a successful restore that requires restart', async () => {
    const completion = deferred<unknown>();
    vi.mocked(open).mockResolvedValue('/backups/identity.harbor-backup');
    vi.mocked(invoke).mockReturnValue(completion.promise);

    render(<IdentityBackupActions />);
    fireEvent.click(screen.getByRole('button', { name: 'Recover from Backup' }));
    await screen.findByRole('dialog');
    fireEvent.change(screen.getByLabelText('Backup password'), {
      target: { value: 'backup password' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Recover Account' }));

    expect(relaunch).not.toHaveBeenCalled();
    completion.resolve({
      account: { id: 'account-1' },
      restartRequired: true,
    });
    await waitFor(() => expect(relaunch).toHaveBeenCalledOnce());
  });
});
