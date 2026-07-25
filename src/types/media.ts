export type MediaTransferStatus =
  'queued' | 'discovering' | 'transferring' | 'ready' | 'unavailable' | 'retrying' | 'failed';

export interface MediaTransferState {
  mediaHash: string;
  sourcePeerId: string | null;
  mediaType: string;
  mimeType: string | null;
  fileName: string | null;
  totalBytes: number | null;
  bytesReceived: number;
  status: MediaTransferStatus;
  attemptCount: number;
  errorCode: string | null;
  errorMessage: string | null;
  updatedAt: number;
}

export interface EnsureMediaTransferInput {
  mediaHash: string;
  sourcePeerId?: string;
  mediaType: 'image' | 'video' | 'audio';
  mimeType?: string;
  fileName?: string;
  totalBytes?: number;
}

export interface MediaCacheSettings {
  enabled: boolean;
  retentionSeconds: number;
  maxBytes: number;
}

export interface MediaCacheDiagnostics {
  settings: MediaCacheSettings;
  entryCount: number;
  cachedCount: number;
  pendingCount: number;
  cachedBytes: number;
  evictedLastRun: number;
}

export interface StoredMediaInfo {
  mediaHash: string;
  mimeType: string;
  fileName: string;
  totalBytes: number;
}

export interface MediaAssetInfo {
  filePath: string;
  mimeType: string;
  totalBytes: number;
}
