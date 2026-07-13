import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getDomainFromUrl } from '../../utils/urlDetection';
import { parseProviderEmbed } from '../../utils/providerEmbeds';
import { ProviderEmbed } from './ProviderEmbed';

interface LinkPreviewData {
  url: string;
  title: string | null;
  description: string | null;
  image_url: string | null;
  site_name: string | null;
}

type PreviewState =
  | { status: 'loading' }
  | { status: 'ready'; preview: LinkPreviewData }
  | { status: 'fallback'; preview: LinkPreviewData }
  | { status: 'error'; message: string };

interface CacheEntry {
  expiresAt: number;
  state: Exclude<PreviewState, { status: 'loading' }>;
}

const CACHE_CAPACITY = 64;
const SUCCESS_TTL_MS = 15 * 60 * 1000;
const ERROR_TTL_MS = 60 * 1000;
const previewCache = new Map<string, CacheEntry>();
const pendingRequests = new Map<string, Promise<Exclude<PreviewState, { status: 'loading' }>>>();

function normalizeHttpUrl(value: string): string | null {
  try {
    const parsed = new URL(value.trim());
    if (!['http:', 'https:'].includes(parsed.protocol) || parsed.username || parsed.password) {
      return null;
    }
    parsed.hash = '';
    return parsed.toString();
  } catch {
    return null;
  }
}

function isSafeBackendPreview(preview: LinkPreviewData): boolean {
  const canonical = normalizeHttpUrl(preview.url);
  const imageIsSafe =
    preview.image_url === null ||
    /^data:image\/(?:png|jpeg|gif|webp);base64,[a-z0-9+/=]+$/i.test(preview.image_url);
  return canonical === preview.url && imageIsSafe;
}

function readCache(key: string): CacheEntry['state'] | null {
  const entry = previewCache.get(key);
  if (!entry) return null;
  if (entry.expiresAt <= Date.now()) {
    previewCache.delete(key);
    return null;
  }
  previewCache.delete(key);
  previewCache.set(key, entry);
  return entry.state;
}

function writeCache(key: string, state: CacheEntry['state']) {
  previewCache.delete(key);
  previewCache.set(key, {
    state,
    expiresAt: Date.now() + (state.status === 'error' ? ERROR_TTL_MS : SUCCESS_TTL_MS),
  });
  while (previewCache.size > CACHE_CAPACITY) {
    const oldest = previewCache.keys().next().value as string | undefined;
    if (!oldest) break;
    previewCache.delete(oldest);
  }
}

async function loadPreview(key: string): Promise<CacheEntry['state']> {
  const cached = readCache(key);
  if (cached) return cached;
  const pending = pendingRequests.get(key);
  if (pending) return pending;

  const request = invoke<LinkPreviewData>('fetch_link_preview', { url: key })
    .then((preview): CacheEntry['state'] => {
      if (!isSafeBackendPreview(preview)) {
        return { status: 'error', message: 'Harbor rejected unsafe preview metadata.' };
      }
      return preview.title || preview.description || preview.image_url
        ? { status: 'ready', preview }
        : { status: 'fallback', preview };
    })
    .catch((): CacheEntry['state'] => ({
      status: 'error',
      message: 'Preview details are unavailable. You can still open this link.',
    }))
    .then((state) => {
      writeCache(key, state);
      return state;
    })
    .finally(() => pendingRequests.delete(key));
  pendingRequests.set(key, request);
  return request;
}

export function clearLinkPreviewCacheForTests() {
  previewCache.clear();
  pendingRequests.clear();
}

interface LinkPreviewCardProps {
  url: string;
}

export function LinkPreviewCard({ url }: LinkPreviewCardProps) {
  const normalizedUrl = normalizeHttpUrl(url);
  const [state, setState] = useState<PreviewState>(
    normalizedUrl ? { status: 'loading' } : { status: 'error', message: 'This link is invalid.' },
  );

  useEffect(() => {
    if (!normalizedUrl) {
      setState({ status: 'error', message: 'This link is invalid.' });
      return;
    }
    const cached = readCache(normalizedUrl);
    if (cached) {
      setState(cached);
      return;
    }
    let cancelled = false;
    setState({ status: 'loading' });
    loadPreview(normalizedUrl).then((nextState) => {
      if (!cancelled) setState(nextState);
    });
    return () => {
      cancelled = true;
    };
  }, [normalizedUrl]);

  const targetUrl =
    state.status === 'ready' || state.status === 'fallback' ? state.preview.url : normalizedUrl;
  const domain = targetUrl ? getDomainFromUrl(targetUrl) : 'Invalid link';

  const handleOpen = async () => {
    if (!targetUrl) return;
    try {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl(targetUrl);
    } catch {
      window.open(targetUrl, '_blank', 'noopener,noreferrer');
    }
  };

  if (state.status === 'loading') {
    return (
      <div
        className="mt-3 rounded-lg overflow-hidden animate-pulse motion-reduce:animate-none"
        style={{
          border: '1px solid hsl(var(--harbor-border-subtle))',
          background: 'hsl(var(--harbor-surface-1))',
        }}
        role="status"
        aria-label="Loading link preview"
        data-state="loading"
      >
        <div className="p-3 space-y-2">
          <div className="h-3 w-20 rounded bg-[hsl(var(--harbor-surface-2))]" />
          <div className="h-4 w-3/4 rounded bg-[hsl(var(--harbor-surface-2))]" />
          <div className="h-3 w-full rounded bg-[hsl(var(--harbor-surface-2))]" />
          <span className="sr-only">Loading link preview</span>
        </div>
      </div>
    );
  }

  const preview = state.status === 'ready' || state.status === 'fallback' ? state.preview : null;
  const isClickable = targetUrl !== null;
  const image = preview?.image_url?.startsWith('data:image/') ? preview.image_url : null;
  const providerEmbed = preview ? parseProviderEmbed(preview.url) : null;

  return (
    <>
      <div
        className={`mt-3 rounded-lg overflow-hidden transition-all duration-200 motion-reduce:transition-none ${
          isClickable
            ? 'harbor-interactive card-interactive cursor-pointer hover:brightness-110'
            : ''
        }`}
        style={{
          border: '1px solid hsl(var(--harbor-border-subtle))',
          background: 'hsl(var(--harbor-surface-1))',
        }}
        onClick={isClickable ? handleOpen : undefined}
        role={isClickable ? 'link' : 'status'}
        tabIndex={isClickable ? 0 : undefined}
        data-state={state.status}
        onKeyDown={(event) => {
          if (isClickable && (event.key === 'Enter' || event.key === ' ')) {
            event.preventDefault();
            handleOpen();
          }
        }}
      >
        <div className="flex">
          <div className="flex-1 min-w-0 p-3 flex flex-col justify-center">
            <p
              className="text-xs font-medium uppercase tracking-wide mb-1 truncate"
              style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
            >
              {preview?.site_name || domain}
            </p>
            {preview?.title && (
              <p
                className="text-sm font-semibold leading-snug mb-1 line-clamp-2"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                {preview.title}
              </p>
            )}
            {preview?.description && (
              <p
                className="text-xs leading-relaxed line-clamp-2"
                style={{ color: 'hsl(var(--harbor-text-secondary))' }}
              >
                {preview.description}
              </p>
            )}
            {state.status === 'fallback' && (
              <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                No preview details were published for this link.
              </p>
            )}
            {state.status === 'error' && (
              <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                {state.message}
              </p>
            )}
            {targetUrl && (
              <p className="text-xs truncate" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                {targetUrl}
              </p>
            )}
          </div>
          {image && (
            <div
              className="w-28 flex-shrink-0 relative"
              style={{ background: 'hsl(var(--harbor-surface-2))' }}
            >
              <img
                src={image}
                alt=""
                className="absolute inset-0 w-full h-full object-cover"
                loading="lazy"
              />
            </div>
          )}
        </div>
      </div>
      {providerEmbed && <ProviderEmbed embed={providerEmbed} />}
    </>
  );
}
