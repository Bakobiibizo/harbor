import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mentionsService } from '../services';
import { useContactsStore } from '../stores';
import { NamedContactWallPage } from './NamedContactWall';

vi.mock('../services', () => ({ mentionsService: { resolve: vi.fn() } }));
vi.mock('./ContactWall', () => ({
  ContactWallPage: ({
    peerIdOverride,
    verifiedQualifiedNameOverride,
    hidePeerId,
  }: {
    peerIdOverride: string;
    verifiedQualifiedNameOverride: string;
    hidePeerId: boolean;
  }) => (
    <div>
      named wall {verifiedQualifiedNameOverride} {peerIdOverride} {String(hidePeerId)}
    </div>
  ),
}));

function renderRoute(name = '%40bugs%40harbor.social') {
  return render(
    <MemoryRouter initialEntries={[`/name/${name}/wall`]}>
      <Routes>
        <Route path="/name/:qualifiedName/wall" element={<NamedContactWallPage />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe('NamedContactWallPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useContactsStore.setState({ contacts: [], isLoading: false, error: null });
    vi.spyOn(useContactsStore.getState(), 'loadContacts').mockResolvedValue();
  });

  it('resolves the name internally and keeps the raw ID out of the route', async () => {
    vi.mocked(mentionsService.resolve).mockResolvedValue({
      qualifiedName: '@bugs@harbor.social',
      status: 'known',
      peerId: 'peer-private-lookup-value',
    });
    renderRoute();
    await waitFor(() =>
      expect(screen.getByText(/named wall @bugs@harbor.social/)).toBeInTheDocument(),
    );
    expect(window.location.href).not.toContain('peer-private-lookup-value');
  });

  it('shows a safe unavailable state without revealing a lookup key', async () => {
    vi.mocked(mentionsService.resolve).mockResolvedValue({
      qualifiedName: '@bugs@harbor.social',
      status: 'private',
    });
    renderRoute();
    expect(await screen.findByRole('alert')).toHaveTextContent(/could not resolve this account/i);
  });
});
