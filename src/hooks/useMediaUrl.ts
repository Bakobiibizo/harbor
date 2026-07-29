import { useEffect, useState } from 'react';
import { mediaService } from '../services/media';

/** Resolve content-addressed media through Tauri's packaged asset protocol. */
export function useMediaUrl(mediaHash: string | null | undefined): string | null {
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setUrl(null);
    if (!mediaHash)
      return () => {
        active = false;
      };
    void mediaService.getMediaUrl(mediaHash).then(
      (resolved) => {
        if (active) setUrl(resolved);
      },
      () => {
        if (active) setUrl(null);
      },
    );
    return () => {
      active = false;
    };
  }, [mediaHash]);

  return url;
}
