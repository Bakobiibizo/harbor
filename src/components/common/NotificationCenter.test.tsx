import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { NotificationCenter } from './NotificationCenter';
import { useIdentityStore, useNotificationsStore } from '../../stores';

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => false, invoke: vi.fn() }));

describe('NotificationCenter', () => {
  beforeEach(() => {
    vi.stubGlobal('crypto', { randomUUID: vi.fn(() => 'notice-1') });
    useNotificationsStore.getState().reset();
    useIdentityStore.setState({
      state: { status: 'unlocked', identity: { peerId: 'peer-local' } as never },
    });
  });

  it('exposes unread history, marks items read, and mutes a sender', () => {
    useNotificationsStore.getState().add({
      dedupeKey: 'message:peer-a:1',
      kind: 'message',
      ownerPeerId: 'peer-local',
      peerId: 'peer-a',
      senderName: '@alice@harbor.social',
      title: 'Message from @alice@harbor.social',
      body: 'Open Harbor to read this private message.',
      route: '/chat',
      createdAt: 1_000,
    });

    render(
      <MemoryRouter>
        <NotificationCenter />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications, 1 unread' }));
    expect(screen.getByText('Message from @alice@harbor.social')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Mute @alice@harbor.social' }));
    expect(useNotificationsStore.getState().mutedPeerIds).toEqual(['peer-local:peer-a']);
    fireEvent.click(screen.getByRole('button', { name: 'Mark all read' }));
    expect(useNotificationsStore.getState().notifications[0].read).toBe(true);
  });
});
