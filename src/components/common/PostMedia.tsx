import { useState, useEffect } from 'react';
import { mediaService } from '../../services/media';
import { createLogger } from '../../utils/logger';

const log = createLogger('PostMedia');

export interface PostMediaItem {
  type: 'image' | 'video';
  url: string;
  name?: string;
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

  useEffect(() => {
    // Blob URLs and regular URLs can be used directly
    if (isBlobUrl(item.url) || !isContentHash(item.url)) {
      setResolvedUrl(item.url);
      return;
    }

    // Content hash: resolve via the media service
    let cancelled = false;
    setIsLoading(true);
    setError(null);

    mediaService
      .getMediaUrl(item.url)
      .then((url) => {
        if (!cancelled) {
          setResolvedUrl(url);
          setIsLoading(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          log.error(`Failed to resolve media hash: ${item.url}`, err);
          setError('Failed to load media');
          setIsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [item.url]);

  if (isLoading) {
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
          <div
            className="w-6 h-6 border-2 border-t-transparent rounded-full animate-spin"
            style={{
              borderColor: 'hsl(var(--harbor-primary))',
              borderTopColor: 'transparent',
            }}
          />
          <span
            className="text-xs"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            Loading media...
          </span>
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
          <span
            className="text-xs"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            {error || 'Media unavailable'}
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
        <video
          src={resolvedUrl}
          controls
          className="max-w-full max-h-96"
          preload="metadata"
        >
          <track kind="captions" />
        </video>
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
