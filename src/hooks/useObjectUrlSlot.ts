import { useCallback, useEffect, useRef } from 'react';

/** Owns at most one object URL and revokes it on replacement, clear, or unmount. */
export function useObjectUrlSlot() {
  const currentUrlRef = useRef<string | null>(null);

  const clear = useCallback(() => {
    const currentUrl = currentUrlRef.current;
    if (!currentUrl) return;
    currentUrlRef.current = null;
    URL.revokeObjectURL(currentUrl);
  }, []);

  const replace = useCallback(
    (value: Blob) => {
      const nextUrl = URL.createObjectURL(value);
      clear();
      currentUrlRef.current = nextUrl;
      return nextUrl;
    },
    [clear],
  );

  useEffect(() => clear, [clear]);

  return { replace, clear };
}
