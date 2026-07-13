import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { WindowsTitleBar } from './WindowsTitleBar';

const h = vi.hoisted(() => ({
  close: vi.fn(async () => undefined),
  isMaximized: vi.fn(async () => false),
  minimize: vi.fn(async () => undefined),
  onResized: vi.fn(async () => vi.fn()),
  toggleMaximize: vi.fn(async () => undefined),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => h,
}));

describe('WindowsTitleBar', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    h.isMaximized.mockResolvedValue(false);
    h.onResized.mockResolvedValue(vi.fn());
  });

  it('uses Harbor branding and exposes native window controls', async () => {
    render(<WindowsTitleBar />);

    expect(screen.getByText('Harbor')).toBeInTheDocument();
    expect(screen.getByText('Private beta')).toBeInTheDocument();
    await waitFor(() => expect(h.isMaximized).toHaveBeenCalled());

    fireEvent.click(screen.getByRole('button', { name: 'Minimize Harbor' }));
    fireEvent.click(screen.getByRole('button', { name: 'Maximize Harbor' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close Harbor' }));

    expect(h.minimize).toHaveBeenCalledOnce();
    expect(h.toggleMaximize).toHaveBeenCalledOnce();
    expect(h.close).toHaveBeenCalledOnce();
  });

  it('shows the restore control when the window is maximized', async () => {
    h.isMaximized.mockResolvedValue(true);

    render(<WindowsTitleBar />);

    expect(
      await screen.findByRole('button', { name: 'Restore Harbor window' }),
    ).toBeInTheDocument();
  });
});
