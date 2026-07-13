import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAccountsStore } from '../../stores';
import type { AccountInfo } from '../../types';
import { AccountSelection } from './AccountSelection';

const account = (overrides: Partial<AccountInfo> = {}): AccountInfo => ({
  id: 'account-1',
  displayName: 'Bakobiibizo',
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
    useAccountsStore.setState({
      accounts: [account()],
      activeAccount: null,
      isLoading: false,
      error: null,
      removeAccount: vi.fn(async () => undefined),
    });
  });

  it('identifies a local account by its saved name, bio, and peer fingerprint', () => {
    render(<AccountSelection onSelectAccount={vi.fn()} onCreateAccount={vi.fn()} />);

    expect(screen.getByText('Bakobiibizo')).toBeInTheDocument();
    expect(screen.getByText("Hi, I'm bako")).toBeInTheDocument();
    expect(screen.getByText(/12D3KooWMEo4…nSqy7z · saved on this device/)).toBeInTheDocument();
    expect(screen.queryByText(/Peer 12D3KooW… \(unverified\)/)).not.toBeInTheDocument();
  });

  it('uses a peer-based fallback when the saved name is blank', () => {
    useAccountsStore.setState({ accounts: [account({ displayName: '   ', bio: null })] });

    render(<AccountSelection onSelectAccount={vi.fn()} onCreateAccount={vi.fn()} />);

    expect(screen.getByText('Local profile 12D3KooW…')).toBeInTheDocument();
  });
});
