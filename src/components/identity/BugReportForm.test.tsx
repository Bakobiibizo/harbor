import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { mentionsService } from '../../services';
import { BugReportForm } from './BugReportForm';
vi.mock('../../services', () => ({ mentionsService: { resolve: vi.fn(), publish: vi.fn() } }));

describe('BugReportForm', () => {
  it('submits a repost request and shows the tracking wall', async () => {
    vi.mocked(mentionsService.resolve).mockResolvedValue({
      qualifiedName: '@bugs@harbor.social',
      status: 'private',
      claimDigest: 'd',
    });
    vi.mocked(mentionsService.publish).mockResolvedValue({
      postId: 'p',
      createdAt: 1,
      trackingWall: '#/contacts/bugs/wall',
    });
    render(<BugReportForm />);
    fireEvent.change(screen.getByLabelText('Bug summary'), { target: { value: 'Crash' } });
    fireEvent.change(screen.getByLabelText('Bug details'), { target: { value: 'It crashed' } });
    fireEvent.click(screen.getByRole('button', { name: 'Submit signed bug report' }));
    await waitFor(() => expect(mentionsService.publish).toHaveBeenCalled());
    expect(screen.getByText('Bug report submitted')).toBeInTheDocument();
    const trackingLink = screen.getByRole('link', {
      name: 'Track this report on @bugs@harbor.social’s wall',
    });
    expect(trackingLink).toHaveAttribute('href', '#/name/%40bugs%40harbor.social/wall');
    expect(trackingLink.getAttribute('href')).not.toContain('peer');
    expect(trackingLink.getAttribute('href')).not.toContain('harbor://');
    expect(vi.mocked(mentionsService.publish).mock.calls[0][0].mentions[0].intent).toBe(
      'repost-request',
    );
  });
});
