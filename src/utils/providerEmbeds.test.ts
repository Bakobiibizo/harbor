import { beforeEach, describe, expect, it } from 'vitest';
import {
  clearProviderSessionConsent,
  grantProviderSessionConsent,
  hasProviderSessionConsent,
  parseProviderEmbed,
} from './providerEmbeds';

describe('provider embed URL parsing', () => {
  it.each([
    [
      'https://youtu.be/dQw4w9WgXcQ?t=2',
      'youtube',
      'https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?rel=0',
    ],
    [
      'https://www.youtube.com/watch?v=dQw4w9WgXcQ&utm_source=test',
      'youtube',
      'https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?rel=0',
    ],
    [
      'https://soundcloud.com/artist/track?si=tracking',
      'soundcloud',
      'https://w.soundcloud.com/player/',
    ],
    [
      'https://open.spotify.com/intl-ca/track/4uLU6hMCjMI75M1A2tKUQC?si=tracking',
      'spotify',
      'https://open.spotify.com/embed/track/4uLU6hMCjMI75M1A2tKUQC',
    ],
    [
      'https://www.tiktok.com/@creator/video/7412345678901234567?lang=en',
      'tiktok',
      'https://www.tiktok.com/player/v1/7412345678901234567?autoplay=0&loop=0',
    ],
  ])('creates a fixed %s descriptor', (source, provider, expectedEmbed) => {
    const descriptor = parseProviderEmbed(source)!;
    expect(descriptor.provider).toBe(provider);
    expect(descriptor.embedUrl.startsWith(expectedEmbed)).toBe(true);
    expect(descriptor.embedUrl).not.toContain('tracking');
  });

  it.each([
    'http://www.youtube.com/watch?v=dQw4w9WgXcQ',
    'https://youtube.com.evil.test/watch?v=dQw4w9WgXcQ',
    'https://youtube.com@evil.test/watch?v=dQw4w9WgXcQ',
    'https://www.youtube.com:8443/watch?v=dQw4w9WgXcQ',
    'https://www.youtube.com/watch?v=%3Cscript%3E',
    'https://www.youtube.com/watch?v=dQw4w9WgXcQ%2F..',
    'https://youtu.be//dQw4w9WgXcQ',
    'https://w.soundcloud.com/player/?url=https://evil.test',
    'https://soundcloud.com/connect/callback',
    'https://soundcloud.com/artist//track',
    'https://open.spotify.com/embed/track/4uLU6hMCjMI75M1A2tKUQC',
    'https://open.spotify.com/intl-attacker/track/4uLU6hMCjMI75M1A2tKUQC',
    'https://open.spotify.com/track/not-a-valid-id',
    'https://open.spotify.com/track//4uLU6hMCjMI75M1A2tKUQC',
    'https://www.tiktok.com/embed/v2/7412345678901234567',
    'https://www.tiktok.com/@creator/video/123<script>',
  ])('rejects noncanonical or attacker-controlled input %s', (source) => {
    expect(parseProviderEmbed(source)).toBeNull();
  });
});

describe('session consent', () => {
  beforeEach(clearProviderSessionConsent);

  it('is absent by default and only exists after an explicit grant', () => {
    expect(hasProviderSessionConsent('youtube')).toBe(false);
    grantProviderSessionConsent('youtube');
    expect(hasProviderSessionConsent('youtube')).toBe(true);
    expect(hasProviderSessionConsent('spotify')).toBe(false);
  });
});
