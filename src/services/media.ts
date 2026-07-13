import { invoke } from '@tauri-apps/api/core';
import type { EnsureMediaTransferInput, MediaTransferState } from '../types';

/** Media storage service - wraps Tauri commands for content-addressed media storage */
export const mediaService = {
  /**
   * Store a media file from a filesystem path and return its SHA256 hash.
   * Useful when you have a path from a file dialog.
   */
  async storeMedia(filePath: string, mimeType: string): Promise<string> {
    return invoke<string>('store_media', { filePath, mimeType });
  },

  /**
   * Store media from raw bytes (as a Uint8Array) and return its SHA256 hash.
   * Useful when you have file data in memory from a drag-and-drop or paste.
   */
  async storeMediaBytes(data: Uint8Array, mimeType: string): Promise<string> {
    return invoke<string>('store_media_bytes', {
      data: Array.from(data),
      mimeType,
    });
  },

  /**
   * Get a URL that can be used in <img> or <video> src attributes to display
   * a stored media file. Returns an asset:// protocol URL.
   */
  async getMediaUrl(hash: string): Promise<string> {
    return invoke<string>('get_media_url', { hash });
  },

  /**
   * Check if a media file exists locally by its SHA256 hash.
   */
  async hasMedia(hash: string): Promise<boolean> {
    return invoke<boolean>('has_media', { hash });
  },

  /**
   * Scan for missing media and send P2P fetch requests to connected authors.
   * Returns the number of fetch requests sent.
   */
  async preloadMissingMedia(): Promise<number> {
    return invoke<number>('preload_missing_media');
  },

  async ensureTransfer(input: EnsureMediaTransferInput): Promise<MediaTransferState> {
    return invoke<MediaTransferState>('ensure_media_transfer', { input });
  },

  async getTransfer(mediaHash: string): Promise<MediaTransferState | null> {
    return invoke<MediaTransferState | null>('get_media_transfer', { mediaHash });
  },

  async retryTransfer(mediaHash: string): Promise<MediaTransferState> {
    return invoke<MediaTransferState>('retry_media_transfer', { mediaHash });
  },
};
