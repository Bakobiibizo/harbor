import { useEffect, useRef, useState } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { useNavigate } from 'react-router-dom';
import { useIdentityStore, useNotificationsStore } from '../../stores';
import { createAsyncDisposerScope, registerAtomicResources } from '../../utils/asyncDisposer';

export function NotificationCenter() {
  const navigate = useNavigate();
  const allNotifications = useNotificationsStore((state) => state.notifications);
  const mutedPeerIds = useNotificationsStore((state) => state.mutedPeerIds);
  const markRead = useNotificationsStore((state) => state.markRead);
  const markOwnerRead = useNotificationsStore((state) => state.markOwnerRead);
  const setPeerMuted = useNotificationsStore((state) => state.setPeerMuted);
  const identity = useIdentityStore((state) =>
    state.state.status === 'unlocked' ? state.state.identity : null,
  );
  const notifications = allNotifications.filter((item) => item.ownerPeerId === identity?.peerId);
  const [open, setOpen] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  const unread = notifications.filter((item) => !item.read).length;

  useEffect(() => {
    if (!isTauri()) return;
    const reportListenerError = (error: unknown) =>
      console.warn('[Notifications] Action listener unavailable:', error);
    const listenerScope = createAsyncDisposerScope(reportListenerError);
    void registerAtomicResources(
      listenerScope,
      [
        async () => {
          const { onAction } = await import('@tauri-apps/plugin-notification');
          const listener = await onAction((event) => {
            if (listenerScope.disposed) return;
            const id =
              typeof event.extra?.notificationId === 'string' ? event.extra.notificationId : '';
            const route = typeof event.extra?.route === 'string' ? event.extra.route : '/chat';
            if (id) markRead(id);
            navigate(route);
          });
          return () => listener.unregister();
        },
      ],
      reportListenerError,
    );
    return () => listenerScope.dispose();
  }, [markRead, navigate]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    const onPointerDown = (event: PointerEvent) => {
      if (!panelRef.current?.contains(event.target as Node)) setOpen(false);
    };
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('pointerdown', onPointerDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('pointerdown', onPointerDown);
    };
  }, [open]);

  const openNotification = (id: string, route: string) => {
    markRead(id);
    setOpen(false);
    navigate(route);
  };

  return (
    <div className="relative" ref={panelRef}>
      <button
        type="button"
        aria-label={unread ? `Notifications, ${unread} unread` : 'Notifications'}
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="harbor-interactive relative flex h-9 w-9 items-center justify-center rounded-lg"
        style={{ background: 'hsl(var(--harbor-surface-2))' }}
      >
        <svg
          aria-hidden="true"
          className="h-5 w-5"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="1.7"
            d="M15 17h5l-1.4-1.4A2 2 0 0118 14.2V11a6 6 0 10-12 0v3.2c0 .5-.2 1-.6 1.4L4 17h5m6 0a3 3 0 01-6 0"
          />
        </svg>
        {unread > 0 && (
          <span
            className="absolute -right-1 -top-1 min-w-5 rounded-full px-1 text-[11px] font-semibold"
            style={{
              background: 'hsl(var(--harbor-accent))',
              color: 'hsl(var(--harbor-bg-primary))',
            }}
          >
            {unread > 99 ? '99+' : unread}
          </span>
        )}
      </button>
      {open && (
        <div
          role="dialog"
          aria-label="Notifications"
          className="absolute bottom-0 left-12 z-[160] w-80 max-h-[70vh] overflow-hidden rounded-xl border shadow-2xl"
          style={{
            background: 'hsl(var(--harbor-bg-elevated))',
            borderColor: 'hsl(var(--harbor-border-subtle))',
          }}
        >
          <div
            className="flex items-center justify-between border-b p-3"
            style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
          >
            <h2 className="font-semibold">Notifications</h2>
            <button
              type="button"
              onClick={() => identity && markOwnerRead(identity.peerId)}
              className="harbor-interactive text-xs"
              style={{ color: 'hsl(var(--harbor-primary))' }}
            >
              Mark all read
            </button>
          </div>
          <div className="max-h-[60vh] overflow-y-auto">
            {notifications.length === 0 ? (
              <p
                className="p-6 text-center text-sm"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                No notifications yet
              </p>
            ) : (
              notifications.map((item) => {
                const muted = mutedPeerIds.includes(`${item.ownerPeerId}:${item.peerId}`);
                return (
                  <div
                    key={item.id}
                    className="border-b p-3"
                    style={{
                      borderColor: 'hsl(var(--harbor-border-subtle))',
                      background: item.read ? 'transparent' : 'hsl(var(--harbor-primary) / 0.08)',
                    }}
                  >
                    <button
                      type="button"
                      onClick={() => openNotification(item.id, item.route)}
                      className="harbor-interactive w-full text-left"
                    >
                      <p className="text-sm font-medium">{item.title}</p>
                      <p
                        className="mt-1 text-xs"
                        style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                      >
                        {item.body}
                      </p>
                      <time
                        className="mt-1 block text-[11px]"
                        style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                      >
                        {new Date(item.createdAt).toLocaleString()}
                      </time>
                    </button>
                    <button
                      type="button"
                      onClick={() => setPeerMuted(item.ownerPeerId, item.peerId, !muted)}
                      className="harbor-interactive mt-2 text-[11px]"
                      style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                    >
                      {muted ? `Unmute ${item.senderName}` : `Mute ${item.senderName}`}
                    </button>
                  </div>
                );
              })
            )}
          </div>
        </div>
      )}
    </div>
  );
}
