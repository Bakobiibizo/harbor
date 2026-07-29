import { create } from 'zustand';
import { contactsService } from '../services/contacts';
import type { Contact, ContactDecisionResult, ContactRequest } from '../types';
import { getErrorMessage } from '../utils/errors';

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
  respondToRequest: (
    requestId: string,
    decision: 'accepted' | 'declined',
  ) => Promise<ContactDecisionResult>;
  retryRequest: (requestId: string) => Promise<void>;
  isContact: (peerId: string) => boolean;
  getContact: (peerId: string) => Contact | undefined;
  reset: () => void;
}

let lifecycleGeneration = 0;

export const useContactsStore = create<ContactsState>((set, get) => ({
  // Initial state
  contacts: [],
  requests: [],
  isLoading: false,
  error: null,

  // Load all contacts
  loadContacts: async () => {
    const generation = lifecycleGeneration;
    set({ isLoading: true, error: null });
    try {
      const contacts = await contactsService.getActiveContacts();
      if (generation === lifecycleGeneration) set({ contacts, isLoading: false });
    } catch (error) {
      if (generation !== lifecycleGeneration) return;
      console.error('Failed to load contacts:', error);
      set({ error: getErrorMessage(error), isLoading: false });
    }
  },

  // Refresh contacts (alias for loadContacts, used after adding new contact)
  refreshContacts: async () => {
    const generation = lifecycleGeneration;
    try {
      const contacts = await contactsService.getActiveContacts();
      if (generation === lifecycleGeneration) set({ contacts });
    } catch (error) {
      if (generation === lifecycleGeneration) set({ error: getErrorMessage(error) });
    }
  },

  loadRequests: async () => {
    const generation = lifecycleGeneration;
    try {
      const requests = await contactsService.getContactRequests();
      if (generation === lifecycleGeneration) set({ requests });
    } catch (error) {
      if (generation === lifecycleGeneration) set({ error: getErrorMessage(error) });
    }
  },

  sendRequest: async (peerId) => {
    const generation = lifecycleGeneration;
    await contactsService.requestPeerIdentity(peerId);
    if (generation !== lifecycleGeneration) return;
    await get().loadRequests();
  },

  respondToRequest: async (requestId, decision) => {
    const generation = lifecycleGeneration;
    const result = await contactsService.respondContactRequest(requestId, decision);
    if (generation !== lifecycleGeneration) return result;
    await Promise.all([get().loadRequests(), get().refreshContacts()]);
    return result;
  },

  retryRequest: async (requestId) => {
    const generation = lifecycleGeneration;
    await contactsService.retryContactRequest(requestId);
    if (generation !== lifecycleGeneration) return;
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
  reset: () => {
    lifecycleGeneration += 1;
    set({ contacts: [], requests: [], isLoading: false, error: null });
  },
}));
