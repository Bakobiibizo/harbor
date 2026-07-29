import { useState, useEffect } from 'react';
import { mediaService } from '../../services/media';
import { createLogger } from '../../utils/logger';
import { useMediaTransfersStore } from '../../stores/mediaTransfers';

const log = createLogger('PostMedia');

export interface PostMediaItem {
  type: 'image' | 'video' | 'audio';
  url: string;
  name?: string;
  sourcePeerId?: string;
  mimeType?: string;
  totalBytes?: number;
}

interface PostMediaProps {
  media: PostMediaItem[];
}

/**
 * Checks whether a URL is a blob URL (created via URL.createObjectURL).
 * Blob URLs start with "blob:".
 */
function isBlobUrl(url: string): boolean {
  return url.startsWith('blob:');
}

/**
 * Checks whether a string looks like a content hash (hex-encoded SHA256).
 * A SHA256 hash is 64 hex characters.
 */
function isContentHash(value: string): boolean {
  return /^[a-f0-9]{64}$/i.test(value);
}

/**
 * Individual media item that resolves its display URL.
 * If the URL is a blob URL, it uses it directly.
 * If the URL looks like a content hash, it calls the backend to resolve it.
 * Otherwise it uses the URL as-is.
 */
function MediaItem({ item }: { item: PostMediaItem }) {
  const [resolvedUrl, setResolvedUrl] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const transfer = useMediaTransfersStore((state) => state.transfers[item.url]);
  const ensure = useMediaTransfersStore((state) => state.ensure);
  const retry = useMediaTransfersStore((state) => state.retry);

  useEffect(() => {
    // Blob URLs and regular URLs can be used directly
    if (isBlobUrl(item.url) || !isContentHash(item.url)) {
      setResolvedUrl(item.url);
      return;
    }

    // Content hash: retain verified metadata while bytes are transferred.
    let cancelled = false;
    setIsLoading(true);
    setError(null);
    ensure({
      mediaHash: item.url,
      sourcePeerId: item.sourcePeerId,
      mediaType: item.type,
      mimeType: item.mimeType,
      fileName: item.name,
      totalBytes: item.totalBytes,
    })
      .then((state) => {
        if (state.status === 'queued' && item.sourcePeerId) {
          void retry(item.url).catch(() => {});
          return null;
        }
        if (state.status !== 'ready') return null;
        return mediaService.getMediaUrl(item.url);
      })
      .then((url) => {
        if (!cancelled && url) setResolvedUrl(url);
      })
      .catch((err) => {
        if (!cancelled) {
          log.warn('Failed to prepare attachment lifecycle', err);
          setError('Attachment state is unavailable');
        }
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [
    ensure,
    item.mimeType,
    item.name,
    item.sourcePeerId,
    item.totalBytes,
    item.type,
    item.url,
    retry,
  ]);

  useEffect(() => {
    if (!isContentHash(item.url) || transfer?.status !== 'ready' || resolvedUrl) return;
    let cancelled = false;
    mediaService
      .getMediaUrl(item.url)
      .then((url) => {
        if (!cancelled) setResolvedUrl(url);
      })
      .catch(() => {
        if (!cancelled) setError('Attachment could not be opened');
      });
    return () => {
      cancelled = true;
    };
  }, [item.url, resolvedUrl, transfer?.status]);

  const status = transfer?.status ?? (isLoading ? 'queued' : null);
  const percent =
    transfer?.totalBytes && transfer.totalBytes > 0
      ? Math.min(100, Math.round((transfer.bytesReceived / transfer.totalBytes) * 100))
      : null;
  const statusLabel =
    status === 'queued'
      ? 'Attachment queued'
      : status === 'discovering'
        ? 'Finding attachment source'
        : status === 'transferring'
          ? percent == null
            ? 'Transferring attachment'
            : `Transferring attachment, ${percent}%`
          : status === 'retrying'
            ? 'Retrying attachment transfer'
            : status === 'unavailable'
              ? 'Attachment source unavailable'
              : status === 'failed'
                ? 'Attachment transfer failed'
                : 'Preparing attachment';

  if (!resolvedUrl && !error) {
    return (
      <div
        className="rounded-lg flex items-center justify-center"
        role="status"
        aria-live="polite"
        aria-label={`${item.type} attachment: ${statusLabel}`}
        style={{
          background: 'hsl(var(--harbor-surface-1))',
          width: '100%',
          maxWidth: '24rem',
          height: '12rem',
        }}
      >
        <div className="flex flex-col items-center gap-2">
          <span className="text-2xl" aria-hidden="true">
            {item.type === 'video' ? '▶' : item.type === 'audio' ? '♪' : '▧'}
          </span>
          <span className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
            {statusLabel}
          </span>
          {percent != null && (
            <progress className="w-40" value={percent} max={100} aria-label={`${percent}%`} />
          )}
          {(status === 'failed' || status === 'unavailable') && (
            <button
              type="button"
              onClick={() => retry(item.url).catch(() => setError('Retry could not be started'))}
              className="px-3 py-1.5 rounded-md text-xs font-medium"
              style={{
                color: 'hsl(var(--harbor-text-primary))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
              }}
            >
              Retry
            </button>
          )}
        </div>
      </div>
    );
  }

  if (error || !resolvedUrl) {
    return (
      <div
        className="rounded-lg flex items-center justify-center"
        style={{
          background: 'hsl(var(--harbor-surface-1))',
          width: '100%',
          maxWidth: '24rem',
          height: '12rem',
        }}
      >
        <div className="flex flex-col items-center gap-2">
          <svg
            className="w-8 h-8"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"
            />
          </svg>
          <span className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
            {error || transfer?.errorMessage || 'Attachment unavailable'}
          </span>
        </div>
      </div>
    );
  }

  if (item.type === 'video') {
    return (
      <div
        className="rounded-lg overflow-hidden"
        style={{ background: 'hsl(var(--harbor-surface-1))' }}
      >
        <video src={resolvedUrl} controls className="max-w-full max-h-96" preload="metadata">
          <track kind="captions" />
        </video>
      </div>
    );
  }

  if (item.type === 'audio') {
    return (
      <div
        className="rounded-lg overflow-hidden p-3"
        style={{ background: 'hsl(var(--harbor-surface-1))' }}
      >
        <audio src={resolvedUrl} controls className="w-72 max-w-full" />
      </div>
    );
  }

  return (
    <div
      className="rounded-lg overflow-hidden"
      style={{ background: 'hsl(var(--harbor-surface-1))' }}
    >
      <img
        src={resolvedUrl}
        alt={item.name || 'Image'}
        className="max-w-full max-h-96 object-contain"
        loading="lazy"
      />
    </div>
  );
}

/**
 * Reusable component for rendering post media attachments.
 * Handles content-hash resolution, blob URLs, loading states, and errors.
 */
export function PostMedia({ media }: PostMediaProps) {
  if (!media || media.length === 0) return null;

  return (
    <div className="mt-4 flex flex-wrap gap-3">
      {media.map((item, index) => (
        <MediaItem key={`${item.url}-${index}`} item={item} />
      ))}
    </div>
  );
}
