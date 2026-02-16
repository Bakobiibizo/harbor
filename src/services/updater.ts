import { check, type DownloadEvent } from '@tauri-apps/plugin-updater';
import { ask, message } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';

export interface UpdateInfo {
  available: boolean;
  currentVersion: string;
  version?: string;
  date?: string;
  body?: string;
}

/** Check if an update is available */
export async function checkForUpdate(): Promise<UpdateInfo> {
  try {
    const update = await check();

    if (update) {
      return {
        available: true,
        currentVersion: update.currentVersion,
        version: update.version,
        date: update.date,
        body: update.body,
      };
    }

    return {
      available: false,
      currentVersion: 'unknown',
    };
  } catch (error) {
    console.error('Failed to check for updates:', error);
    throw error;
  }
}

/** Download and install an update, then relaunch the app */
export async function downloadAndInstallUpdate(): Promise<void> {
  const update = await check();

  if (!update) {
    await message('No update available', { title: 'Harbor Update', kind: 'info' });
    return;
  }

  const shouldUpdate = await ask(
    `A new version of Harbor is available!\n\nCurrent: ${update.currentVersion}\nNew: ${update.version}\n\n${update.body || 'No release notes available.'}\n\nWould you like to update now?`,
    { title: 'Update Available', kind: 'info', okLabel: 'Update', cancelLabel: 'Later' },
  );

  if (!shouldUpdate) {
    return;
  }

  let downloaded = 0;
  let contentLength = 0;

  // Download the update with progress tracking
  await update.downloadAndInstall((event: DownloadEvent) => {
    switch (event.event) {
      case 'Started':
        contentLength = event.data.contentLength || 0;
        console.log(`[Updater] Download started, size: ${contentLength} bytes`);
        break;
      case 'Progress':
        downloaded += event.data.chunkLength;
        const percent = contentLength > 0 ? Math.round((downloaded / contentLength) * 100) : 0;
        console.log(`[Updater] Download progress: ${percent}%`);
        break;
      case 'Finished':
        console.log('[Updater] Download finished');
        break;
    }
  });

  // Ask to relaunch
  const shouldRelaunch = await ask(
    'Update installed successfully! The app needs to restart to apply the update.\n\nRestart now?',
    { title: 'Update Installed', kind: 'info', okLabel: 'Restart', cancelLabel: 'Later' },
  );

  if (shouldRelaunch) {
    await relaunch();
  }
}
