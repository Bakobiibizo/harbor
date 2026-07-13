export type EmbedProvider = 'youtube' | 'soundcloud' | 'spotify' | 'tiktok';
export type ProviderEmbedConsent = 'per-use' | 'session';

export interface ProviderEmbedDescriptor {
  provider: EmbedProvider;
  providerLabel: string;
  sourceUrl: string;
  embedUrl: string;
  title: string;
  aspectRatio: string;
  minimumHeight?: number;
  maximumWidth?: number;
}

const sessionConsent = new Set<EmbedProvider>();

export function hasProviderSessionConsent(provider: EmbedProvider): boolean {
  return sessionConsent.has(provider);
}

export function grantProviderSessionConsent(provider: EmbedProvider) {
  sessionConsent.add(provider);
}

export function clearProviderSessionConsent() {
  sessionConsent.clear();
}

function safeProviderUrl(value: string): URL | null {
  try {
    const url = new URL(value);
    if (url.protocol !== 'https:' || url.username || url.password || url.port) return null;
    url.hash = '';
    return url;
  } catch {
    return null;
  }
}

function hasCanonicalPath(url: URL, segments: string[]): boolean {
  const path = `/${segments.join('/')}`;
  return url.pathname === path || url.pathname === `${path}/`;
}

function youtubeDescriptor(url: URL): ProviderEmbedDescriptor | null {
  const host = url.hostname.toLowerCase();
  let id: string | null = null;
  const segments = url.pathname.split('/').filter(Boolean);
  if (host === 'youtu.be' && segments.length === 1 && hasCanonicalPath(url, segments)) {
    id = segments[0];
  } else if (['youtube.com', 'www.youtube.com', 'm.youtube.com'].includes(host)) {
    if (url.pathname === '/watch') id = url.searchParams.get('v');
    if (
      segments.length === 2 &&
      hasCanonicalPath(url, segments) &&
      ['shorts', 'embed'].includes(segments[0])
    )
      id = segments[1];
  }
  if (!id || !/^[A-Za-z0-9_-]{11}$/.test(id)) return null;
  return {
    provider: 'youtube',
    providerLabel: 'YouTube',
    sourceUrl: `https://www.youtube.com/watch?v=${id}`,
    embedUrl: `https://www.youtube-nocookie.com/embed/${id}?rel=0`,
    title: 'YouTube video player',
    aspectRatio: '16 / 9',
  };
}

function soundCloudDescriptor(url: URL): ProviderEmbedDescriptor | null {
  if (!['soundcloud.com', 'www.soundcloud.com'].includes(url.hostname.toLowerCase())) return null;
  const segments = url.pathname.split('/').filter(Boolean);
  if (
    segments.length < 2 ||
    segments.length > 4 ||
    !hasCanonicalPath(url, segments) ||
    segments.some((segment) => !/^[A-Za-z0-9_-]{1,100}$/.test(segment)) ||
    ['connect', 'discover', 'settings', 'you'].includes(segments[0].toLowerCase())
  ) {
    return null;
  }
  const sourceUrl = `https://soundcloud.com/${segments.join('/')}`;
  const params = new URLSearchParams({
    url: sourceUrl,
    auto_play: 'false',
    hide_related: 'true',
    show_comments: 'false',
    show_user: 'true',
    show_reposts: 'false',
  });
  return {
    provider: 'soundcloud',
    providerLabel: 'SoundCloud',
    sourceUrl,
    embedUrl: `https://w.soundcloud.com/player/?${params.toString()}`,
    title: 'SoundCloud audio player',
    aspectRatio: '16 / 5',
    minimumHeight: 166,
  };
}

function spotifyDescriptor(url: URL): ProviderEmbedDescriptor | null {
  if (url.hostname.toLowerCase() !== 'open.spotify.com') return null;
  const segments = url.pathname.split('/').filter(Boolean);
  if (!hasCanonicalPath(url, segments)) return null;
  if (/^intl-[a-z]{2}$/i.test(segments[0] ?? '')) segments.shift();
  if (segments.length !== 2) return null;
  const [kind, id] = segments;
  if (!['album', 'episode', 'playlist', 'show', 'track'].includes(kind)) return null;
  if (!/^[A-Za-z0-9]{22}$/.test(id)) return null;
  return {
    provider: 'spotify',
    providerLabel: 'Spotify',
    sourceUrl: `https://open.spotify.com/${kind}/${id}`,
    embedUrl: `https://open.spotify.com/embed/${kind}/${id}`,
    title: `Spotify ${kind} player`,
    aspectRatio: kind === 'track' || kind === 'episode' ? '16 / 5' : '16 / 9',
    minimumHeight: kind === 'track' || kind === 'episode' ? 152 : 352,
  };
}

function tikTokDescriptor(url: URL): ProviderEmbedDescriptor | null {
  if (!['tiktok.com', 'www.tiktok.com'].includes(url.hostname.toLowerCase())) return null;
  const match = url.pathname.match(/^\/@([A-Za-z0-9._-]{2,32})\/video\/(\d{6,24})\/?$/);
  if (!match) return null;
  const [, username, id] = match;
  return {
    provider: 'tiktok',
    providerLabel: 'TikTok',
    sourceUrl: `https://www.tiktok.com/@${username}/video/${id}`,
    embedUrl: `https://www.tiktok.com/player/v1/${id}?autoplay=0&loop=0`,
    title: 'TikTok video player',
    aspectRatio: '9 / 16',
    minimumHeight: 575,
    maximumWidth: 420,
  };
}

export function parseProviderEmbed(value: string): ProviderEmbedDescriptor | null {
  const url = safeProviderUrl(value);
  if (!url) return null;
  return (
    youtubeDescriptor(url) ??
    soundCloudDescriptor(url) ??
    spotifyDescriptor(url) ??
    tikTokDescriptor(url)
  );
}
