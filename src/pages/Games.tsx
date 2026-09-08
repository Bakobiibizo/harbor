import { useCallback, useEffect, useRef, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import toast from 'react-hot-toast';
import { Button } from '../components/common/Button';
import {
  configureGameDiscoveryFolder,
  deleteGameSaves,
  getGameDiscoveryFolder,
  getStoreGameMetadata,
  importGamePackage,
  inspectDiscoveredGamePackages,
  inspectGamePackage,
  installStoreGame,
  listInstalledGames,
  uninstallGame,
} from '../services/games';
import type {
  GameDiscoveryPreview,
  GameInstallation,
  GamePackagePreview,
  StoreGamePreview,
} from '../types';
import { getErrorMessage } from '../utils/errors';
import {
  attachGameInput,
  createHarborGameRuntime,
  type HarborGameRuntimeController,
  type HarborGameRuntimeState,
} from '../games/harborGameRuntime';

export const PRODUCTION_GAMES_ORIGIN = 'https://games.social-harbor.com';
const developmentOrigin = import.meta.env.DEV
  ? import.meta.env.VITE_HARBOR_GAMES_ORIGIN
  : undefined;
export const GAMES_ORIGIN = developmentOrigin || PRODUCTION_GAMES_ORIGIN;

interface InstallIntent {
  gameId: string;
  source: 'neo-grounds';
  type: 'harbor-game-install-intent';
  version: 1;
  versionId: string;
}

interface PendingInstall {
  filePath?: string;
  preview: GamePackagePreview | StoreGamePreview;
  source: 'manual' | 'folder' | 'store';
}

export function parseStoreInstallIntent(
  event: Pick<MessageEvent, 'data' | 'origin' | 'source'>,
  expectedSource: Window | null,
): InstallIntent | null {
  if (event.origin !== GAMES_ORIGIN || event.source !== expectedSource) return null;
  const value = event.data;
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const keys = Object.keys(value).sort();
  const expected = ['gameId', 'source', 'type', 'version', 'versionId'];
  if (keys.length !== expected.length || keys.some((key, index) => key !== expected[index])) {
    return null;
  }
  if (
    value.source !== 'neo-grounds' ||
    value.type !== 'harbor-game-install-intent' ||
    value.version !== 1 ||
    !validIdentifier(value.gameId) ||
    !validIdentifier(value.versionId)
  ) {
    return null;
  }
  return value as InstallIntent;
}

export function getHarborParentOrigin(location: Pick<Location, 'origin' | 'protocol'>): string {
  if (location.protocol === 'tauri:') return 'tauri://localhost';
  if (location.origin === 'https://tauri.localhost') return location.origin;
  return 'https://tauri.localhost';
}

export function GamesPage() {
  const [tab, setTab] = useState<'store' | 'library'>('store');
  const [games, setGames] = useState<GameInstallation[]>([]);
  const [folder, setFolder] = useState<string | null>(null);
  const [discoveries, setDiscoveries] = useState<GameDiscoveryPreview[]>([]);
  const [pendingInstall, setPendingInstall] = useState<PendingInstall | null>(null);
  const [selectedGame, setSelectedGame] = useState<GameInstallation | null>(null);
  const [uninstallTarget, setUninstallTarget] = useState<GameInstallation | null>(null);
  const [busy, setBusy] = useState(false);
  const [online, setOnline] = useState(() => navigator.onLine);
  const iframeRef = useRef<HTMLIFrameElement>(null);

  const refreshLibrary = useCallback(async () => {
    const [installed, configuredFolder] = await Promise.all([
      listInstalledGames(),
      getGameDiscoveryFolder(),
    ]);
    setGames(installed);
    setFolder(configuredFolder);
  }, []);

  useEffect(() => {
    void refreshLibrary().catch((error) => toast.error(getErrorMessage(error)));
  }, [refreshLibrary]);

  useEffect(() => {
    const updateOnline = () => setOnline(navigator.onLine);
    window.addEventListener('online', updateOnline);
    window.addEventListener('offline', updateOnline);
    return () => {
      window.removeEventListener('online', updateOnline);
      window.removeEventListener('offline', updateOnline);
    };
  }, []);

  useEffect(() => {
    const receiveIntent = (event: MessageEvent) => {
      const intent = parseStoreInstallIntent(event, iframeRef.current?.contentWindow ?? null);
      if (!intent) return;
      setBusy(true);
      void getStoreGameMetadata(intent.gameId, intent.versionId)
        .then((preview) => setPendingInstall({ preview, source: 'store' }))
        .catch((error) => toast.error(`Could not resolve approved game: ${getErrorMessage(error)}`))
        .finally(() => setBusy(false));
    };
    window.addEventListener('message', receiveIntent);
    return () => window.removeEventListener('message', receiveIntent);
  }, []);

  async function choosePackage() {
    const selected = await open({
      filters: [{ extensions: ['harborgame'], name: 'Harbor Game' }],
      multiple: false,
    });
    if (typeof selected !== 'string') return;
    setBusy(true);
    try {
      const preview = await inspectGamePackage(selected);
      setPendingInstall({ filePath: selected, preview, source: 'manual' });
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function chooseFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== 'string') return;
    setBusy(true);
    try {
      await configureGameDiscoveryFolder(selected);
      setFolder(selected);
      setDiscoveries(await inspectDiscoveredGamePackages());
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function scanFolder() {
    setBusy(true);
    try {
      setDiscoveries(await inspectDiscoveredGamePackages());
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function confirmInstall() {
    if (!pendingInstall) return;
    setBusy(true);
    try {
      const { preview } = pendingInstall;
      if (pendingInstall.source === 'store') {
        await installStoreGame(preview.gameId, preview.versionId, preview.permissions);
      } else {
        await importGamePackage(pendingInstall.filePath!, preview.permissions);
      }
      toast.success(`${preview.title} installed`);
      setPendingInstall(null);
      setTab('library');
      await refreshLibrary();
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function confirmUninstall(deleteSaves: boolean) {
    if (!uninstallTarget) return;
    setBusy(true);
    try {
      await uninstallGame(uninstallTarget.gameId);
      if (deleteSaves) await deleteGameSaves(uninstallTarget.gameId);
      setUninstallTarget(null);
      await refreshLibrary();
      toast.success('Game uninstalled');
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  const parentOrigin = getHarborParentOrigin(window.location);
  const storeUrl = `${GAMES_ORIGIN}/store?parentOrigin=${encodeURIComponent(parentOrigin)}`;

  return (
    <div className="min-h-full p-4 md:p-6">
      <div className="mx-auto max-w-6xl space-y-5">
        <header className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.2em] text-cyan-400">
              Local-first arcade
            </p>
            <h1 className="text-3xl font-bold">Games</h1>
            <p style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Browse approved games, then install and play them locally.
            </p>
          </div>
          <div className="flex gap-2" role="tablist" aria-label="Games sections">
            {(['store', 'library'] as const).map((section) => (
              <button
                key={section}
                className="rounded-lg px-4 py-2 text-sm font-semibold"
                role="tab"
                aria-selected={tab === section}
                onClick={() => setTab(section)}
                style={{
                  background:
                    tab === section ? 'hsl(var(--harbor-primary))' : 'hsl(var(--harbor-surface-1))',
                  color: tab === section ? 'white' : 'hsl(var(--harbor-text-primary))',
                }}
              >
                {section === 'store' ? 'Store' : `Library (${games.length})`}
              </button>
            ))}
          </div>
        </header>

        {developmentOrigin && (
          <Notice tone="warning">
            Development store override active: {developmentOrigin}. Release builds always use{' '}
            {PRODUCTION_GAMES_ORIGIN}.
          </Notice>
        )}

        {tab === 'store' ? (
          <section
            className="overflow-hidden rounded-xl"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            {!online && (
              <Notice tone="warning">
                The store is offline. Installed games remain available in your library.
              </Notice>
            )}
            <iframe
              ref={iframeRef}
              className="h-[calc(100vh-15rem)] min-h-[32rem] w-full border-0"
              sandbox="allow-forms allow-same-origin allow-scripts"
              src={storeUrl}
              title="Neo Grounds approved Harbor games store"
            />
          </section>
        ) : (
          <>
            <section
              className="rounded-xl p-5"
              style={{
                background: 'hsl(var(--harbor-bg-elevated))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
              }}
            >
              <div className="flex flex-wrap gap-3">
                <Button onClick={choosePackage} loading={busy}>
                  Import .harborgame
                </Button>
                <Button variant="secondary" onClick={chooseFolder} disabled={busy}>
                  Choose Games Folder
                </Button>
                <Button variant="secondary" onClick={scanFolder} disabled={busy || !folder}>
                  Scan Folder
                </Button>
              </div>
              <p
                className="mt-3 break-all text-xs"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                {folder ? `Discovery source: ${folder}` : 'No discovery folder configured.'} This
                machine-visible folder is scanned non-recursively. Harbor validates and copies
                packages into this profile before play; it never executes files in place.
              </p>
            </section>

            {discoveries.length > 0 && (
              <section className="space-y-3" aria-label="Discovered game packages">
                <h2 className="text-xl font-semibold">Discovered packages</h2>
                {discoveries.map((item) => (
                  <div
                    key={item.filePath}
                    className="flex flex-wrap items-center justify-between gap-3 rounded-xl p-4"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      border: '1px solid hsl(var(--harbor-border-subtle))',
                    }}
                  >
                    <div>
                      <p className="font-medium">{item.package?.title ?? item.fileName}</p>
                      <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                        {item.error ??
                          `${item.package?.versionId} · creator ${item.package?.creatorPeerId}`}
                      </p>
                    </div>
                    {item.package && (
                      <Button
                        size="sm"
                        onClick={() =>
                          setPendingInstall({
                            filePath: item.filePath,
                            preview: item.package!,
                            source: 'folder',
                          })
                        }
                      >
                        Review & Install
                      </Button>
                    )}
                  </div>
                ))}
              </section>
            )}

            <section
              className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3"
              aria-label="Installed games"
            >
              {games.map((game) => (
                <GameCard
                  key={game.gameId}
                  game={game}
                  onPlay={() => setSelectedGame(game)}
                  onUninstall={() => setUninstallTarget(game)}
                />
              ))}
              {!games.length && (
                <Notice>Your library is empty. Import a package or install from the store.</Notice>
              )}
            </section>
          </>
        )}
      </div>

      {pendingInstall && (
        <InstallDialog
          pending={pendingInstall}
          busy={busy}
          onCancel={() => setPendingInstall(null)}
          onConfirm={confirmInstall}
        />
      )}
      {uninstallTarget && (
        <UninstallDialog
          game={uninstallTarget}
          busy={busy}
          onCancel={() => setUninstallTarget(null)}
          onConfirm={confirmUninstall}
        />
      )}
      {selectedGame && (
        <GamePlayer
          game={selectedGame}
          onClose={() => {
            setSelectedGame(null);
            void refreshLibrary();
          }}
        />
      )}
    </div>
  );
}

function GameCard({
  game,
  onPlay,
  onUninstall,
}: {
  game: GameInstallation;
  onPlay: () => void;
  onUninstall: () => void;
}) {
  return (
    <article
      className="rounded-xl p-5"
      style={{
        background: 'hsl(var(--harbor-surface-1))',
        border: '1px solid hsl(var(--harbor-border-subtle))',
      }}
    >
      <p className="text-xs font-semibold uppercase tracking-wide text-cyan-400">{game.source}</p>
      <h2 className="mt-1 text-xl font-semibold">{game.title}</h2>
      <p className="mt-2 break-all text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
        Creator {game.creatorPeerId}
      </p>
      <p className="mt-2 text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
        Version {game.versionId} · {formatBytes(game.byteLength)} · Played {game.playCount} times
      </p>
      <p className="mt-2 text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
        Permissions: {game.permissions.join(', ') || 'None'}
      </p>
      <div className="mt-4 flex gap-2">
        <Button size="sm" onClick={onPlay}>
          Play
        </Button>
        <Button size="sm" variant="secondary" onClick={onUninstall}>
          Uninstall
        </Button>
      </div>
    </article>
  );
}

function InstallDialog({
  pending,
  busy,
  onCancel,
  onConfirm,
}: {
  pending: PendingInstall;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const unsupported = pending.preview.permissions.filter(
    (permission) => permission !== 'save_data',
  );
  return (
    <Modal title="Confirm game installation" onClose={onCancel}>
      <dl className="grid grid-cols-[8rem_1fr] gap-3 text-sm">
        <Field label="Title" value={pending.preview.title} />
        <Field label="Version" value={pending.preview.versionId} />
        <Field label="Creator" value={pending.preview.creatorPeerId} mono />
        <Field label="Download" value={formatBytes(pending.preview.byteLength)} />
        <Field label="Permissions" value={pending.preview.permissions.join(', ') || 'None'} />
        <Field label="Package digest" value={pending.preview.packageDigest} mono />
      </dl>
      {pending.source === 'store' ? (
        <Notice>
          Harbor will independently resolve metadata again, download from the trusted API, and
          verify creator and store signatures before installation.
        </Notice>
      ) : (
        <Notice>This package is creator-signed but has no Harbor store approval.</Notice>
      )}
      {unsupported.length > 0 && (
        <Notice tone="error">
          This Harbor runtime does not support: {unsupported.join(', ')}. Multiplayer is deferred.
        </Notice>
      )}
      <div className="mt-5 flex justify-end gap-3">
        <Button variant="secondary" onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <Button onClick={onConfirm} loading={busy} disabled={busy || unsupported.length > 0}>
          Verify & Install
        </Button>
      </div>
    </Modal>
  );
}

function UninstallDialog({
  game,
  busy,
  onCancel,
  onConfirm,
}: {
  game: GameInstallation;
  busy: boolean;
  onCancel: () => void;
  onConfirm: (deleteSaves: boolean) => void;
}) {
  const [deleteSaves, setDeleteSaves] = useState(false);
  return (
    <Modal title={`Uninstall ${game.title}?`} onClose={onCancel}>
      <p style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
        The installed package will stop being runnable. Saves remain unless you explicitly remove
        them.
      </p>
      <label className="mt-4 flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={deleteSaves}
          onChange={(event) => setDeleteSaves(event.target.checked)}
        />
        Also delete this profile's saves
      </label>
      <div className="mt-5 flex justify-end gap-3">
        <Button variant="secondary" onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <Button variant="danger" onClick={() => onConfirm(deleteSaves)} loading={busy}>
          Uninstall
        </Button>
      </div>
    </Modal>
  );
}

function GamePlayer({ game, onClose }: { game: GameInstallation; onClose: () => void }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const shellRef = useRef<HTMLDivElement>(null);
  const controllerRef = useRef<HarborGameRuntimeController | null>(null);
  const [state, setState] = useState<HarborGameRuntimeState>('loading');
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    let cancelled = false;
    let detachInput: (() => void) | undefined;
    void createHarborGameRuntime({
      canvas,
      gameId: game.gameId,
      onError: setError,
      onStateChange: (next) => {
        if (cancelled) return;
        setState(next);
        if (next === 'ready') controllerRef.current?.start();
      },
    })
      .then((controller) => {
        if (cancelled) {
          controller.destroy();
          return;
        }
        controllerRef.current = controller;
        detachInput = attachGameInput(canvas, (event) => controller.sendInput(event));
        if (controller.getState() === 'ready') controller.start();
      })
      .catch((caught) => setError(getErrorMessage(caught)));
    return () => {
      cancelled = true;
      detachInput?.();
      controllerRef.current?.destroy();
      controllerRef.current = null;
    };
  }, [game.gameId]);

  async function toggleFullscreen() {
    if (!shellRef.current) return;
    if (document.fullscreenElement) await document.exitFullscreen();
    else await shellRef.current.requestFullscreen();
  }

  return (
    <div
      className={`${expanded ? 'fixed inset-0' : 'fixed inset-4 md:inset-10'} z-[80] flex items-center justify-center rounded-xl p-4`}
      style={{ background: 'hsl(var(--harbor-bg-primary) / .98)' }}
      role="dialog"
      aria-modal="true"
      aria-label={`${game.title} player`}
    >
      <div ref={shellRef} className="flex h-full w-full max-w-6xl flex-col gap-3">
        <header className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 className="text-xl font-semibold">{game.title}</h2>
            <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
              {state} · local Worker runtime
            </p>
          </div>
          <div className="flex gap-2">
            {state === 'running' ? (
              <Button size="sm" variant="secondary" onClick={() => controllerRef.current?.stop()}>
                Stop
              </Button>
            ) : (
              <Button
                size="sm"
                onClick={() => controllerRef.current?.start()}
                disabled={state === 'loading'}
              >
                Start
              </Button>
            )}
            <Button size="sm" variant="secondary" onClick={() => setExpanded((value) => !value)}>
              {expanded ? 'Windowed' : 'Expand'}
            </Button>
            <Button size="sm" variant="secondary" onClick={toggleFullscreen}>
              Fullscreen
            </Button>
            <Button size="sm" variant="secondary" onClick={onClose}>
              Close
            </Button>
          </div>
        </header>
        {error && <Notice tone="error">{error}</Notice>}
        <canvas
          ref={canvasRef}
          aria-label={`${game.title} game canvas`}
          className="min-h-0 flex-1 rounded-xl bg-black outline-none focus:ring-2 focus:ring-cyan-400"
          height={600}
          tabIndex={0}
          width={800}
        />
        <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          Keyboard input is active only while the canvas is focused. Installed games work offline.
        </p>
      </div>
    </div>
  );
}

function Modal({
  title,
  children,
  onClose,
}: {
  title: string;
  children: React.ReactNode;
  onClose: () => void;
}) {
  return (
    <div
      className="fixed inset-0 z-[90] flex items-center justify-center p-4"
      style={{ background: 'rgba(0,0,0,.72)' }}
      onClick={onClose}
    >
      <section
        className="w-full max-w-xl rounded-xl p-6"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="mb-4 text-xl font-semibold">{title}</h2>
        {children}
      </section>
    </div>
  );
}

function Notice({
  children,
  tone = 'info',
}: {
  children: React.ReactNode;
  tone?: 'info' | 'warning' | 'error';
}) {
  const color =
    tone === 'error'
      ? 'var(--harbor-error)'
      : tone === 'warning'
        ? 'var(--harbor-warning)'
        : 'var(--harbor-primary)';
  return (
    <div
      className="rounded-lg p-3 text-sm"
      style={{ background: `hsl(${color} / .12)`, color: `hsl(${color})` }}
    >
      {children}
    </div>
  );
}

function Field({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <>
      <dt style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>{label}</dt>
      <dd className={mono ? 'break-all font-mono text-xs' : ''}>{value}</dd>
    </>
  );
}

function formatBytes(bytes: number): string {
  return bytes < 1024 * 1024
    ? `${Math.ceil(bytes / 1024)} KiB`
    : `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

function validIdentifier(value: unknown): value is string {
  return typeof value === 'string' && /^[A-Za-z0-9._-]{1,128}$/u.test(value);
}
