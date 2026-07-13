import { isTauri } from '@tauri-apps/api/core';
import { useNotificationsStore, type HarborNotification } from '../stores/notifications';

export async function requestNativeNotificationPermission(): Promise<boolean> {
  if (!isTauri()) return false;
  const { isPermissionGranted, requestPermission } = await import(
    '@tauri-apps/plugin-notification'
  );
  if (await isPermissionGranted()) return true;
  return (await requestPermission()) === 'granted';
}

export async function sendNativeHarborNotification(
  notification: HarborNotification,
): Promise<boolean> {
  if (!isTauri() || !useNotificationsStore.getState().nativeEnabled) return false;
  try {
    const { isPermissionGranted, sendNotification } = await import(
      '@tauri-apps/plugin-notification'
    );
    if (!(await isPermissionGranted())) return false;
    sendNotification({
      title: notification.title,
      body: notification.body,
      group: `harbor-${notification.kind}-${notification.peerId}`,
      autoCancel: true,
      extra: { notificationId: notification.id, route: notification.route },
    });
    return true;
  } catch (error) {
    console.warn('[Notifications] Native notification failed:', error);
    return false;
  }
}
