import { useNotificationsStore, type HarborNotificationKind } from '../stores/notifications';
import { useIdentityStore } from '../stores/identity';
import { sendNativeHarborNotification } from './nativeNotifications';

interface NotifyInput {
  kind: HarborNotificationKind;
  peerId: string;
  senderName: string;
  eventId: string;
  mediaMode?: 'audio' | 'video';
}

export function notifyHarborEvent(input: NotifyInput) {
  const identityState = useIdentityStore.getState().state;
  if (identityState.status !== 'unlocked') return null;
  const label = input.senderName;
  const copy = (() => {
    switch (input.kind) {
      case 'incoming_call':
        return {
          title: `Incoming ${input.mediaMode === 'video' ? 'video' : 'voice'} call`,
          body: `${label} is calling.`,
        };
      case 'missed_call':
        return { title: 'Missed call', body: `You missed a call from ${label}.` };
      default:
        return {
          title: `Message from ${label}`,
          body: 'Open Harbor to read this private message.',
        };
    }
  })();
  const route = '/chat';
  const notification = useNotificationsStore.getState().add({
    dedupeKey: `${input.kind}:${input.peerId}:${input.eventId}`,
    kind: input.kind,
    ownerPeerId: identityState.identity.peerId,
    peerId: input.peerId,
    senderName: label,
    title: copy.title,
    body: copy.body,
    route,
    createdAt: Date.now(),
  });
  if (notification) void sendNativeHarborNotification(notification);
  return notification;
}
