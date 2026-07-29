import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useSettingsStore } from '../../stores';
import { clearProviderSessionConsent } from '../../utils/providerEmbeds';
import {
  clearLinkPreviewCache,
  clearLinkPreviewCacheForTests,
  LinkPreviewCard,
} from './LinkPreviewCard';
import { fetchLinkPreview } from '../../services/linkPreview';

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));
vi.mock('../../services/linkPreview', () => ({ fetchLinkPreview: vi.fn() }));

const preview = {
  url: 'https://example.com/canonical',
  title: 'Example title',
  description: 'A useful description',
  image_url: 'data:image/png;base64,aGVsbG8=',
  site_name: 'Example',
};

describe('LinkPreviewCard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearLinkPreviewCacheForTests();
    clearProviderSessionConsent();
    useSettingsStore.setState({ providerEmbedConsent: 'per-use' });
  });

  it('shows an explicit loading state and then canonical metadata', async () => {
    let resolve!: (value: typeof preview) => void;
    vi.mocked(fetchLinkPreview).mockReturnValueOnce(new Promise((done) => (resolve = done)));
    render(<LinkPreviewCard url="https://example.com/post#fragment" />);

    expect(screen.getByRole('status', { name: 'Loading link preview' })).toHaveAttribute(
      'data-state',
      'loading',
    );
    expect(fetchLinkPreview).toHaveBeenCalledWith('https://example.com/post');

    resolve(preview);
    expect(await screen.findByText('Example title')).toBeInTheDocument();
    expect(screen.getByText('A useful description')).toBeInTheDocument();
    expect(screen.getByText('https://example.com/canonical')).toBeInTheDocument();
    expect(document.querySelector('img')).toHaveAttribute('src', preview.image_url);
    expect(screen.getByRole('link')).toHaveAttribute('data-state', 'ready');
  });

  it('opens only the canonical URL returned by the trusted backend', async () => {
    vi.mocked(fetchLinkPreview).mockResolvedValueOnce(preview);
    render(<LinkPreviewCard url="https://example.com/tracked?utm_source=test" />);

    fireEvent.click(await screen.findByRole('link'));
    await waitFor(() => expect(openUrl).toHaveBeenCalledWith('https://example.com/canonical'));
  });

  it('opens the safe card with standard keyboard activation', async () => {
    vi.mocked(fetchLinkPreview).mockResolvedValueOnce(preview);
    render(<LinkPreviewCard url="https://example.com/post" />);
    const card = await screen.findByRole('link');
    fireEvent.keyDown(card, { key: ' ' });
    await waitFor(() => expect(openUrl).toHaveBeenCalledWith('https://example.com/canonical'));
  });

  it('shows a distinct fallback when a safe link has no published metadata', async () => {
    vi.mocked(fetchLinkPreview).mockResolvedValueOnce({
      ...preview,
      title: null,
      description: null,
      image_url: null,
    });
    render(<LinkPreviewCard url="https://example.com/file.pdf" />);

    expect(await screen.findByText(/No preview details were published/i)).toBeInTheDocument();
    expect(screen.getByRole('link')).toHaveAttribute('data-state', 'fallback');
  });

  it('shows an explicit error fallback when backend fetching fails', async () => {
    vi.mocked(fetchLinkPreview).mockRejectedValueOnce(new Error('blocked target'));
    render(<LinkPreviewCard url="https://example.com/private" />);

    expect(await screen.findByText(/Preview details are unavailable/i)).toBeInTheDocument();
    expect(screen.getByRole('link')).toHaveAttribute('data-state', 'error');
  });

  it('rejects non-HTTP links before invoking the backend', () => {
    render(<LinkPreviewCard url="javascript:alert(1)" />);

    expect(screen.getByText('This link is invalid.')).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveAttribute('data-state', 'error');
    expect(fetchLinkPreview).not.toHaveBeenCalled();
  });

  it('rejects remote image URLs so the webview never contacts a tracking host', async () => {
    vi.mocked(fetchLinkPreview).mockResolvedValueOnce({
      ...preview,
      image_url: 'https://tracker.example/pixel.png',
    });
    render(<LinkPreviewCard url="https://example.com/post" />);

    expect(await screen.findByText(/rejected unsafe preview metadata/i)).toBeInTheDocument();
    expect(document.querySelector('img')).not.toBeInTheDocument();
  });

  it('deduplicates concurrent requests and reuses the bounded cache', async () => {
    vi.mocked(fetchLinkPreview).mockResolvedValue(preview);
    const first = render(<LinkPreviewCard url="https://example.com/post" />);
    const second = render(<LinkPreviewCard url="https://example.com/post" />);

    await waitFor(() => expect(screen.getAllByText('Example title')).toHaveLength(2));
    expect(fetchLinkPreview).toHaveBeenCalledTimes(1);
    first.unmount();
    second.unmount();

    render(<LinkPreviewCard url="https://example.com/post" />);
    expect(await screen.findByText('Example title')).toBeInTheDocument();
    expect(fetchLinkPreview).toHaveBeenCalledTimes(1);
  });

  it('fetches fresh metadata when the normalized URL prop changes', async () => {
    vi.mocked(fetchLinkPreview)
      .mockResolvedValueOnce(preview)
      .mockResolvedValueOnce({
        ...preview,
        url: 'https://second.example/canonical',
        title: 'Second title',
        site_name: 'Second',
      });
    const view = render(<LinkPreviewCard url="https://example.com/post" />);
    expect(await screen.findByText('Example title')).toBeInTheDocument();

    view.rerender(<LinkPreviewCard url="https://second.example/post" />);
    expect(await screen.findByText('Second title')).toBeInTheDocument();
    expect(fetchLinkPreview).toHaveBeenLastCalledWith('https://second.example/post');
  });

  it('cannot publish or cache metadata resolved after a profile runtime reset', async () => {
    let resolve!: (value: typeof preview) => void;
    vi.mocked(fetchLinkPreview).mockReturnValueOnce(new Promise((done) => (resolve = done)));
    const first = render(<LinkPreviewCard url="https://profile-a.example/post" />);

    clearLinkPreviewCache();
    resolve({ ...preview, title: 'Profile A private preview' });

    expect(
      await screen.findByText(/Preview was cleared when the active account changed/i),
    ).toBeInTheDocument();
    expect(screen.queryByText('Profile A private preview')).not.toBeInTheDocument();
    first.unmount();

    vi.mocked(fetchLinkPreview).mockResolvedValueOnce({ ...preview, title: 'Profile B preview' });
    render(<LinkPreviewCard url="https://profile-a.example/post" />);
    expect(await screen.findByText('Profile B preview')).toBeInTheDocument();
    expect(fetchLinkPreview).toHaveBeenCalledTimes(2);
  });

  it('keeps provider content inert while showing the safe metadata card', async () => {
    vi.mocked(fetchLinkPreview).mockResolvedValueOnce({
      ...preview,
      url: 'https://www.youtube.com/watch?v=dQw4w9WgXcQ',
      title: 'A YouTube video',
      site_name: 'YouTube',
      image_url: null,
    });
    render(<LinkPreviewCard url="https://youtu.be/dQw4w9WgXcQ" />);

    expect(await screen.findByText('A YouTube video')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Load YouTube player' })).toBeInTheDocument();
    expect(screen.queryByTitle('YouTube video player')).not.toBeInTheDocument();
  });
});
