export type RefreshDomain = 'contacts' | 'requests' | 'messages' | 'posts' | 'media';

export interface RefreshHint {
  domains: RefreshDomain[];
  peerId?: string;
}

export type RefreshHandler = (peerIds: ReadonlySet<string>) => Promise<unknown> | unknown;

export interface ReactiveRefreshHandlers {
  contacts: RefreshHandler;
  requests: RefreshHandler;
  messages: RefreshHandler;
  posts: RefreshHandler;
  media: RefreshHandler;
}

interface ReactiveRefreshOptions {
  debounceMs?: number;
  fallbackMs?: number;
}

const DEFAULT_FALLBACK_MS = 60_000;

/**
 * Coalesces backend change notifications into one local-state reconciliation pass.
 * Refreshes raised while a pass is running are retained for one trailing pass, so
 * bursts cannot overlap or recursively fan out into refresh loops.
 */
export class ReactiveRefreshCoordinator {
  private readonly debounceMs: number;
  private readonly fallbackMs: number;
  private readonly pending = new Map<RefreshDomain, Set<string>>();
  private debounceHandle: ReturnType<typeof setTimeout> | null = null;
  private fallbackHandle: ReturnType<typeof setInterval> | null = null;
  private running = false;
  private stopped = false;

  constructor(
    private readonly handlers: ReactiveRefreshHandlers,
    options: ReactiveRefreshOptions = {},
  ) {
    this.debounceMs = Math.max(0, options.debounceMs ?? 100);
    this.fallbackMs = Math.max(5_000, options.fallbackMs ?? DEFAULT_FALLBACK_MS);
  }

  start(): void {
    if (this.fallbackHandle || this.stopped) return;
    this.fallbackHandle = setInterval(() => {
      this.enqueue({ domains: ['contacts', 'requests', 'messages', 'posts'] });
    }, this.fallbackMs);
  }

  enqueue(hint: RefreshHint): void {
    if (this.stopped) return;
    for (const domain of hint.domains) {
      const peers = this.pending.get(domain) ?? new Set<string>();
      if (hint.peerId) peers.add(hint.peerId);
      this.pending.set(domain, peers);
    }
    this.schedule();
  }

  stop(): void {
    this.stopped = true;
    if (this.debounceHandle) clearTimeout(this.debounceHandle);
    if (this.fallbackHandle) clearInterval(this.fallbackHandle);
    this.debounceHandle = null;
    this.fallbackHandle = null;
    this.pending.clear();
  }

  private schedule(): void {
    if (this.running || this.debounceHandle || this.pending.size === 0) return;
    this.debounceHandle = setTimeout(() => {
      this.debounceHandle = null;
      void this.flush();
    }, this.debounceMs);
  }

  private async flush(): Promise<void> {
    if (this.running || this.stopped || this.pending.size === 0) return;
    this.running = true;
    const batch = new Map(this.pending);
    this.pending.clear();

    await Promise.allSettled(
      [...batch].map(([domain, peers]) => Promise.resolve(this.handlers[domain](peers))),
    );

    this.running = false;
    this.schedule();
  }
}
