import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { WallPage } from './Wall';
import { useIdentityStore, useSettingsStore, useWallStore } from '../stores';
import { postsService } from '../services/posts';
import { feedService } from '../services/feed';
import { getShareableContactString } from '../services/network';

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
    render(<WallPage />);

    const publicButton = screen.getByText('Public').closest('button')!;
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
    render(<WallPage />);

    fireEvent.click(screen.getByText('Contacts only').closest('button')!);
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
      screen.getByText(/Contacts with WallRead see public and contacts-only posts/i),
    ).toBeInTheDocument();
  });

  it('copies backend-generated RSS XML that contains only public posts', async () => {
    render(<WallPage />);

    fireEvent.click(screen.getByRole('button', { name: /Copy RSS XML/i }));

    await waitFor(() => {
      expect(feedService.generateRssFeed).toHaveBeenCalledWith({
        base_url: 'harbor://peer/peer-me',
        title: "Test User's Public Harbor Wall",
        description:
          'Locally generated RSS XML containing only posts marked Public on this Harbor wall.',
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
});
