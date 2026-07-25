import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { identityService } from '../../services';
import { CreateIdentity } from './CreateIdentity';

const h = vi.hoisted(() => ({ complete: vi.fn(), loadAccounts: vi.fn() }));
vi.mock('../../stores', () => ({
  useIdentityStore: Object.assign(() => ({
    completeOnboarding: h.complete,
    error: null,
    clearError: vi.fn(),
  })),
  useAccountsStore: () => ({ loadAccounts: h.loadAccounts }),
}));
vi.mock('../../services', () => ({
  identityService: {
    registerRelayName: vi.fn(),
    verifyNameClaim: vi.fn(),
    setPublishingMode: vi.fn(),
  },
  accountsService: { listAccounts: vi.fn().mockResolvedValue([]) },
}));
vi.mock('../../utils/relayNameInput', () => ({
  configuredRelayNamespace: 'relay.test',
  validateRelayLocalName: () => null,
}));
const identity = {
  peerId: 'peer',
  publicKey: '',
  x25519Public: '',
  displayName: 'alice',
  avatarHash: null,
  bio: null,
  passphraseHint: null,
  createdAt: 1,
  updatedAt: 1,
};
const claim = {
  request: {
    domain: 'd',
    version: 1,
    localName: 'alice',
    relay: 'relay.test',
    peerId: 'peer',
    ed25519PublicKey: [],
    x25519PublicKey: [],
    sequence: 1,
    issuedAt: 1,
    nonce: [],
  },
  userSignature: [],
  status: 'active',
  notBefore: 1,
  notAfter: 9,
  relayKeyId: 'k',
  relaySignature: [],
};
async function fill() {
  fireEvent.change(screen.getByLabelText('Harbor name'), { target: { value: 'alice' } });
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
  await screen.findByLabelText('Password');
  fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'password1' } });
  fireEvent.change(screen.getByLabelText('Confirm Password'), { target: { value: 'password1' } });
}
describe('CreateIdentity relay registration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    h.complete.mockResolvedValue(identity);
    vi.mocked(identityService.verifyNameClaim).mockResolvedValue(true);
    vi.mocked(identityService.setPublishingMode).mockResolvedValue();
  });
  it('registers, verifies and attaches the relay claim', async () => {
    h.complete.mockResolvedValue({ ...identity, relayNameClaim: claim });
    render(<CreateIdentity />);
    await fill();
    fireEvent.click(screen.getByRole('button', { name: 'Create Identity' }));
    await waitFor(() => expect(h.complete).toHaveBeenCalled());
  });

  it('offers encrypted-backup recovery before any account exists', () => {
    render(<CreateIdentity />);

    expect(screen.getByText('Already have an encrypted Harbor backup?')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Recover from Backup' })).toBeInTheDocument();
  });
  it('retries registration without creating a second identity', async () => {
    h.complete.mockRejectedValueOnce(new Error('relay offline')).mockResolvedValueOnce(identity);
    render(<CreateIdentity />);
    await fill();
    fireEvent.click(screen.getByRole('button', { name: 'Create Identity' }));
    expect(await screen.findByText('relay offline')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Retry name registration' }));
    await waitFor(() => expect(h.complete).toHaveBeenCalledTimes(2));
  });

  it('shows accessible password confirmation states and blocks mismatched submission', async () => {
    render(<CreateIdentity />);
    fireEvent.change(screen.getByLabelText('Harbor name'), { target: { value: 'alice' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));

    const password = await screen.findByLabelText('Password');
    const confirmation = screen.getByLabelText('Confirm Password');
    const submit = screen.getByRole('button', { name: 'Create Identity' });

    expect(screen.getByRole('status')).toHaveTextContent('Enter the password again to confirm it');
    expect(submit).toBeDisabled();

    fireEvent.change(password, { target: { value: 'password1' } });
    fireEvent.change(confirmation, { target: { value: 'password2' } });
    expect(screen.getByRole('status')).toHaveTextContent('✕ Passwords do not match');
    expect(confirmation).toHaveAttribute('aria-invalid', 'true');
    expect(submit).toBeDisabled();
    fireEvent.click(submit);
    expect(h.complete).not.toHaveBeenCalled();

    fireEvent.change(confirmation, { target: { value: 'password1' } });
    expect(screen.getByRole('status')).toHaveTextContent('✓ Passwords match');
    expect(confirmation).not.toHaveAttribute('aria-invalid');
    expect(submit).toBeEnabled();
  });

  it('accepts pasted password-manager values', async () => {
    render(<CreateIdentity />);
    fireEvent.change(screen.getByLabelText('Harbor name'), { target: { value: 'alice' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));

    const password = await screen.findByLabelText('Password');
    const confirmation = screen.getByLabelText('Confirm Password');
    fireEvent.paste(password, { clipboardData: { getData: () => 'managed-password' } });
    fireEvent.change(password, { target: { value: 'managed-password' } });
    fireEvent.paste(confirmation, { clipboardData: { getData: () => 'managed-password' } });
    fireEvent.change(confirmation, { target: { value: 'managed-password' } });

    expect(password).toHaveAttribute('autocomplete', 'new-password');
    expect(confirmation).toHaveAttribute('autocomplete', 'new-password');
    expect(screen.getByRole('button', { name: 'Create Identity' })).toBeEnabled();
  });
});
