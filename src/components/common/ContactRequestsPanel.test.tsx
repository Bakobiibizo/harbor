import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ContactRequest } from '../../types';
import { ContactRequestsPanel } from './ContactRequestsPanel';

const requests: ContactRequest[] = [
  {
    requestId: 'incoming-1',
    peerId: 'peer-private',
    direction: 'incoming',
    displayName: 'Alice',
    status: 'review',
    error: null,
    createdAt: 1,
    updatedAt: 1,
  },
  {
    requestId: 'outgoing-1',
    peerId: 'peer-other',
    direction: 'outgoing',
    displayName: 'Bob',
    status: 'pending',
    error: null,
    createdAt: 1,
    updatedAt: 1,
  },
];

describe('ContactRequestsPanel', () => {
  it('renders durable direction/status and explicit review actions', async () => {
    const decide = vi.fn().mockResolvedValue(undefined);
    render(<ContactRequestsPanel requests={requests} onDecision={decide} onRetry={vi.fn()} />);
    expect(screen.getByText('Incoming · Needs review')).toBeInTheDocument();
    expect(screen.getByText('Outgoing · Pending')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Accept' }));
    await waitFor(() => expect(decide).toHaveBeenCalledWith('incoming-1', 'accepted'));
    fireEvent.click(screen.getAllByText('Inspect request')[0]);
    expect(screen.getByText(/never accepts contact requests automatically/i)).toBeInTheDocument();
  });

  it.each([
    ['accepted', 'Accepted'],
    ['declined', 'Declined'],
    ['revoked', 'Revoked'],
  ] as const)('renders terminal %s lifecycle state', (status, label) => {
    render(
      <ContactRequestsPanel
        requests={[{ ...requests[1], status }]}
        onDecision={vi.fn()}
        onRetry={vi.fn()}
      />,
    );
    expect(screen.getByText(`Outgoing · ${label}`)).toBeInTheDocument();
  });

  it('offers retry for failed delivery and renders the failure', async () => {
    const retry = vi.fn().mockResolvedValue(undefined);
    render(
      <ContactRequestsPanel
        requests={[{ ...requests[1], status: 'failed', error: 'Peer is offline' }]}
        onDecision={vi.fn()}
        onRetry={retry}
      />,
    );
    expect(screen.getByRole('alert')).toHaveTextContent('Peer is offline');
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(retry).toHaveBeenCalledWith('outgoing-1'));
  });
});
