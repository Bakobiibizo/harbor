import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { relaunch } from '@tauri-apps/plugin-process';
import { useAccountsStore } from '../../stores';
import type { AccountInfo } from '../../types';
import { AccountSelection } from './AccountSelection';
import toast from 'react-hot-toast';
import { accountBackupService } from '../../services';

vi.mock('../../services', () => ({
  accountBackupService: {
    deleteAccountProfile: vi.fn(),
  },
  accountsService: {},
}));

vi.mock('../../services/profileSession', () => ({
  suspendProfile: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: vi.fn(),
}));

vi.mock('react-hot-toast', () => ({
  default: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

const account = (overrides: Partial<AccountInfo> = {}): AccountInfo => ({
  id: 'account-1',
  displayName: 'Bakobiibizo',
  verifiedQualifiedName: '@bakobiibizo@harbor.social',
  verifiedNameNotAfter: 4_000_000_000,
  avatarHash: null,
  bio: "Hi, I'm bako",
  peerId: '12D3KooWMEo4jDyz9hGAVEcZhiGkjRH4A73FXRpk8MwJkhnSqy7z',
  createdAt: 1,
  lastAccessedAt: 1,
  dataPath: 'profile-account-1',
  ...overrides,
});

describe('AccountSelection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAccountsStore.setState({
      accounts: [account()],
      activeAccount: null,
      isLoading: false,
      error: null,
      loadAccounts: vi.fn(async () => undefined),
      setActiveAccount: vi.fn(async () => undefined),
      removeAccount: vi.fn(async () => undefined),
    });
    vi.mocked(accountBackupService.deleteAccountProfile).mockReset();
    vi.mocked(accountBackupService.deleteAccountProfile).mockResolvedValue({
      restartRequired: true,
      nextAccountId: null,
    });
    vi.mocked(relaunch).mockReset();
    vi.mocked(relaunch).mockResolvedValue(undefined);
  });

  it('identifies a local account by its verified relay-qualified name without exposing its key', () => {
    render(<AccountSelection onCreateAccount={vi.fn()} />);

    expect(screen.getByText('@bakobiibizo@harbor.social')).toBeInTheDocument();
    expect(screen.getByText("Hi, I'm bako")).toBeInTheDocument();
    expect(screen.getByText('Saved on this device')).toBeInTheDocument();
    expect(screen.queryByText(/12D3KooW/)).not.toBeInTheDocument();
  });

  it('keeps local accounts distinguishable while marking their labels unverified', () => {
    useAccountsStore.setState({
      accounts: [
        account({ displayName: 'Possible scammer alias', verifiedQualifiedName: null, bio: null }),
      ],
    });

    render(<AccountSelection onCreateAccount={vi.fn()} />);

    expect(screen.getByText('Possible scammer alias@unverified')).toBeInTheDocument();
    expect(screen.queryByText('Possible scammer alias', { exact: true })).not.toBeInTheDocument();
  });

  it('shows distinct labels for multiple locked unverified accounts', () => {
    useAccountsStore.setState({
      accounts: [
        account({ id: 'account-1', displayName: 'Personal', verifiedQualifiedName: null }),
        account({ id: 'account-2', displayName: 'Work', verifiedQualifiedName: null }),
      ],
    });

    render(<AccountSelection onCreateAccount={vi.fn()} />);

    expect(screen.getByText('Personal@unverified')).toBeInTheDocument();
    expect(screen.getByText('Work@unverified')).toBeInTheDocument();
  });

  it('downgrades malformed and expired cached relay names', () => {
    useAccountsStore.setState({
      accounts: [
        account({
          id: 'expired',
          displayName: 'Expired name',
          verifiedNameNotAfter: 1,
        }),
        account({
          id: 'malformed',
          displayName: 'Malformed name',
          verifiedQualifiedName: 'looks verified',
        }),
      ],
    });

    render(<AccountSelection onCreateAccount={vi.fn()} />);

    expect(screen.getByText('Expired name@unverified')).toBeInTheDocument();
    expect(screen.getByText('Malformed name@unverified')).toBeInTheDocument();
  });

  it('persists the selected account before relaunching the application', async () => {
    const setActiveAccount = vi.fn(async () => undefined);
    useAccountsStore.setState({ setActiveAccount });

    render(<AccountSelection onCreateAccount={vi.fn()} />);

    fireEvent.click(screen.getByText('@bakobiibizo@harbor.social'));
    fireEvent.click(screen.getByRole('button', { name: 'Login' }));

    await waitFor(() => expect(setActiveAccount).toHaveBeenCalledWith('account-1'));
    expect(relaunch).toHaveBeenCalledOnce();
    expect(setActiveAccount.mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(relaunch).mock.invocationCallOrder[0],
    );
  });

  it('does not relaunch when persisting the selected account fails', async () => {
    useAccountsStore.setState({
      setActiveAccount: vi.fn(async () => {
        throw new Error('Switch failed');
      }),
    });

    render(<AccountSelection onCreateAccount={vi.fn()} />);

    fireEvent.click(screen.getByText('@bakobiibizo@harbor.social'));
    fireEvent.click(screen.getByRole('button', { name: 'Login' }));

    await waitFor(() => expect(screen.getByRole('button', { name: 'Login' })).toBeEnabled());
    expect(relaunch).not.toHaveBeenCalled();
  });

  it('renders a structured switch failure instead of object coercion', async () => {
    useAccountsStore.setState({
      setActiveAccount: vi.fn(async () => {
        throw { code: 'DATABASE_ERROR', message: 'Could not select that account' };
      }),
    });

    render(<AccountSelection onCreateAccount={vi.fn()} />);
    fireEvent.click(screen.getByText('@bakobiibizo@harbor.social'));
    fireEvent.click(screen.getByRole('button', { name: 'Login' }));

    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(
        'Failed to switch account: Could not select that account',
      ),
    );
    expect(relaunch).not.toHaveBeenCalled();
  });

  it('authenticates chooser deletion before reporting success and relaunching', async () => {
    render(<AccountSelection onCreateAccount={vi.fn()} />);

    fireEvent.click(screen.getByText('@bakobiibizo@harbor.social'));
    fireEvent.click(screen.getByTitle('Delete account'));
    fireEvent.change(screen.getByLabelText('Account password'), {
      target: { value: 'local password' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));

    await waitFor(() =>
      expect(accountBackupService.deleteAccountProfile).toHaveBeenCalledWith(
        'account-1',
        'local password',
      ),
    );
    expect(toast.success).toHaveBeenCalledWith('Account data was deleted from this device.');
    expect(relaunch).toHaveBeenCalledOnce();
    expect(
      vi.mocked(accountBackupService.deleteAccountProfile).mock.invocationCallOrder[0],
    ).toBeLessThan(vi.mocked(relaunch).mock.invocationCallOrder[0]);
  });

  it('keeps the chooser deletion dialog open when authentication fails', async () => {
    vi.mocked(accountBackupService.deleteAccountProfile).mockRejectedValue({
      code: 'IDENTITY_INVALID_PASSPHRASE',
      message: 'The password is incorrect',
    });
    render(<AccountSelection onCreateAccount={vi.fn()} />);

    fireEvent.click(screen.getByText('@bakobiibizo@harbor.social'));
    fireEvent.click(screen.getByTitle('Delete account'));
    fireEvent.change(screen.getByLabelText('Account password'), {
      target: { value: 'wrong password' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('The password is incorrect');
    expect(screen.getByLabelText('Account password')).toHaveValue('wrong password');
    expect(toast.success).not.toHaveBeenCalled();
    expect(relaunch).not.toHaveBeenCalled();
  });
});
