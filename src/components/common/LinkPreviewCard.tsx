import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getDomainFromUrl } from '../../utils/urlDetection';

/** Matches the Rust LinkPreview struct returned by fetch_link_preview */
interface LinkPreviewData {
  url: string;
  title: string | null;
  description: string | null;
  image_url: string | null;
  site_name: string | null;
}

/** In-memory cache to avoid re-fetching the same URL */
const previewCache = new Map<string, LinkPreviewData | 'error'>();

interface LinkPreviewCardProps {
  url: string;
}

/**
 * LinkPreviewCard renders an Open Graph preview card for a given URL.
 *
 * - Calls the Rust backend `fetch_link_preview` command to fetch OG metadata
 * - Caches results in memory to avoid duplicate network requests
 * - Shows a loading skeleton while fetching
 * - Falls back to a simple domain link card on error
 * - Clicking the card opens the URL in the system browser
 */
export function LinkPreviewCard({ url }: LinkPreviewCardProps) {
  const [preview, setPreview] = useState<LinkPreviewData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [imageError, setImageError] = useState(false);
  const fetchedRef = useRef(false);

  useEffect(() => {
    // Prevent double-fetching in React strict mode
    if (fetchedRef.current) return;
    fetchedRef.current = true;

    // Check cache first
    const cached = previewCache.get(url);
    if (cached === 'error') {
      setError(true);
      setLoading(false);
      return;
    }
    if (cached) {
      setPreview(cached);
      setLoading(false);
      return;
    }

    let cancelled = false;

    (async () => {
      try {
        const data = await invoke<LinkPreviewData>('fetch_link_preview', { url });
        if (!cancelled) {
          previewCache.set(url, data);
          setPreview(data);
        }
      } catch {
        if (!cancelled) {
          previewCache.set(url, 'error');
          setError(true);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [url]);

  const handleClick = async () => {
    try {
      // Use tauri-plugin-opener to open the URL in the system browser
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl(url);
    } catch {
      // Fallback: try window.open
      window.open(url, '_blank', 'noopener,noreferrer');
    }
  };

  const domain = getDomainFromUrl(url);

  // Loading skeleton
  if (loading) {
    return (
      <div
        className="mt-3 rounded-lg overflow-hidden animate-pulse"
        style={{
          border: '1px solid hsl(var(--harbor-border-subtle))',
          background: 'hsl(var(--harbor-surface-1))',
        }}
      >
        <div className="flex">
          <div className="flex-1 p-3 space-y-2">
            <div
              className="h-3 w-20 rounded"
              style={{ background: 'hsl(var(--harbor-surface-2))' }}
            />
            <div
              className="h-4 w-3/4 rounded"
              style={{ background: 'hsl(var(--harbor-surface-2))' }}
            />
            <div
              className="h-3 w-full rounded"
              style={{ background: 'hsl(var(--harbor-surface-2))' }}
            />
            <div
              className="h-3 w-2/3 rounded"
              style={{ background: 'hsl(var(--harbor-surface-2))' }}
            />
          </div>
          <div
            className="w-28 flex-shrink-0"
            style={{ background: 'hsl(var(--harbor-surface-2))' }}
          />
        </div>
      </div>
    );
  }

  // Error fallback: simple link card showing domain
  if (error || !preview) {
    return (
      <div
        className="mt-3 rounded-lg overflow-hidden cursor-pointer transition-all duration-200 hover:brightness-110"
        style={{
          border: '1px solid hsl(var(--harbor-border-subtle))',
          background: 'hsl(var(--harbor-surface-1))',
        }}
        onClick={handleClick}
        role="link"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Enter') handleClick();
        }}
      >
        <div className="flex items-center gap-3 px-4 py-3">
          {/* Globe icon */}
          <svg
            className="w-5 h-5 flex-shrink-0"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M12 21a9.004 9.004 0 008.716-6.747M12 21a9.004 9.004 0 01-8.716-6.747M12 21c2.485 0 4.5-4.03 4.5-9S14.485 3 12 3m0 18c-2.485 0-4.5-4.03-4.5-9S9.515 3 12 3m0 0a8.997 8.997 0 017.843 4.582M12 3a8.997 8.997 0 00-7.843 4.582m15.686 0A11.953 11.953 0 0112 10.5c-2.998 0-5.74-1.1-7.843-2.918m15.686 0A8.959 8.959 0 0121 12c0 .778-.099 1.533-.284 2.253m0 0A17.919 17.919 0 0112 16.5c-3.162 0-6.133-.815-8.716-2.247m0 0A9.015 9.015 0 013 12c0-1.605.42-3.113 1.157-4.418"
            />
          </svg>
          <div className="min-w-0 flex-1">
            <p
              className="text-sm font-medium truncate"
              style={{ color: 'hsl(var(--harbor-primary))' }}
            >
              {domain}
            </p>
            <p
              className="text-xs truncate"
              style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
            >
              {url}
            </p>
          </div>
          {/* External link icon */}
          <svg
            className="w-4 h-4 flex-shrink-0"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M13.5 6H5.25A2.25 2.25 0 003 8.25v10.5A2.25 2.25 0 005.25 21h10.5A2.25 2.25 0 0018 18.75V10.5m-10.5 6L21 3m0 0h-5.25M21 3v5.25"
            />
          </svg>
        </div>
      </div>
    );
  }

  // Has preview data but no meaningful content -- use the fallback style
  const hasContent = preview.title || preview.description;
  if (!hasContent) {
    return (
      <div
        className="mt-3 rounded-lg overflow-hidden cursor-pointer transition-all duration-200 hover:brightness-110"
        style={{
          border: '1px solid hsl(var(--harbor-border-subtle))',
          background: 'hsl(var(--harbor-surface-1))',
        }}
        onClick={handleClick}
        role="link"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Enter') handleClick();
        }}
      >
        <div className="flex items-center gap-3 px-4 py-3">
          <svg
            className="w-5 h-5 flex-shrink-0"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M12 21a9.004 9.004 0 008.716-6.747M12 21a9.004 9.004 0 01-8.716-6.747M12 21c2.485 0 4.5-4.03 4.5-9S14.485 3 12 3m0 18c-2.485 0-4.5-4.03-4.5-9S9.515 3 12 3m0 0a8.997 8.997 0 017.843 4.582M12 3a8.997 8.997 0 00-7.843 4.582m15.686 0A11.953 11.953 0 0112 10.5c-2.998 0-5.74-1.1-7.843-2.918m15.686 0A8.959 8.959 0 0121 12c0 .778-.099 1.533-.284 2.253m0 0A17.919 17.919 0 0112 16.5c-3.162 0-6.133-.815-8.716-2.247m0 0A9.015 9.015 0 013 12c0-1.605.42-3.113 1.157-4.418"
            />
          </svg>
          <div className="min-w-0 flex-1">
            <p
              className="text-sm font-medium truncate"
              style={{ color: 'hsl(var(--harbor-primary))' }}
            >
              {preview.site_name || domain}
            </p>
            <p
              className="text-xs truncate"
              style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
            >
              {url}
            </p>
          </div>
          <svg
            className="w-4 h-4 flex-shrink-0"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M13.5 6H5.25A2.25 2.25 0 003 8.25v10.5A2.25 2.25 0 005.25 21h10.5A2.25 2.25 0 0018 18.75V10.5m-10.5 6L21 3m0 0h-5.25M21 3v5.25"
            />
          </svg>
        </div>
      </div>
    );
  }

  // Full preview card with OG data
  const showImage = preview.image_url && !imageError;

  return (
    <div
      className="mt-3 rounded-lg overflow-hidden cursor-pointer transition-all duration-200 hover:brightness-110"
      style={{
        border: '1px solid hsl(var(--harbor-border-subtle))',
        background: 'hsl(var(--harbor-surface-1))',
      }}
      onClick={handleClick}
      role="link"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === 'Enter') handleClick();
      }}
    >
      <div className="flex">
        {/* Text content */}
        <div className="flex-1 min-w-0 p-3 flex flex-col justify-center">
          {/* Site name */}
          <p
            className="text-xs font-medium uppercase tracking-wide mb-1 truncate"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            {preview.site_name || domain}
          </p>

          {/* Title */}
          {preview.title && (
            <p
              className="text-sm font-semibold leading-snug mb-1 line-clamp-2"
              style={{ color: 'hsl(var(--harbor-text-primary))' }}
            >
              {preview.title}
            </p>
          )}

          {/* Description */}
          {preview.description && (
            <p
              className="text-xs leading-relaxed line-clamp-2"
              style={{ color: 'hsl(var(--harbor-text-secondary))' }}
            >
              {preview.description}
            </p>
          )}
        </div>

        {/* Thumbnail image */}
        {showImage && (
          <div
            className="w-28 flex-shrink-0 relative"
            style={{ background: 'hsl(var(--harbor-surface-2))' }}
          >
            <img
              src={preview.image_url!}
              alt=""
              className="absolute inset-0 w-full h-full object-cover"
              onError={() => setImageError(true)}
              loading="lazy"
              referrerPolicy="no-referrer"
            />
          </div>
        )}
      </div>
    </div>
  );
}
