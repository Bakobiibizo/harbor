import { render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { mentionsService } from '../../services';
import { MentionResolution } from './MentionResolution';
vi.mock('../../services', () => ({ mentionsService: { resolve: vi.fn() } }));

describe('MentionResolution', () => {
  it.each([
    ['known', 'contact'],
    ['private', 'private introduction'],
    ['unknown', 'unknown'],
    ['blocked', 'blocked'],
  ] as const)('renders %s resolution', async (status, label) => {
    vi.mocked(mentionsService.resolve).mockResolvedValue({
      qualifiedName: '@alice@relay.test',
      status,
    });
    render(<MentionResolution text="hi @alice@relay.test" onResolved={vi.fn()} />);
    expect(await screen.findByText(new RegExp(label))).toBeInTheDocument();
  });
  it('surfaces resolution failures instead of silently trusting unknown', async () => {
    vi.mocked(mentionsService.resolve).mockRejectedValue(new Error('offline'));
    const resolved = vi.fn();
    render(<MentionResolution text="hi @alice@relay.test" onResolved={resolved} />);
    expect(await screen.findByRole('alert')).toHaveTextContent('could not verify');
    await waitFor(() =>
      expect(resolved).toHaveBeenCalledWith([
        { qualifiedName: '@alice@relay.test', status: 'unknown' },
      ]),
    );
  });
});
