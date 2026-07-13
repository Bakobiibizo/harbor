import { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { HarborIcon } from '../icons';

const appWindow = getCurrentWindow();

export function WindowsTitleBar() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const syncWindowState = async () => {
      setIsMaximized(await appWindow.isMaximized());
    };

    void syncWindowState();
    void appWindow.onResized(syncWindowState).then((stopListening) => {
      unlisten = stopListening;
    });

    return () => unlisten?.();
  }, []);

  return (
    <header
      data-tauri-drag-region
      className="h-10 flex shrink-0 select-none items-center border-b"
      style={{
        background: 'hsl(var(--harbor-bg-elevated))',
        borderColor: 'hsl(var(--harbor-border-subtle))',
      }}
      onDoubleClick={() => void appWindow.toggleMaximize()}
    >
      <div data-tauri-drag-region className="flex min-w-0 flex-1 items-center gap-2.5 px-3">
        <HarborIcon className="h-5 w-5 shrink-0" />
        <span
          data-tauri-drag-region
          className="truncate text-sm font-semibold tracking-wide"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          Harbor
        </span>
        <span
          data-tauri-drag-region
          className="rounded-full px-2 py-0.5 text-[10px] font-medium uppercase tracking-[0.12em]"
          style={{
            background: 'hsl(var(--harbor-primary) / 0.12)',
            color: 'hsl(var(--harbor-primary))',
          }}
        >
          Private beta
        </span>
      </div>

      <div className="flex h-full" onDoubleClick={(event) => event.stopPropagation()}>
        <button
          type="button"
          className="group grid h-full w-12 place-items-center transition-colors hover:bg-white/5"
          aria-label="Minimize Harbor"
          title="Minimize"
          onClick={() => void appWindow.minimize()}
        >
          <svg
            aria-hidden="true"
            className="h-3.5 w-3.5"
            viewBox="0 0 16 16"
            style={{ color: 'hsl(var(--harbor-text-secondary))' }}
          >
            <path d="M3 8.5h10v1H3z" fill="currentColor" />
          </svg>
        </button>
        <button
          type="button"
          className="group grid h-full w-12 place-items-center transition-colors hover:bg-white/5"
          aria-label={isMaximized ? 'Restore Harbor window' : 'Maximize Harbor'}
          title={isMaximized ? 'Restore' : 'Maximize'}
          onClick={() => void appWindow.toggleMaximize()}
        >
          <svg
            aria-hidden="true"
            className="h-3.5 w-3.5"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            style={{ color: 'hsl(var(--harbor-text-secondary))' }}
          >
            {isMaximized ? (
              <>
                <path d="M5.5 5.5h7v7h-7z" />
                <path d="M3.5 10.5v-7h7" />
              </>
            ) : (
              <path d="M3.5 3.5h9v9h-9z" />
            )}
          </svg>
        </button>
        <button
          type="button"
          className="group grid h-full w-12 place-items-center transition-colors hover:bg-red-600"
          aria-label="Close Harbor"
          title="Close"
          onClick={() => void appWindow.close()}
        >
          <svg
            aria-hidden="true"
            className="h-3.5 w-3.5"
            viewBox="0 0 16 16"
            stroke="currentColor"
            style={{ color: 'hsl(var(--harbor-text-secondary))' }}
          >
            <path d="m3.5 3.5 9 9m0-9-9 9" />
          </svg>
        </button>
      </div>
    </header>
  );
}
