import { useEffect, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { isTauri } from '@tauri-apps/api/core';

export function useAppVersion(): string {
  const [version, setVersion] = useState('Development build');

  useEffect(() => {
    if (!isTauri()) return;

    let active = true;
    getVersion()
      .then((installedVersion) => {
        if (active) setVersion(`v${installedVersion}`);
      })
      .catch((error) => {
        console.warn('Could not read the installed Harbor version', error);
      });

    return () => {
      active = false;
    };
  }, []);

  return version;
}
