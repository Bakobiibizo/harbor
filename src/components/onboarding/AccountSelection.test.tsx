import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAccountsStore } from '../../stores';
import type { AccountInfo } from '../../types';
import { AccountSelection } from './AccountSelection';

const account = (overrides: Partial<AccountInfo> = {}): AccountInfo => ({
  id: 'account-1',
  displayName: 'Bakobiibizo',
  verifiedQualifiedName: '@bakobiibizo@harbor.social',
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

  it('identifies a local account by its verified relay-qualified name without exposing its key', () => {
    render(<AccountSelection onSelectAccount={vi.fn()} onCreateAccount={vi.fn()} />);

    expect(screen.getByText('@bakobiibizo@harbor.social')).toBeInTheDocument();
    expect(screen.getByText("Hi, I'm bako")).toBeInTheDocument();
    expect(screen.getByText('Saved on this device')).toBeInTheDocument();
    expect(screen.queryByText(/12D3KooW/)).not.toBeInTheDocument();
  });

  it('uses a neutral local fallback instead of an unverified alias', () => {
    useAccountsStore.setState({
      accounts: [
        account({ displayName: 'Possible scammer alias', verifiedQualifiedName: null, bio: null }),
      ],
    });

    render(<AccountSelection onSelectAccount={vi.fn()} onCreateAccount={vi.fn()} />);

    expect(screen.getByText('Local Harbor account')).toBeInTheDocument();
    expect(screen.queryByText('Possible scammer alias')).not.toBeInTheDocument();
  });
});
