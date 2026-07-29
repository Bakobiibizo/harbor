import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import toast from 'react-hot-toast';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mentionsService } from '../../services';
import { mediaService } from '../../services/media';
import { useIdentityStore, useSettingsStore, useWallStore } from '../../stores';
import { ComposePostModal } from './ComposePostModal';

vi.mock('../../services', () => ({
  mentionsService: {
    resolve: vi.fn(),
    publish: vi.fn(),
  },
}));

vi.mock('../../services/media', () => ({
  mediaService: {
    selectAndStore: vi.fn(),
  },
}));

const identity = {
  peerId: 'peer-me',
  displayName: 'Test User',
  relayNameVerified: true,
  relayNameClaim: {
    request: { localName: 'tester', relay: 'relay.test' },
    status: 'active',
  },
};

describe('ComposePostModal mention and attachment actions', () => {
  const createPost = vi.fn();
  const loadPosts = vi.fn();
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(mentionsService.resolve).mockResolvedValue({
      qualifiedName: '@alice@relay.test',
      status: 'known',
      peerId: 'peer-alice',
      claimDigest: 'claim-alice',
    });
    vi.mocked(mentionsService.publish).mockResolvedValue({
      postId: 'mentioned-post',
      createdAt: 1_700_000_000,
    });
    vi.mocked(mediaService.selectAndStore).mockResolvedValue({
      mediaHash: 'a'.repeat(64),
      mimeType: 'image/png',
      fileName: 'harbor.png',
      totalBytes: 5,
      previewUrl: 'asset://localhost/harbor.png',
    });
    loadPosts.mockResolvedValue(undefined);
    useIdentityStore.setState({
      state: { status: 'unlocked', identity: identity as never },
      error: null,
    });
    useSettingsStore.setState({ defaultVisibility: 'public' });
    useWallStore.setState({ createPost, loadPosts });
  });

  afterEach(() => vi.restoreAllMocks());

  it('disables attachment controls and explains the limitation when a mention is present', async () => {
    render(<ComposePostModal isOpen onClose={vi.fn()} />);
    fireEvent.change(screen.getByRole('textbox'), {
      target: { value: 'Hello @alice@relay.test' },
    });

    await screen.findByText('@alice@relay.test · contact');

    const addImage = screen.getByRole('button', { name: 'Add image' });
    expect(addImage).toBeDisabled();
    expect(addImage).toHaveAttribute('aria-describedby', 'mention-media-explanation');
    expect(
      screen.getByText(/Attachments are unavailable while this post contains an @mention/i),
    ).toBeInTheDocument();
    fireEvent.click(addImage);
    expect(mediaService.selectAndStore).not.toHaveBeenCalled();
  });

  it('blocks an existing mixed draft before submit, then publishes through the signed mention path after attachment removal', async () => {
    const onClose = vi.fn();
    render(<ComposePostModal isOpen onClose={onClose} />);
    fireEvent.click(screen.getByRole('button', { name: 'Add image' }));
    expect(await screen.findByAltText('harbor.png')).toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox'), {
      target: { value: 'Hello @alice@relay.test' },
    });

    const conflict = await screen.findByRole('alert');
    expect(conflict).toHaveTextContent(
      /cannot be published with both an attachment and an @mention/i,
    );
    const publish = screen.getByRole('button', { name: 'Publish' });
    expect(publish).toBeDisabled();
    fireEvent.click(publish);
    expect(mentionsService.publish).not.toHaveBeenCalled();
    expect(createPost).not.toHaveBeenCalled();
    expect(toast.error).not.toHaveBeenCalledWith('Mentioned posts cannot include attachments yet');

    await screen.findByText('@alice@relay.test · contact');
    fireEvent.click(screen.getByRole('button', { name: 'Remove harbor.png' }));
    expect(publish).toBeEnabled();
    fireEvent.click(publish);

    await waitFor(() =>
      expect(mentionsService.publish).toHaveBeenCalledWith({
        contentType: 'text',
        contentText: 'Hello @alice@relay.test',
        visibility: 'public',
        mentions: [
          {
            qualifiedName: '@alice@relay.test',
            intent: 'notify',
            authorizedPeerId: 'peer-alice',
            claimDigest: 'claim-alice',
          },
        ],
      }),
    );
    expect(createPost).not.toHaveBeenCalled();
    expect(loadPosts).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('keeps imported attachment intent when publishing fails', async () => {
    const onClose = vi.fn();
    createPost.mockRejectedValueOnce(new Error('offline'));
    render(<ComposePostModal isOpen onClose={onClose} />);

    fireEvent.click(screen.getByRole('button', { name: 'Add image' }));
    expect(await screen.findByAltText('harbor.png')).toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Retry me' } });
    fireEvent.click(screen.getByRole('button', { name: 'Publish' }));

    await waitFor(() => expect(createPost).toHaveBeenCalledTimes(1));
    expect(await screen.findByAltText('harbor.png')).toBeInTheDocument();
    expect(screen.getByRole('textbox')).toHaveValue('Retry me');
    expect(onClose).not.toHaveBeenCalled();
  });
});
