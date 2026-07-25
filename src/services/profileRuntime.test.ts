import { beforeEach, describe, expect, it, vi } from 'vitest';
import { contactsService } from './contacts';
import { resetProfileRuntime } from './profileRuntime';
import { registerProfileRuntimeReset } from './profileRuntimeLifecycle';
import { useBoardsStore } from '../stores/boards';
import { useCallingStore } from '../stores/calling';
import { useContactsStore } from '../stores/contacts';
import { useContactWallStore } from '../stores/contactWall';
import { useIdentityStore } from '../stores/identity';
import { useMediaTransfersStore } from '../stores/mediaTransfers';
import { useNetworkStore } from '../stores/network';
import { useWallStore } from '../stores/wall';
import {
  grantProviderSessionConsent,
  hasProviderSessionConsent,
} from '../utils/providerEmbeds';

vi.mock('./contacts', () => ({
  contactsService: {
    getActiveContacts: vi.fn(),
    getContactRequests: vi.fn(),
  },
}));

const identity = {
  peerId: 'peer-profile-a',
  publicKey: 'public-a',
  x25519Public: 'exchange-a',
  displayName: 'Profile A',
  avatarHash: null,
  bio: null,
  passphraseHint: null,
  createdAt: 1,
  updatedAt: 1,
};

describe('resetProfileRuntime', () => {
  beforeEach(() => {
    resetProfileRuntime();
    vi.clearAllMocks();
  });

  it('removes profile-bound process state while retaining only the locked public identity summary', () => {
    useIdentityStore.setState({ state: { status: 'unlocked', identity }, error: 'private failure' });
    useNetworkStore.setState({
      isRunning: true,
      status: 'connected',
      connectedPeers: [{ peerId: 'peer-a' } as never],
      pendingDeepLinkContact: 'harbor://profile-a',
    });
    useContactsStore.setState({ contacts: [{ peerId: 'contact-a' } as never] });
    useBoardsStore.setState({
      communities: [{ relayPeerId: 'relay-a' } as never],
      boardPosts: [{ postId: 'board-post-a' } as never],
    });
    useWallStore.setState({ posts: [{ postId: 'wall-a' } as never] });
    useContactWallStore.setState({
      authorPeerId: 'contact-a',
      wallItems: [{ postId: 'contact-wall-a' } as never],
    });
    useMediaTransfersStore.setState({
      transfers: { mediaA: { mediaHash: 'mediaA' } as never },
    });
    useCallingStore.setState({
      activeCalls: [{ callId: 'call-a' } as never],
      lastEventPeerId: 'peer-a',
    });
    grantProviderSessionConsent('youtube');
    const registeredReset = vi.fn();
    const unregister = registerProfileRuntimeReset(registeredReset);

    resetProfileRuntime();

    expect(registeredReset).toHaveBeenCalledOnce();
    expect(useIdentityStore.getState()).toMatchObject({
      state: { status: 'locked', identity },
      error: null,
    });
    expect(useNetworkStore.getState()).toMatchObject({
      isRunning: false,
      status: 'disconnected',
      connectedPeers: [],
      pendingDeepLinkContact: null,
    });
    expect(useContactsStore.getState().contacts).toEqual([]);
    expect(useBoardsStore.getState().communities).toEqual([]);
    expect(useBoardsStore.getState().boardPosts).toEqual([]);
    expect(useWallStore.getState().posts).toEqual([]);
    expect(useContactWallStore.getState().wallItems).toEqual([]);
    expect(useMediaTransfersStore.getState().transfers).toEqual({});
    expect(useCallingStore.getState().activeCalls).toEqual([]);
    expect(useCallingStore.getState().lastEventPeerId).toBeNull();
    expect(hasProviderSessionConsent('youtube')).toBe(false);
    unregister();
  });

  it('prevents a profile A contact read from repopulating state after teardown', async () => {
    let release!: (contacts: never[]) => void;
    vi.mocked(contactsService.getActiveContacts).mockReturnValue(
      new Promise((resolve) => {
        release = resolve;
      }),
    );

    const pending = useContactsStore.getState().loadContacts();
    resetProfileRuntime();
    release([{ peerId: 'late-profile-a-contact' } as never]);
    await pending;

    expect(useContactsStore.getState().contacts).toEqual([]);
    expect(useContactsStore.getState().isLoading).toBe(false);
  });
});
