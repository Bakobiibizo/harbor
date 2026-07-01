import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { WallPage } from './Wall';
import { useIdentityStore, useSettingsStore, useWallStore } from '../stores';
import { postsService } from '../services/posts';

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
  },
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

describe('WallPage visibility controls', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(postsService.getMyPosts).mockResolvedValue([]);
    vi.mocked(postsService.getPostMedia).mockResolvedValue([]);
    vi.mocked(postsService.createPost).mockResolvedValue({
      postId: 'new-post',
      createdAt: 1700000100,
    });
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
});
