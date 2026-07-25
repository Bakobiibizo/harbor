import type {
  EnsureMediaTransferInput,
  MediaCacheDiagnostics,
  MediaCacheSettings,
  MediaTransferState,
  StoredMediaInfo,
  MediaAssetInfo,
} from '../types';
import { invokeCommand } from './command';
import { convertFileSrc } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

export type SelectableMediaType = 'image' | 'video' | 'audio';
export interface SelectedStoredMedia extends StoredMediaInfo {
  type: SelectableMediaType;
  previewUrl: string;
}

const MEDIA_EXTENSIONS: Record<SelectableMediaType, string[]> = {
  image: ['jpg', 'jpeg', 'png', 'gif', 'webp'],
  video: ['mp4', 'webm', 'mov'],
  audio: ['mp3', 'm4a', 'wav', 'ogg', 'webm'],
};

function typeForPath(path: string, allowed: SelectableMediaType[]): SelectableMediaType {
  const extension = path.split('.').pop()?.toLowerCase() ?? '';
  return allowed.find((type) => MEDIA_EXTENSIONS[type].includes(extension)) ?? allowed[0];
}

/** Media storage service - wraps Tauri commands for content-addressed media storage */
export const mediaService = {
  async selectAndStore(allowed: SelectableMediaType[]): Promise<SelectedStoredMedia | null> {
    const filePath = await open({
      multiple: false,
      directory: false,
      filters: allowed.map((type) => ({ name: type, extensions: MEDIA_EXTENSIONS[type] })),
    });
    if (typeof filePath !== 'string') return null;
    const stored = await this.storeMedia(filePath);
    return {
      ...stored,
      type: typeForPath(filePath, allowed),
      previewUrl: await this.getMediaUrl(stored.mediaHash),
    };
  },
  /**
   * Store a media file from a filesystem path and return its SHA256 hash.
   * Useful when you have a path from a file dialog.
   */
  async storeMedia(filePath: string, mimeType?: string): Promise<StoredMediaInfo> {
    return invokeCommand('store_media', { filePath, mimeType });
  },

  /**
   * Get a URL that can be used in <img> or <video> src attributes to display
   * a stored media file. Returns an asset:// protocol URL.
   */
  async getMediaUrl(hash: string): Promise<string> {
    const asset: MediaAssetInfo = await invokeCommand('get_media_asset', { hash });
    return convertFileSrc(asset.filePath);
  },

  /**
   * Check if a media file exists locally by its SHA256 hash.
   */
  async hasMedia(hash: string): Promise<boolean> {
    return invokeCommand('has_media', { hash });
  },

  /**
   * Scan for missing media and send P2P fetch requests to connected authors.
   * Returns the number of fetch requests sent.
   */
  async preloadMissingMedia(): Promise<number> {
    return invokeCommand('preload_missing_media');
  },

  async ensureTransfer(input: EnsureMediaTransferInput): Promise<MediaTransferState> {
    return invokeCommand('ensure_media_transfer', { input });
  },

  async getTransfer(mediaHash: string): Promise<MediaTransferState | null> {
    return invokeCommand('get_media_transfer', { mediaHash });
  },

  async retryTransfer(mediaHash: string): Promise<MediaTransferState> {
    return invokeCommand('retry_media_transfer', { mediaHash });
  },

  async getCacheDiagnostics(): Promise<MediaCacheDiagnostics> {
    return invokeCommand('get_media_cache_diagnostics');
  },

  async updateCacheSettings(settings: MediaCacheSettings): Promise<MediaCacheDiagnostics> {
    return invokeCommand('update_media_cache_settings', { settings });
  },
};
