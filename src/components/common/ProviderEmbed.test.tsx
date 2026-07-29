import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useSettingsStore } from '../../stores';
import { clearProviderSessionConsent, parseProviderEmbed } from '../../utils/providerEmbeds';
import { ProviderEmbed } from './ProviderEmbed';

const youtube = parseProviderEmbed('https://www.youtube.com/watch?v=dQw4w9WgXcQ')!;
const providers = [
  youtube,
  parseProviderEmbed('https://soundcloud.com/artist/track')!,
  parseProviderEmbed('https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC')!,
  parseProviderEmbed('https://www.tiktok.com/@creator/video/7412345678901234567')!,
];

describe('ProviderEmbed', () => {
  afterEach(() => vi.useRealTimers());

  beforeEach(() => {
    clearProviderSessionConsent();
    useSettingsStore.setState({ providerEmbedConsent: 'per-use' });
    Object.defineProperty(navigator, 'onLine', { value: true, configurable: true });
  });

  it('does not create any provider network surface before explicit consent', () => {
    render(<ProviderEmbed embed={youtube} />);

    expect(screen.getByText(/shares your IP address and browser details/i)).toBeInTheDocument();
    expect(screen.queryByTitle('YouTube video player')).not.toBeInTheDocument();
    expect(document.querySelector('script')).not.toBeInTheDocument();
  });

  it.each(providers)(
    'keeps the $providerLabel player inert until its own allowlisted consent action',
    (embed) => {
      render(<ProviderEmbed embed={embed} />);
      expect(screen.queryByTitle(embed.title)).not.toBeInTheDocument();

      fireEvent.click(screen.getByRole('button', { name: `Load ${embed.providerLabel} player` }));
      expect(screen.getByTitle(embed.title)).toHaveAttribute('src', embed.embedUrl);
    },
  );

  it('loads only the fixed privacy-enhanced iframe after a keyboard-accessible button click', () => {
    render(<ProviderEmbed embed={youtube} />);
    const loadButton = screen.getByRole('button', { name: 'Load YouTube player' });
    loadButton.focus();
    expect(loadButton).toHaveFocus();
    fireEvent.click(loadButton);

    const frame = screen.getByTitle('YouTube video player');
    expect(frame).toHaveAttribute(
      'src',
      'https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?rel=0',
    );
    expect(frame).toHaveAttribute('sandbox', 'allow-scripts allow-same-origin allow-presentation');
    expect(frame).toHaveAttribute('referrerpolicy', 'no-referrer');
    expect(frame).toHaveAttribute(
      'allow',
      'autoplay; encrypted-media; fullscreen; picture-in-picture',
    );

    fireEvent.load(frame);
    expect(screen.getByLabelText('YouTube embedded player')).toHaveAttribute('data-state', 'ready');
  });

  it('asks again after remount in per-use mode', () => {
    const first = render(<ProviderEmbed embed={youtube} />);
    fireEvent.click(screen.getByRole('button', { name: 'Load YouTube player' }));
    expect(screen.getByTitle('YouTube video player')).toBeInTheDocument();
    first.unmount();

    render(<ProviderEmbed embed={youtube} />);
    expect(screen.getByRole('button', { name: 'Load YouTube player' })).toBeInTheDocument();
    expect(screen.queryByTitle('YouTube video player')).not.toBeInTheDocument();
  });

  it('remembers an explicitly granted provider only for session mode', () => {
    useSettingsStore.setState({ providerEmbedConsent: 'session' });
    const first = render(<ProviderEmbed embed={youtube} />);
    fireEvent.click(screen.getByRole('button', { name: 'Load YouTube player' }));
    first.unmount();

    render(<ProviderEmbed embed={youtube} />);
    expect(screen.getByTitle('YouTube video player')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Load YouTube player' })).not.toBeInTheDocument();
  });

  it('falls back without creating an iframe while offline', () => {
    Object.defineProperty(navigator, 'onLine', { value: false, configurable: true });
    render(<ProviderEmbed embed={youtube} />);
    fireEvent.click(screen.getByRole('button', { name: 'Load YouTube player' }));

    expect(screen.getByText(/player is unavailable/i)).toBeInTheDocument();
    expect(screen.queryByTitle('YouTube video player')).not.toBeInTheDocument();
    expect(screen.getByText(/safe link card is still available/i)).toBeInTheDocument();
  });

  it('times out gracefully when a provider never loads and can be unloaded', () => {
    vi.useFakeTimers();
    render(<ProviderEmbed embed={youtube} />);
    fireEvent.click(screen.getByRole('button', { name: 'Load YouTube player' }));
    act(() => vi.advanceTimersByTime(12_000));
    expect(screen.getByText(/player is unavailable/i)).toBeInTheDocument();

    Object.defineProperty(navigator, 'onLine', { value: true, configurable: true });
    fireEvent.click(screen.getByRole('button', { name: 'Retry player' }));
    expect(screen.getByTitle('YouTube video player')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Unload this player' }));
    expect(screen.queryByTitle('YouTube video player')).not.toBeInTheDocument();
  });
});
