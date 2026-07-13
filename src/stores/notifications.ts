import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type HarborNotificationKind = 'message' | 'incoming_call' | 'missed_call';

export interface HarborNotification {
  id: string;
  dedupeKey: string;
  kind: HarborNotificationKind;
  ownerPeerId: string;
  peerId: string;
  senderName: string;
  title: string;
  body: string;
  route: string;
  createdAt: number;
  read: boolean;
}

interface NotificationState {
  notifications: HarborNotification[];
  mutedPeerIds: string[];
  nativeEnabled: boolean;
  showMessagePreviews: boolean;
  add: (notification: Omit<HarborNotification, 'id' | 'read'>) => HarborNotification | null;
  markRead: (id: string) => void;
  markAllRead: () => void;
  markOwnerRead: (ownerPeerId: string) => void;
  remove: (id: string) => void;
  clear: () => void;
  clearOwner: (ownerPeerId: string) => void;
  setPeerMuted: (ownerPeerId: string, peerId: string, muted: boolean) => void;
  setNativeEnabled: (enabled: boolean) => void;
  setShowMessagePreviews: (enabled: boolean) => void;
  reset: () => void;
}

const MAX_NOTIFICATIONS = 200;
const DEDUPE_WINDOW_MS = 5_000;

const initialState = {
  notifications: [] as HarborNotification[],
  mutedPeerIds: [] as string[],
  nativeEnabled: false,
  showMessagePreviews: false,
};

export const useNotificationsStore = create<NotificationState>()(
  persist(
    (set, get) => ({
      ...initialState,
      add: (input) => {
        if (get().mutedPeerIds.includes(`${input.ownerPeerId}:${input.peerId}`)) return null;
        const duplicate = get().notifications.some(
          (item) =>
            item.dedupeKey === input.dedupeKey &&
            Math.abs(item.createdAt - input.createdAt) <= DEDUPE_WINDOW_MS,
        );
        if (duplicate) return null;
        const notification: HarborNotification = {
          ...input,
          id: crypto.randomUUID(),
          read: false,
        };
        set((state) => ({
          notifications: [notification, ...state.notifications].slice(0, MAX_NOTIFICATIONS),
        }));
        return notification;
      },
      markRead: (id) =>
        set((state) => ({
          notifications: state.notifications.map((item) =>
            item.id === id ? { ...item, read: true } : item,
          ),
        })),
      markAllRead: () =>
        set((state) => ({
          notifications: state.notifications.map((item) => ({ ...item, read: true })),
        })),
      markOwnerRead: (ownerPeerId) =>
        set((state) => ({
          notifications: state.notifications.map((item) =>
            item.ownerPeerId === ownerPeerId ? { ...item, read: true } : item,
          ),
        })),
      remove: (id) =>
        set((state) => ({
          notifications: state.notifications.filter((item) => item.id !== id),
        })),
      clear: () => set({ notifications: [] }),
      clearOwner: (ownerPeerId) =>
        set((state) => ({
          notifications: state.notifications.filter((item) => item.ownerPeerId !== ownerPeerId),
        })),
      setPeerMuted: (ownerPeerId, peerId, muted) => {
        const key = `${ownerPeerId}:${peerId}`;
        set((state) => ({
          mutedPeerIds: muted
            ? [...new Set([...state.mutedPeerIds, key])]
            : state.mutedPeerIds.filter((item) => item !== key),
        }));
      },
      setNativeEnabled: (nativeEnabled) => set({ nativeEnabled }),
      setShowMessagePreviews: (showMessagePreviews) => set({ showMessagePreviews }),
      reset: () => set(initialState),
    }),
    {
      name: 'harbor-notifications-v1',
      partialize: (state) => ({
        notifications: state.notifications,
        mutedPeerIds: state.mutedPeerIds,
        nativeEnabled: state.nativeEnabled,
        showMessagePreviews: state.showMessagePreviews,
      }),
    },
  ),
);
