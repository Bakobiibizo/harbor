import { create } from 'zustand';
import { contactsService } from '../services/contacts';
import type { Contact, ContactRequest } from '../types';

interface ContactsState {
  // State
  contacts: Contact[];
  requests: ContactRequest[];
  isLoading: boolean;
  error: string | null;

  // Actions
  loadContacts: () => Promise<void>;
  refreshContacts: () => Promise<void>;
  loadRequests: () => Promise<void>;
  sendRequest: (peerId: string) => Promise<void>;
  respondToRequest: (requestId: string, decision: 'accepted' | 'declined') => Promise<void>;
  retryRequest: (requestId: string) => Promise<void>;
  isContact: (peerId: string) => boolean;
  getContact: (peerId: string) => Contact | undefined;
}

export const useContactsStore = create<ContactsState>((set, get) => ({
  // Initial state
  contacts: [],
  requests: [],
  isLoading: false,
  error: null,

  // Load all contacts
  loadContacts: async () => {
    set({ isLoading: true, error: null });
    try {
      const contacts = await contactsService.getActiveContacts();
      set({ contacts, isLoading: false });
    } catch (error) {
      console.error('Failed to load contacts:', error);
      set({ error: String(error), isLoading: false });
    }
  },

  // Refresh contacts (alias for loadContacts, used after adding new contact)
  refreshContacts: async () => {
    try {
      const contacts = await contactsService.getActiveContacts();
      set({ contacts });
    } catch (error) {
      console.error('Failed to refresh contacts:', error);
    }
  },

  loadRequests: async () => {
    try {
      const requests = await contactsService.getContactRequests();
      set({ requests });
    } catch (error) {
      set({ error: String(error) });
    }
  },

  sendRequest: async (peerId) => {
    await contactsService.requestPeerIdentity(peerId);
    await get().loadRequests();
  },

  respondToRequest: async (requestId, decision) => {
    await contactsService.respondContactRequest(requestId, decision);
    await Promise.all([get().loadRequests(), get().refreshContacts()]);
  },

  retryRequest: async (requestId) => {
    await contactsService.retryContactRequest(requestId);
    await get().loadRequests();
  },

  // Check if a peer is a contact
  isContact: (peerId: string) => {
    return get().contacts.some((c) => c.peerId === peerId);
  },

  // Get a contact by peer ID
  getContact: (peerId: string) => {
    return get().contacts.find((c) => c.peerId === peerId);
  },
}));
