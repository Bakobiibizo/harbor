import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mentionsService } from '../../services';
import { useContactsStore } from '../../stores';
import { MentionInbox } from './MentionInbox';
vi.mock('../../services', () => ({ mentionsService: { listPending: vi.fn(), review: vi.fn() } }));
const receipt = {
  mentionId: 'm',
  postId: 'p',
  qualifiedName: '@alice@relay.test',
  intent: 'repost-request' as const,
  status: 'pending' as const,
  senderPeerId: 'peer',
  preview: 'please review',
  createdAt: 1,
};
describe('MentionInbox', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useContactsStore.setState({
      contacts: [
        {
          id: 1,
          peerId: 'peer',
          publicKey: 'public-key',
          x25519Public: 'x25519-public',
          displayName: 'Saved alias that must not be trusted',
          verifiedQualifiedName: '@alice@relay.test',
          avatarHash: null,
          bio: null,
          isBlocked: false,
          trustLevel: 1,
          lastSeenAt: null,
          addedAt: 1,
          updatedAt: 1,
        },
      ],
      isLoading: false,
      error: null,
    });
    vi.mocked(mentionsService.listPending).mockResolvedValue([receipt]);
    vi.mocked(mentionsService.review).mockResolvedValue();
  });
  it.each([
    ['Accept notification', 'accept-notification'],
    ['Repost on my wall', 'accept-repost'],
    ['Decline', 'decline'],
    ['Block', 'block'],
  ] as const)('shows qualified requester and handles %s', async (button, decision) => {
    render(<MentionInbox />);
    expect(await screen.findByText('@alice@relay.test')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: button }));
    await waitFor(() => expect(mentionsService.review).toHaveBeenCalledWith('m', decision));
  });
  it('surfaces load and review failures', async () => {
    vi.mocked(mentionsService.listPending).mockRejectedValueOnce(new Error('inbox offline'));
    const { unmount } = render(<MentionInbox />);
    expect(await screen.findByRole('alert')).toHaveTextContent('inbox offline');
    unmount();
    vi.mocked(mentionsService.listPending).mockResolvedValue([receipt]);
    vi.mocked(mentionsService.review).mockRejectedValue(new Error('review failed'));
    render(<MentionInbox />);
    fireEvent.click(await screen.findByRole('button', { name: 'Decline' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('review failed');
  });

  it('does not expose a sender key or saved alias when their name is unverified', async () => {
    useContactsStore.setState({
      contacts: [
        {
          ...useContactsStore.getState().contacts[0],
          verifiedQualifiedName: null,
        },
      ],
    });
    render(<MentionInbox />);

    expect(await screen.findByText('Unverified Harbor user')).toBeInTheDocument();
    expect(screen.queryByText('Saved alias that must not be trusted')).not.toBeInTheDocument();
    expect(document.body).not.toHaveTextContent('peer');
  });
});
