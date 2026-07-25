import type { Contact, ContactData, ContactDecisionResult, ContactRequest } from '../types';
import { invokeCommand } from './command';

/** Contacts service - wraps Tauri commands */
export const contactsService = {
  /** Get all contacts */
  async getContacts(): Promise<Contact[]> {
    return invokeCommand('get_contacts');
  },

  /** Get active (non-blocked) contacts */
  async getActiveContacts(): Promise<Contact[]> {
    return invokeCommand('get_active_contacts');
  },

  /** Get a specific contact by peer ID */
  async getContact(peerId: string): Promise<Contact | null> {
    return invokeCommand('get_contact', { peerId });
  },

  /** Add a new contact */
  async addContact(contact: ContactData): Promise<number> {
    // Convert base64 strings to byte arrays for the backend
    const publicKey = Array.from(atob(contact.publicKey), (c) => c.charCodeAt(0));
    const x25519Public = Array.from(atob(contact.x25519Public), (c) => c.charCodeAt(0));
    return invokeCommand('add_contact', {
      peerId: contact.peerId,
      publicKey,
      x25519Public,
      displayName: contact.displayName,
      avatarHash: contact.avatarHash ?? null,
      bio: contact.bio ?? null,
    });
  },

  /** Block a contact */
  async blockContact(peerId: string): Promise<boolean> {
    return invokeCommand('block_contact', { peerId });
  },

  /** Unblock a contact */
  async unblockContact(peerId: string): Promise<boolean> {
    return invokeCommand('unblock_contact', { peerId });
  },

  /** Remove a contact */
  async removeContact(peerId: string): Promise<boolean> {
    return invokeCommand('remove_contact', { peerId });
  },

  /** Check if a peer is a contact */
  async isContact(peerId: string): Promise<boolean> {
    return invokeCommand('is_contact', { peerId });
  },

  /** Check if a peer is blocked */
  async isBlocked(peerId: string): Promise<boolean> {
    return invokeCommand('is_contact_blocked', { peerId });
  },

  /** Request identity exchange with a peer (adds them as a contact) */
  async requestPeerIdentity(peerId: string): Promise<string> {
    return invokeCommand('request_peer_identity', { peerId });
  },

  async getContactRequests(): Promise<ContactRequest[]> {
    return invokeCommand('get_contact_requests');
  },

  async respondContactRequest(
    requestId: string,
    decision: 'accepted' | 'declined',
  ): Promise<ContactDecisionResult> {
    return invokeCommand('respond_contact_request', { requestId, decision });
  },

  async retryContactRequest(requestId: string): Promise<void> {
    return invokeCommand('retry_contact_request', { requestId });
  },
};
