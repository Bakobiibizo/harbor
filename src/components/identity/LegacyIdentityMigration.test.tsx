import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { identityService } from '../../services';
import { LegacyIdentityMigration } from './LegacyIdentityMigration';

const attach = vi.fn();
const complete = vi.fn();
vi.mock('../../stores', () => ({
  useIdentityStore: (selector: (s: unknown) => unknown) =>
    selector({ attachVerifiedRelayName: attach, completeOnboarding: complete }),
}));
vi.mock('../../utils/relayNameInput', async () => {
  const actual = await vi.importActual<typeof import('../../utils/relayNameInput')>(
    '../../utils/relayNameInput',
  );
  return { ...actual, configuredRelayNamespace: 'relay.example' };
});
vi.mock('../../services', () => ({
  identityService: {
    getIdentityEntryState: vi.fn(),
    getLocalNameClaim: vi.fn(),
    getMigrationState: vi.fn(),
    verifyNameClaim: vi.fn(),
    registerRelayName: vi.fn(),
    setMigrationMode: vi.fn(),
  },
}));

const identity = {
  peerId: 'peer-me',
  publicKey: '',
  x25519Public: '',
  displayName: 'Old Name',
  avatarHash: null,
  bio: null,
  passphraseHint: null,
  createdAt: 1,
  updatedAt: 1,
};
const claim = {
  request: {
    domain: 'harbor/name-claim-request/1',
    version: 1,
    localName: 'new-name',
    relay: 'relay.example',
    peerId: 'peer-me',
    ed25519PublicKey: [],
    x25519PublicKey: [],
    sequence: 1,
    issuedAt: 1,
    nonce: [],
  },
  userSignature: [],
  status: 'active',
  notBefore: 1,
  notAfter: 9999999999,
  relayKeyId: 'key',
  relaySignature: [],
};

describe('LegacyIdentityMigration release gates', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(identityService.getIdentityEntryState).mockResolvedValue({
      claim: null,
      mode: 'required',
    });
    vi.mocked(identityService.setMigrationMode).mockResolvedValue();
  });
  it('recovers from a collision and retries without losing the legacy profile', async () => {
    complete
      .mockRejectedValueOnce(new Error('name already taken'))
      .mockResolvedValueOnce({ ...identity, relayNameClaim: claim, relayNameVerified: true });
    render(
      <LegacyIdentityMigration identity={identity}>
        <div>app restored</div>
      </LegacyIdentityMigration>,
    );
    await screen.findByText('Choose your verified Harbor name');
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'new-name' } });
    fireEvent.click(screen.getByRole('button', { name: 'Claim this name' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('name already taken');
    fireEvent.click(screen.getByRole('button', { name: 'Retry name claim' }));
    expect(await screen.findByText('app restored')).toBeInTheDocument();
    expect(attach).toHaveBeenCalledWith(claim);
  });
  it('surfaces offline startup and permits explicit compatibility retry', async () => {
    vi.mocked(identityService.getIdentityEntryState).mockRejectedValue(new Error('relay offline'));
    render(
      <LegacyIdentityMigration identity={identity}>
        <div>compatibility app</div>
      </LegacyIdentityMigration>,
    );
    expect(await screen.findByRole('alert')).toHaveTextContent('relay offline');
    fireEvent.click(screen.getByRole('button', { name: 'Continue in beta compatibility mode' }));
    expect(await screen.findByText('compatibility app')).toBeInTheDocument();
  });
  it('keeps the migration gate when compatibility persistence is cancelled or fails', async () => {
    vi.mocked(identityService.setMigrationMode).mockRejectedValue(new Error('save cancelled'));
    render(
      <LegacyIdentityMigration identity={identity}>
        <div>must not open</div>
      </LegacyIdentityMigration>,
    );
    await screen.findByText('Choose your verified Harbor name');
    fireEvent.click(screen.getByRole('button', { name: 'Continue in beta compatibility mode' }));
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('save cancelled'));
    expect(screen.queryByText('must not open')).not.toBeInTheDocument();
  });
  it('attaches an existing verified claim on startup and persists verified mode', async () => {
    vi.mocked(identityService.getIdentityEntryState).mockResolvedValue({
      claim,
      mode: 'verified',
    });
    render(
      <LegacyIdentityMigration identity={identity}>
        <div>verified app</div>
      </LegacyIdentityMigration>,
    );
    expect(await screen.findByText('verified app')).toBeInTheDocument();
    expect(attach).toHaveBeenCalledWith(claim);
    expect(identityService.setMigrationMode).toHaveBeenCalledWith('verified');
  });
  it('restores explicitly persisted compatibility mode after remount', async () => {
    vi.mocked(identityService.getIdentityEntryState).mockResolvedValue({
      claim: null,
      mode: 'compatibility',
    });
    const first = render(
      <LegacyIdentityMigration identity={identity}>
        <div>restored app</div>
      </LegacyIdentityMigration>,
    );
    expect(await screen.findByText('restored app')).toBeInTheDocument();
    first.unmount();
    render(
      <LegacyIdentityMigration identity={identity}>
        <div>restored again</div>
      </LegacyIdentityMigration>,
    );
    expect(await screen.findByText('restored again')).toBeInTheDocument();
    expect(identityService.getIdentityEntryState).toHaveBeenCalledTimes(2);
  });

  it('enters immediately with a claim restored and verified during unlock', () => {
    render(
      <LegacyIdentityMigration
        identity={{ ...identity, relayNameClaim: claim, relayNameVerified: true }}
      >
        <div>returning user app</div>
      </LegacyIdentityMigration>,
    );

    expect(screen.getByText('returning user app')).toBeInTheDocument();
    expect(screen.queryByText('Choose your verified Harbor name')).not.toBeInTheDocument();
    expect(identityService.getIdentityEntryState).not.toHaveBeenCalled();
  });
});
