import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { identityService } from '../../services';
import { CreateIdentity } from './CreateIdentity';

const h = vi.hoisted(() => ({ createIdentity: vi.fn(), attach: vi.fn(), loadAccounts: vi.fn() }));
const { createIdentity, attach, loadAccounts } = h;
vi.mock('../../stores', () => ({
  useIdentityStore: Object.assign(
    () => ({ createIdentity: h.createIdentity, error: null, clearError: vi.fn() }),
    { getState: () => ({ attachVerifiedRelayName: h.attach }) },
  ),
  useAccountsStore: () => ({ loadAccounts: h.loadAccounts }),
}));
vi.mock('../../services', () => ({
  identityService: {
    registerRelayName: vi.fn(),
    verifyNameClaim: vi.fn(),
    setMigrationMode: vi.fn(),
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
  await screen.findByLabelText('Passphrase');
  fireEvent.change(screen.getByLabelText('Passphrase'), { target: { value: 'password1' } });
  fireEvent.change(screen.getByLabelText('Confirm Passphrase'), { target: { value: 'password1' } });
}
describe('CreateIdentity relay registration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    createIdentity.mockResolvedValue(identity);
    vi.mocked(identityService.verifyNameClaim).mockResolvedValue(true);
    vi.mocked(identityService.setMigrationMode).mockResolvedValue();
  });
  it('registers, verifies and attaches the relay claim', async () => {
    vi.mocked(identityService.registerRelayName).mockResolvedValue(claim);
    render(<CreateIdentity />);
    await fill();
    fireEvent.click(screen.getByRole('button', { name: 'Create Identity' }));
    await waitFor(() => expect(attach).toHaveBeenCalledWith(claim));
    expect(identityService.setMigrationMode).toHaveBeenCalledWith('verified');
  });
  it('retries registration without creating a second identity', async () => {
    vi.mocked(identityService.registerRelayName)
      .mockRejectedValueOnce(new Error('relay offline'))
      .mockResolvedValueOnce(claim);
    render(<CreateIdentity />);
    await fill();
    fireEvent.click(screen.getByRole('button', { name: 'Create Identity' }));
    expect(await screen.findByText('relay offline')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Retry name registration' }));
    await waitFor(() => expect(attach).toHaveBeenCalledWith(claim));
    expect(createIdentity).toHaveBeenCalledTimes(1);
  });
});
