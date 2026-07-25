import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { NotificationCenter } from './NotificationCenter';
import { useIdentityStore, useNotificationsStore } from '../../stores';
import { activateProfile, suspendProfile } from '../../services/profileSession';

const tauriMocks = vi.hoisted(() => ({
  isTauri: vi.fn(() => false),
  onAction: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ isTauri: tauriMocks.isTauri, invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-notification', () => ({ onAction: tauriMocks.onAction }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

describe('NotificationCenter', () => {
  beforeEach(() => {
    tauriMocks.isTauri.mockReset().mockReturnValue(false);
    tauriMocks.onAction.mockReset();
    suspendProfile();
    activateProfile('notification-center-test');
    localStorage.clear();
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

  it('closes the notification panel when interacting outside it', () => {
    render(
      <MemoryRouter>
        <NotificationCenter />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    expect(screen.getByRole('dialog', { name: 'Notifications' })).toBeInTheDocument();

    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole('dialog', { name: 'Notifications' })).not.toBeInTheDocument();
  });

  it('unregisters a notification listener that resolves after unmount', async () => {
    const registration = deferred<{ unregister: () => Promise<void> }>();
    const unregister = vi.fn().mockResolvedValue(undefined);
    tauriMocks.isTauri.mockReturnValue(true);
    tauriMocks.onAction.mockReturnValue(registration.promise);

    const view = render(
      <MemoryRouter>
        <NotificationCenter />
      </MemoryRouter>,
    );
    await waitFor(() => expect(tauriMocks.onAction).toHaveBeenCalledTimes(1));
    view.unmount();
    registration.resolve({ unregister });

    await waitFor(() => expect(unregister).toHaveBeenCalledTimes(1));
  });

  it('does not let delayed setup from an old mount dispose the current listener', async () => {
    const oldRegistration = deferred<{ unregister: () => Promise<void> }>();
    const unregisterOld = vi.fn().mockResolvedValue(undefined);
    const unregisterCurrent = vi.fn().mockResolvedValue(undefined);
    tauriMocks.isTauri.mockReturnValue(true);
    tauriMocks.onAction
      .mockReturnValueOnce(oldRegistration.promise)
      .mockResolvedValueOnce({ unregister: unregisterCurrent });

    const oldView = render(
      <MemoryRouter>
        <NotificationCenter />
      </MemoryRouter>,
    );
    await waitFor(() => expect(tauriMocks.onAction).toHaveBeenCalledTimes(1));
    oldView.unmount();

    const currentView = render(
      <MemoryRouter>
        <NotificationCenter />
      </MemoryRouter>,
    );
    await waitFor(() => expect(tauriMocks.onAction).toHaveBeenCalledTimes(2));
    expect(unregisterCurrent).not.toHaveBeenCalled();

    oldRegistration.resolve({ unregister: unregisterOld });
    await waitFor(() => expect(unregisterOld).toHaveBeenCalledTimes(1));
    expect(unregisterCurrent).not.toHaveBeenCalled();

    currentView.unmount();
    await waitFor(() => expect(unregisterCurrent).toHaveBeenCalledTimes(1));
  });
});
