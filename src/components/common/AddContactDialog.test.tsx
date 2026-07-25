import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import toast from 'react-hot-toast';
import { describe, expect, it, vi } from 'vitest';
import { addContactFromString } from '../../services/network';
import { identityService } from '../../services/identity';
import { AddContactDialog } from './AddContactDialog';

vi.mock('../../services/network', () => ({
  addContactFromString: vi.fn(),
}));

vi.mock('../../services/identity', () => ({
  identityService: {
    verifyNameClaim: vi.fn().mockResolvedValue(false),
  },
}));

vi.mock('../../utils/contactInvite', () => ({
  parseContactInvite: () => ({
    displayName: 'Alice',
    peerId: 'peer-alice',
    publicKey: 'public',
    x25519Public: 'exchange',
    relayNameClaim: {
      request: {
        localName: 'alice',
        relay: 'harbor.social',
      },
    },
  }),
}));

vi.mock('react-hot-toast', () => ({
  default: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

describe('AddContactDialog', () => {
  it('shows a relay-qualified name only after backend verification', async () => {
    vi.mocked(identityService.verifyNameClaim).mockResolvedValueOnce(true);
    render(
      <AddContactDialog contactString="harbor://contact/v1/verified" onClose={vi.fn()} />,
    );

    expect(await screen.findByText('@alice@harbor.social')).toBeVisible();
    expect(screen.getByText(/verified against your pinned relay key/i)).toBeVisible();
  });

  it('stays open and displays a structured command failure', async () => {
    vi.mocked(addContactFromString).mockRejectedValue({
      code: 'NETWORK_PEER_UNREACHABLE',
      message: 'Alice could not be reached',
    });
    const onClose = vi.fn();

    render(<AddContactDialog contactString="harbor://contact" onClose={onClose} />);
    fireEvent.click(screen.getByRole('button', { name: 'Send Request' }));

    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith('Failed to add contact: Alice could not be reached'),
    );
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Send Request' })).toBeEnabled();
  });

  it('describes and confirms a request without claiming the contact is trusted', async () => {
    vi.mocked(addContactFromString).mockResolvedValue({
      requestId: 'request-1',
      peerId: 'peer-alice',
      status: 'pending',
      delivery: 'offline',
    });
    const onClose = vi.fn();
    render(<AddContactDialog contactString="harbor://contact/v1/test" onClose={onClose} />);

    expect(
      screen.getByText(/keys and sharing access are added only after they accept/i),
    ).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: 'Send Request' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(toast.success).toHaveBeenCalledWith(
      'Contact request sent. Keys and sharing access will be added after acceptance.',
    );
  });
});
