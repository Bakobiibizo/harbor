import { invoke } from "@tauri-apps/api/core";
import type { Contact, ContactData } from "../types";

/** Contacts service - wraps Tauri commands */
export const contactsService = {
  /** Get all contacts */
  async getContacts(): Promise<Contact[]> {
    return invoke<Contact[]>("get_contacts");
  },

  /** Get active (non-blocked) contacts */
  async getActiveContacts(): Promise<Contact[]> {
    return invoke<Contact[]>("get_active_contacts");
  },

  /** Get a specific contact by peer ID */
  async getContact(peerId: string): Promise<Contact | null> {
    return invoke<Contact | null>("get_contact", { peerId });
  },

  /** Add a new contact */
  async addContact(contact: ContactData): Promise<number> {
    // Convert base64 strings to byte arrays for the backend
    const publicKey = Array.from(atob(contact.publicKey), (c) =>
      c.charCodeAt(0)
    );
    const x25519Public = Array.from(atob(contact.x25519Public), (c) =>
      c.charCodeAt(0)
    );
    return invoke<number>("add_contact", {
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
    return invoke<boolean>("block_contact", { peerId });
  },

  /** Unblock a contact */
  async unblockContact(peerId: string): Promise<boolean> {
    return invoke<boolean>("unblock_contact", { peerId });
  },

  /** Remove a contact */
  async removeContact(peerId: string): Promise<boolean> {
    return invoke<boolean>("remove_contact", { peerId });
  },

  /** Check if a peer is a contact */
  async isContact(peerId: string): Promise<boolean> {
    return invoke<boolean>("is_contact", { peerId });
  },

  /** Check if a peer is blocked */
  async isBlocked(peerId: string): Promise<boolean> {
    return invoke<boolean>("is_contact_blocked", { peerId });
  },
};
