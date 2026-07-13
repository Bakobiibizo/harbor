import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { clearLinkPreviewCacheForTests, LinkPreviewCard } from './LinkPreviewCard';

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

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
  });

  it('shows an explicit loading state and then canonical metadata', async () => {
    let resolve!: (value: typeof preview) => void;
    vi.mocked(invoke).mockReturnValueOnce(new Promise((done) => (resolve = done)));
    render(<LinkPreviewCard url="https://example.com/post#fragment" />);

    expect(screen.getByRole('status', { name: 'Loading link preview' })).toHaveAttribute(
      'data-state',
      'loading',
    );
    expect(invoke).toHaveBeenCalledWith('fetch_link_preview', {
      url: 'https://example.com/post',
    });

    resolve(preview);
    expect(await screen.findByText('Example title')).toBeInTheDocument();
    expect(screen.getByText('A useful description')).toBeInTheDocument();
    expect(screen.getByText('https://example.com/canonical')).toBeInTheDocument();
    expect(document.querySelector('img')).toHaveAttribute('src', preview.image_url);
    expect(screen.getByRole('link')).toHaveAttribute('data-state', 'ready');
  });

  it('opens only the canonical URL returned by the trusted backend', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(preview);
    render(<LinkPreviewCard url="https://example.com/tracked?utm_source=test" />);

    fireEvent.click(await screen.findByRole('link'));
    await waitFor(() => expect(openUrl).toHaveBeenCalledWith('https://example.com/canonical'));
  });

  it('shows a distinct fallback when a safe link has no published metadata', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
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
    vi.mocked(invoke).mockRejectedValueOnce(new Error('blocked target'));
    render(<LinkPreviewCard url="https://example.com/private" />);

    expect(await screen.findByText(/Preview details are unavailable/i)).toBeInTheDocument();
    expect(screen.getByRole('link')).toHaveAttribute('data-state', 'error');
  });

  it('rejects non-HTTP links before invoking the backend', () => {
    render(<LinkPreviewCard url="javascript:alert(1)" />);

    expect(screen.getByText('This link is invalid.')).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveAttribute('data-state', 'error');
    expect(invoke).not.toHaveBeenCalled();
  });

  it('rejects remote image URLs so the webview never contacts a tracking host', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      ...preview,
      image_url: 'https://tracker.example/pixel.png',
    });
    render(<LinkPreviewCard url="https://example.com/post" />);

    expect(await screen.findByText(/rejected unsafe preview metadata/i)).toBeInTheDocument();
    expect(document.querySelector('img')).not.toBeInTheDocument();
  });

  it('deduplicates concurrent requests and reuses the bounded cache', async () => {
    vi.mocked(invoke).mockResolvedValue(preview);
    const first = render(<LinkPreviewCard url="https://example.com/post" />);
    const second = render(<LinkPreviewCard url="https://example.com/post" />);

    await waitFor(() => expect(screen.getAllByText('Example title')).toHaveLength(2));
    expect(invoke).toHaveBeenCalledTimes(1);
    first.unmount();
    second.unmount();

    render(<LinkPreviewCard url="https://example.com/post" />);
    expect(await screen.findByText('Example title')).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('fetches fresh metadata when the normalized URL prop changes', async () => {
    vi.mocked(invoke)
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
    expect(invoke).toHaveBeenLastCalledWith('fetch_link_preview', {
      url: 'https://second.example/post',
    });
  });
});
