import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { identityService } from '../../services';
import { HarborError } from '../../utils/errors';
import {
  IdentityPublishingGate,
  requestIdentityVerification,
} from './IdentityPublishingGate';

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
    getPublishingState: vi.fn(),
    verifyNameClaim: vi.fn(),
    registerRelayName: vi.fn(),
    setPublishingMode: vi.fn(),
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

describe('IdentityPublishingGate release gates', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(identityService.getIdentityEntryState).mockResolvedValue({
      claim: null,
      mode: 'required',
    });
    vi.mocked(identityService.setPublishingMode).mockResolvedValue();
  });
  it('recovers from a collision and retries without losing the local profile', async () => {
    complete
      .mockRejectedValueOnce(new Error('name already taken'))
      .mockResolvedValueOnce({ ...identity, relayNameClaim: claim, relayNameVerified: true });
    render(
      <IdentityPublishingGate identity={identity}>
        <div>app restored</div>
      </IdentityPublishingGate>,
    );
    await screen.findByText('Choose your verified Harbor name');
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'new-name' } });
    fireEvent.click(screen.getByRole('button', { name: 'Claim this name' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('name already taken');
    fireEvent.click(screen.getByRole('button', { name: 'Retry name claim' }));
    expect(await screen.findByText('app restored')).toBeInTheDocument();
    expect(attach).toHaveBeenCalledWith(claim);
  });
  it('shows the relay rejection instead of mislabeling it as an internet failure', async () => {
    complete.mockRejectedValue(
      new HarborError({
        code: 'NETWORK_ERROR',
        message: 'A network error occurred',
        details: 'Network error: NAME_REGISTRATION_REJECTED',
      }),
    );
    render(
      <IdentityPublishingGate identity={identity}>
        <div>app</div>
      </IdentityPublishingGate>,
    );
    await screen.findByText('Choose your verified Harbor name');
    fireEvent.click(screen.getByRole('button', { name: 'Claim this name' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The relay could not grant this name',
    );
    expect(screen.getByRole('alert')).not.toHaveTextContent('A network error occurred');
  });
  it('surfaces offline startup and permits explicit unverified publishing', async () => {
    vi.mocked(identityService.getIdentityEntryState).mockRejectedValue(new Error('relay offline'));
    render(
      <IdentityPublishingGate identity={identity}>
        <div>unverified app</div>
      </IdentityPublishingGate>,
    );
    expect(await screen.findByRole('alert')).toHaveTextContent('relay offline');
    fireEvent.click(screen.getByRole('button', { name: 'Continue with an unverified identity' }));
    expect(await screen.findByText('unverified app')).toBeInTheDocument();
  });
  it('keeps the publishing gate when unverified persistence is cancelled or fails', async () => {
    vi.mocked(identityService.setPublishingMode).mockRejectedValue(new Error('save cancelled'));
    render(
      <IdentityPublishingGate identity={identity}>
        <div>must not open</div>
      </IdentityPublishingGate>,
    );
    await screen.findByText('Choose your verified Harbor name');
    fireEvent.click(screen.getByRole('button', { name: 'Continue with an unverified identity' }));
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('save cancelled'));
    expect(screen.queryByText('must not open')).not.toBeInTheDocument();
  });
  it('attaches an existing verified claim on startup and persists verified mode', async () => {
    vi.mocked(identityService.getIdentityEntryState).mockResolvedValue({
      claim,
      mode: 'verified',
    });
    render(
      <IdentityPublishingGate identity={identity}>
        <div>verified app</div>
      </IdentityPublishingGate>,
    );
    expect(await screen.findByText('verified app')).toBeInTheDocument();
    expect(attach).toHaveBeenCalledWith(claim);
    expect(identityService.setPublishingMode).toHaveBeenCalledWith('verified');
  });
  it('restores explicitly persisted unverified mode after remount', async () => {
    vi.mocked(identityService.getIdentityEntryState).mockResolvedValue({
      claim: null,
      mode: 'unverified',
    });
    const first = render(
      <IdentityPublishingGate identity={identity}>
        <div>restored app</div>
      </IdentityPublishingGate>,
    );
    expect(await screen.findByText('restored app')).toBeInTheDocument();
    first.unmount();
    render(
      <IdentityPublishingGate identity={identity}>
        <div>restored again</div>
      </IdentityPublishingGate>,
    );
    expect(await screen.findByText('restored again')).toBeInTheDocument();
    expect(identityService.getIdentityEntryState).toHaveBeenCalledTimes(2);
  });

  it('allows an explicitly unverified account to reopen name verification', async () => {
    vi.mocked(identityService.getIdentityEntryState).mockResolvedValue({
      claim: null,
      mode: 'unverified',
    });
    render(
      <IdentityPublishingGate identity={identity}>
        <div>unverified account</div>
      </IdentityPublishingGate>,
    );
    expect(await screen.findByText('unverified account')).toBeInTheDocument();

    requestIdentityVerification();

    expect(await screen.findByText('Choose your verified Harbor name')).toBeInTheDocument();
    expect(screen.queryByText('unverified account')).not.toBeInTheDocument();
  });

  it('resets an unverified frontend mode when the active account changes', async () => {
    vi.mocked(identityService.getIdentityEntryState)
      .mockResolvedValueOnce({ claim: null, mode: 'unverified' })
      .mockResolvedValueOnce({ claim: null, mode: 'required' });
    const view = render(
      <IdentityPublishingGate identity={identity}>
        <div>first account app</div>
      </IdentityPublishingGate>,
    );
    expect(await screen.findByText('first account app')).toBeInTheDocument();

    view.rerender(
      <IdentityPublishingGate identity={{ ...identity, peerId: 'peer-two' }}>
        <div>second account app</div>
      </IdentityPublishingGate>,
    );

    expect(await screen.findByText('Choose your verified Harbor name')).toBeInTheDocument();
    expect(screen.queryByText('second account app')).not.toBeInTheDocument();
  });

  it('enters immediately with a claim restored and verified during unlock', () => {
    render(
      <IdentityPublishingGate
        identity={{ ...identity, relayNameClaim: claim, relayNameVerified: true }}
      >
        <div>returning user app</div>
      </IdentityPublishingGate>,
    );

    expect(screen.getByText('returning user app')).toBeInTheDocument();
    expect(screen.queryByText('Choose your verified Harbor name')).not.toBeInTheDocument();
    expect(identityService.getIdentityEntryState).not.toHaveBeenCalled();
  });
});
