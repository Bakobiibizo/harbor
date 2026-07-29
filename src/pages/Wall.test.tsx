import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act, render, screen, fireEvent, waitFor } from '@testing-library/react';
import { profilePostedMilestoneKey, WallPage } from './Wall';
import { useIdentityStore, useSettingsStore, useWallStore } from '../stores';
import { postsService } from '../services/posts';
import { feedService } from '../services/feed';
import { getShareableContactString } from '../services/network';
import { ComposePostModal } from '../components/common/ComposePostModal';

vi.mock('../services/posts', () => ({
  postsService: {
    getMyPosts: vi.fn(),
    getPostMedia: vi.fn(),
    createPost: vi.fn(),
    updatePost: vi.fn(),
    deletePost: vi.fn(),
    addPostMedia: vi.fn(),
  },
}));

vi.mock('../services/media', () => ({
  mediaService: {
    storeMediaBytes: vi.fn(),
    getMediaUrl: vi.fn(),
  },
}));

vi.mock('../services/feed', () => ({
  feedService: {
    syncWallToRelay: vi.fn(() => Promise.resolve()),
    getWallPreview: vi.fn(),
    getWallVisibilityStats: vi.fn(),
    generateRssFeed: vi.fn(),
    getRssFeedUrl: vi.fn(),
  },
}));

vi.mock('../services/network', () => ({
  getShareableContactString: vi.fn(),
}));

vi.mock('../components/common/LinkPreviewCard', () => ({
  LinkPreviewCard: () => null,
}));

const identity = {
  peerId: 'peer-me',
  publicKey: 'pub',
  x25519Public: 'xpub',
  displayName: 'Test User',
  relayNameVerified: true,
  relayNameClaim: {
    request: {
      domain: 'harbor.relay-name',
      version: 1,
      localName: 'tester',
      relay: 'relay.test',
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
    notAfter: 4102444800,
    relayKeyId: 'relay-key',
    relaySignature: [],
  },
  avatarHash: null,
  bio: null,
  passphraseHint: null,
  createdAt: 1700000000,
  updatedAt: 1700000000,
};

const publicPreviewPost = {
  postId: 'public-post',
  authorPeerId: 'peer-me',
  authorDisplayName: 'Test User',
  contentType: 'text',
  contentText: 'Public preview post',
  visibility: 'public',
  lamportClock: 1,
  createdAt: 1700000000,
  updatedAt: 1700000000,
  isLocal: true,
};

const contactsPreviewPost = {
  postId: 'contacts-post',
  authorPeerId: 'peer-me',
  authorDisplayName: 'Test User',
  contentType: 'text',
  contentText: 'Contacts-only preview post',
  visibility: 'contacts',
  lamportClock: 2,
  createdAt: 1700000100,
  updatedAt: 1700000100,
  isLocal: true,
};

describe('WallPage visibility controls', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: vi.fn().mockResolvedValue(undefined) },
      configurable: true,
    });
    vi.mocked(postsService.getMyPosts).mockResolvedValue([]);
    vi.mocked(postsService.getPostMedia).mockResolvedValue([]);
    vi.mocked(postsService.createPost).mockResolvedValue({
      postId: 'new-post',
      createdAt: 1700000100,
    });
    vi.mocked(feedService.getWallPreview).mockImplementation(async (perspective) =>
      perspective === 'guest' ? [publicPreviewPost] : [contactsPreviewPost, publicPreviewPost],
    );
    vi.mocked(feedService.getWallVisibilityStats).mockResolvedValue({
      totalPosts: 2,
      publicPosts: 1,
      contactsOnlyPosts: 1,
      guestVisible: 1,
      contactVisible: 2,
    });
    vi.mocked(feedService.generateRssFeed).mockResolvedValue(
      '<rss><channel><item><title>Public preview post</title></item></channel></rss>',
    );
    vi.mocked(feedService.getRssFeedUrl).mockResolvedValue('harbor://feed/peer-me');
    vi.mocked(getShareableContactString).mockResolvedValue('harbor://contact-invite-public-data');
    useIdentityStore.setState({ state: { status: 'unlocked', identity }, error: null });
    useSettingsStore.setState({ defaultVisibility: 'public' });
    useWallStore.setState({
      posts: [],
      isLoading: false,
      error: null,
      editingPostId: null,
    });
  });

  it('selects the persisted default visibility before publishing', async () => {
    render(<ComposePostModal isOpen onClose={vi.fn()} />);

    const publicButton = screen.getByRole('button', { name: 'Public' });
    expect(publicButton).toHaveAttribute('aria-pressed', 'true');

    fireEvent.change(screen.getByPlaceholderText(/share your thoughts/i), {
      target: { value: 'Hello public wall' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Publish' }));

    await waitFor(() => {
      expect(postsService.createPost).toHaveBeenCalledWith(
        'text',
        'Hello public wall',
        'public',
        undefined,
      );
    });
  });

  it('lets the author override visibility per post', async () => {
    render(<ComposePostModal isOpen onClose={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: 'Public' }));
    fireEvent.change(screen.getByPlaceholderText(/share your thoughts/i), {
      target: { value: 'Private to contacts' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Publish' }));

    await waitFor(() => {
      expect(postsService.createPost).toHaveBeenCalledWith(
        'text',
        'Private to contacts',
        'contacts',
        undefined,
      );
    });
  });

  it('opens with focus in the accessible dialog and closes with Escape', async () => {
    const onClose = vi.fn();
    render(<ComposePostModal isOpen onClose={onClose} />);

    expect(screen.getByRole('dialog', { name: 'Create a post' })).toHaveAttribute(
      'aria-modal',
      'true',
    );
    await waitFor(() => expect(screen.getByPlaceholderText(/share your thoughts/i)).toHaveFocus());

    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('loads backend guest preview and switches to contact preview without showing contacts-only posts to guests', async () => {
    render(<WallPage />);

    await waitFor(() => {
      expect(feedService.getWallPreview).toHaveBeenCalledWith('guest', 20);
    });

    expect(await screen.findByText('Public preview post')).toBeInTheDocument();
    expect(screen.queryByText('Contacts-only preview post')).not.toBeInTheDocument();
    expect(
      screen.getByText(/Guest preview is loaded from the backend as public-only/i),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('tab', { name: /Contact preview/i }));

    await waitFor(() => {
      expect(feedService.getWallPreview).toHaveBeenCalledWith('contact', 20);
    });
    expect(await screen.findByText('Contacts-only preview post')).toBeInTheDocument();
    expect(
      screen.getByText(/Approved contacts see public and contacts-only posts/i),
    ).toBeInTheDocument();
  });

  it('copies backend-generated RSS XML that contains only public posts', async () => {
    render(<WallPage />);

    fireEvent.click(screen.getByRole('button', { name: /Copy RSS XML/i }));

    await waitFor(() => {
      expect(feedService.generateRssFeed).toHaveBeenCalledWith({
        base_url: 'harbor://peer/peer-me',
        title: "@tester@relay.test's Public Harbor Posts",
        description:
          'Locally generated RSS XML containing only posts marked Public on this Harbor profile.',
        max_items: 50,
      });
    });

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
      expect.stringContaining('Public preview post'),
    );
    expect(navigator.clipboard.writeText).not.toHaveBeenCalledWith(
      expect.stringContaining('Contacts-only preview post'),
    );
  });

  it('copies shareable feed and contact links without private backup material', async () => {
    render(<WallPage />);

    fireEvent.click(screen.getByRole('button', { name: /Copy public feed URI/i }));
    await waitFor(() => {
      expect(feedService.getRssFeedUrl).toHaveBeenCalled();
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith('harbor://feed/peer-me');
    });

    fireEvent.click(screen.getByRole('button', { name: /Copy contact invite/i }));
    await waitFor(() => {
      expect(getShareableContactString).toHaveBeenCalled();
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
        'harbor://contact-invite-public-data',
      );
    });

    const copiedValues = vi
      .mocked(navigator.clipboard.writeText)
      .mock.calls.map(([value]) => value);
    expect(copiedValues.join('\n')).not.toMatch(/private|backup|passphrase/i);
  });

  it('opens the post composer from the profile header action', () => {
    const openComposer = vi.fn();
    window.addEventListener('harbor:new-post', openComposer);
    render(<WallPage />);

    fireEvent.click(screen.getByRole('button', { name: 'Add post' }));
    expect(openComposer).toHaveBeenCalledOnce();

    window.removeEventListener('harbor:new-post', openComposer);
  });

  it('shows the empty-profile placeholder only until the account has had its first post', async () => {
    render(<WallPage />);

    expect(await screen.findByTestId('empty-profile-placeholder')).toBeInTheDocument();

    act(() => {
      useWallStore.setState({
        posts: [
          {
            postId: 'first-post',
            content: 'First post',
            contentType: 'post',
            timestamp: new Date(),
            likes: 0,
            comments: 0,
            liked: false,
            authorPeerId: 'peer-me',
            visibility: 'public',
            lamportClock: 1,
            relayStatus: 'local',
          } as never,
        ],
      });
    });

    await waitFor(() => {
      expect(screen.queryByTestId('empty-profile-placeholder')).not.toBeInTheDocument();
      expect(localStorage.getItem(profilePostedMilestoneKey('peer-me'))).toBe('1');
    });

    act(() => useWallStore.setState({ posts: [] }));
    expect(screen.queryByTestId('empty-profile-placeholder')).not.toBeInTheDocument();
  });
});
