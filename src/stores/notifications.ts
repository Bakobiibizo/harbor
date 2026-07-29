import { create } from 'zustand';
import { requireProfileId } from '../services/profileSession';
import { migrateLegacyProfileValue, profileStorageKey } from '../services/profileStorage';

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
const NOTIFICATIONS_LEGACY_KEY = 'harbor-notifications-v1';
const NOTIFICATIONS_PROFILE_NAMESPACE = 'notifications';
const NOTIFICATIONS_PROFILE_VERSION = 1;

const initialState = {
  notifications: [] as HarborNotification[],
  mutedPeerIds: [] as string[],
  nativeEnabled: false,
  showMessagePreviews: false,
};

type PersistedNotifications = typeof initialState;

function persistedNotifications(state: NotificationState): PersistedNotifications {
  return {
    notifications: state.notifications,
    mutedPeerIds: state.mutedPeerIds,
    nativeEnabled: state.nativeEnabled,
    showMessagePreviews: state.showMessagePreviews,
  };
}

function writeNotifications(state: NotificationState): void {
  localStorage.setItem(
    profileStorageKey(NOTIFICATIONS_PROFILE_NAMESPACE, NOTIFICATIONS_PROFILE_VERSION),
    JSON.stringify({
      state: persistedNotifications(state),
      version: NOTIFICATIONS_PROFILE_VERSION,
    }),
  );
}

function readNotifications(): PersistedNotifications {
  const raw = migrateLegacyProfileValue(
    NOTIFICATIONS_LEGACY_KEY,
    NOTIFICATIONS_PROFILE_NAMESPACE,
    NOTIFICATIONS_PROFILE_VERSION,
  );
  if (!raw) return initialState;
  try {
    const parsed = JSON.parse(raw) as { state?: Partial<PersistedNotifications> };
    const state = parsed.state ?? {};
    return {
      notifications: Array.isArray(state.notifications) ? state.notifications : [],
      mutedPeerIds: Array.isArray(state.mutedPeerIds)
        ? state.mutedPeerIds.filter((value): value is string => typeof value === 'string')
        : [],
      nativeEnabled: state.nativeEnabled === true,
      showMessagePreviews: state.showMessagePreviews === true,
    };
  } catch {
    return initialState;
  }
}

export const useNotificationsStore = create<NotificationState>()((set, get) => {
  const commit = (
    update:
      | Partial<PersistedNotifications>
      | ((state: NotificationState) => Partial<PersistedNotifications>),
  ) => {
    requireProfileId();
    set(update);
    writeNotifications(get());
  };

  return {
    ...initialState,
    add: (input) => {
      requireProfileId();
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
      commit((state) => ({
        notifications: [notification, ...state.notifications].slice(0, MAX_NOTIFICATIONS),
      }));
      return notification;
    },
    markRead: (id) =>
      commit((state) => ({
        notifications: state.notifications.map((item) =>
          item.id === id ? { ...item, read: true } : item,
        ),
      })),
    markAllRead: () =>
      commit((state) => ({
        notifications: state.notifications.map((item) => ({ ...item, read: true })),
      })),
    markOwnerRead: (ownerPeerId) =>
      commit((state) => ({
        notifications: state.notifications.map((item) =>
          item.ownerPeerId === ownerPeerId ? { ...item, read: true } : item,
        ),
      })),
    remove: (id) =>
      commit((state) => ({
        notifications: state.notifications.filter((item) => item.id !== id),
      })),
    clear: () => commit({ notifications: [] }),
    clearOwner: (ownerPeerId) =>
      commit((state) => ({
        notifications: state.notifications.filter((item) => item.ownerPeerId !== ownerPeerId),
      })),
    setPeerMuted: (ownerPeerId, peerId, muted) => {
      const key = `${ownerPeerId}:${peerId}`;
      commit((state) => ({
        mutedPeerIds: muted
          ? [...new Set([...state.mutedPeerIds, key])]
          : state.mutedPeerIds.filter((item) => item !== key),
      }));
    },
    setNativeEnabled: (nativeEnabled) => commit({ nativeEnabled }),
    setShowMessagePreviews: (showMessagePreviews) => commit({ showMessagePreviews }),
    reset: () => set(initialState),
  };
});

export function hydrateNotificationsProfile(): void {
  useNotificationsStore.setState(readNotifications());
}

export function resetNotificationsProfileMemory(): void {
  useNotificationsStore.setState(initialState);
}
