import { create } from "zustand";
import { contactsService } from "../services/contacts";
import type { Contact } from "../types";

interface ContactsState {
  // State
  contacts: Contact[];
  isLoading: boolean;
  error: string | null;

  // Actions
  loadContacts: () => Promise<void>;
  refreshContacts: () => Promise<void>;
  isContact: (peerId: string) => boolean;
  getContact: (peerId: string) => Contact | undefined;
}

export const useContactsStore = create<ContactsState>((set, get) => ({
  // Initial state
  contacts: [],
  isLoading: false,
  error: null,

  // Load all contacts
  loadContacts: async () => {
    set({ isLoading: true, error: null });
    try {
      const contacts = await contactsService.getActiveContacts();
      set({ contacts, isLoading: false });
    } catch (error) {
      console.error("Failed to load contacts:", error);
      set({ error: String(error), isLoading: false });
    }
  },

  // Refresh contacts (alias for loadContacts, used after adding new contact)
  refreshContacts: async () => {
    try {
      const contacts = await contactsService.getActiveContacts();
      set({ contacts });
    } catch (error) {
      console.error("Failed to refresh contacts:", error);
    }
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
