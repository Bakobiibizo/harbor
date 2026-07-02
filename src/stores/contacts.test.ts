import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useContactsStore } from './contacts';
import { contactsService } from '../services/contacts';

vi.mock('../services/contacts', () => ({
  contactsService: {
    getActiveContacts: vi.fn(),
  },
}));

import type { Contact } from '../types/contacts';

const mockContacts: Contact[] = [
  {
    id: 1,
    peerId: 'peer-alice',
    publicKey: 'key-alice',
    x25519Public: 'x-alice',
    displayName: 'Alice',
    avatarHash: null,
    bio: 'Developer',
    isBlocked: false,
    trustLevel: 1,
    lastSeenAt: null,
    addedAt: 1700000000,
    updatedAt: 1700000000,
  },
  {
    id: 2,
    peerId: 'peer-bob',
    publicKey: 'key-bob',
    x25519Public: 'x-bob',
    displayName: 'Bob',
    avatarHash: null,
    bio: 'Designer',
    isBlocked: false,
    trustLevel: 1,
    lastSeenAt: null,
    addedAt: 1700000100,
    updatedAt: 1700000100,
  },
];

describe('useContactsStore', () => {
  beforeEach(() => {
    useContactsStore.setState({
      contacts: [],
      isLoading: false,
      error: null,
    });
    vi.clearAllMocks();
  });

  describe('loadContacts', () => {
    it('should load contacts from backend', async () => {
      vi.mocked(contactsService.getActiveContacts).mockResolvedValue(mockContacts);

      await useContactsStore.getState().loadContacts();

      const state = useContactsStore.getState();
      expect(state.contacts).toEqual(mockContacts);
      expect(state.isLoading).toBe(false);
      expect(state.error).toBeNull();
    });

    it('should set isLoading during load', async () => {
      let resolvePromise: (value: Contact[]) => void;
      vi.mocked(contactsService.getActiveContacts).mockReturnValue(
        new Promise((resolve) => {
          resolvePromise = resolve;
        }),
      );

      const promise = useContactsStore.getState().loadContacts();
      expect(useContactsStore.getState().isLoading).toBe(true);

      resolvePromise!(mockContacts);
      await promise;
      expect(useContactsStore.getState().isLoading).toBe(false);
    });

    it('should handle load errors', async () => {
      vi.mocked(contactsService.getActiveContacts).mockRejectedValue(new Error('Contacts error'));

      await useContactsStore.getState().loadContacts();

      expect(useContactsStore.getState().error).toContain('Contacts error');
      expect(useContactsStore.getState().isLoading).toBe(false);
    });
  });

  describe('refreshContacts', () => {
    it('should refresh contacts without changing loading state', async () => {
      vi.mocked(contactsService.getActiveContacts).mockResolvedValue(mockContacts);

      await useContactsStore.getState().refreshContacts();

      expect(useContactsStore.getState().contacts).toEqual(mockContacts);
    });

    it('should handle refresh errors gracefully', async () => {
      // Set initial contacts
      useContactsStore.setState({ contacts: mockContacts });
      vi.mocked(contactsService.getActiveContacts).mockRejectedValue(new Error('Refresh error'));

      await useContactsStore.getState().refreshContacts();

      // Contacts should remain unchanged on error
      expect(useContactsStore.getState().contacts).toEqual(mockContacts);
    });
  });

  describe('isContact', () => {
    it('should return true for existing contacts', () => {
      useContactsStore.setState({ contacts: mockContacts });

      expect(useContactsStore.getState().isContact('peer-alice')).toBe(true);
      expect(useContactsStore.getState().isContact('peer-bob')).toBe(true);
    });

    it('should return false for non-contacts', () => {
      useContactsStore.setState({ contacts: mockContacts });

      expect(useContactsStore.getState().isContact('peer-unknown')).toBe(false);
    });

    it('should return false when no contacts loaded', () => {
      expect(useContactsStore.getState().isContact('peer-alice')).toBe(false);
    });
  });

  describe('getContact', () => {
    it('should return contact by peerId', () => {
      useContactsStore.setState({ contacts: mockContacts });

      const contact = useContactsStore.getState().getContact('peer-alice');
      expect(contact).toBeDefined();
      expect(contact?.displayName).toBe('Alice');
    });

    it('should return undefined for unknown peerId', () => {
      useContactsStore.setState({ contacts: mockContacts });

      expect(useContactsStore.getState().getContact('peer-unknown')).toBeUndefined();
    });
  });
});
