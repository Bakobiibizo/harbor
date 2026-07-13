import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mentionsService } from '../../services';
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
});
