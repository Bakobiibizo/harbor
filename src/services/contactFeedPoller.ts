import type { Contact, ContactRequest } from '../types';

export interface ContactFeedPollContext {
  profileId: string | null;
  online: boolean;
  enabled: boolean;
  intervalMs: number;
  contacts: readonly Contact[];
  requests: readonly ContactRequest[];
}

export interface ContactFeedPollResult {
  /** False when the backend can prove its durable cursor did not advance. */
  changed?: boolean;
}

export interface ContactFeedPollerDependencies {
  fetchContact: (peerId: string) => Promise<ContactFeedPollResult | void>;
  publishRefresh: (peerId: string) => void;
  now?: () => number;
  random?: () => number;
}

export interface ContactFeedPollerOptions {
  concurrency?: number;
  startupJitterMs?: number;
  jitterRatio?: number;
  backoffBaseMs?: number;
  maxBackoffMs?: number;
  minIntervalMs?: number;
}

interface PeerSchedule {
  failures: number;
  dueAt: number;
}

const DEFAULT_INTERVAL_MS = 5 * 60_000;

/**
 * Incrementally asks the relay for accepted contacts' walls. Durable cursors
 * remain in SQLite and are applied by `fetch_contact_wall_from_relay`; this
 * scheduler deliberately never maintains a second browser-side cursor.
 */
export class ContactFeedPoller {
  private readonly concurrency: number;
  private readonly startupJitterMs: number;
  private readonly jitterRatio: number;
  private readonly backoffBaseMs: number;
  private readonly maxBackoffMs: number;
  private readonly minIntervalMs: number;
  private readonly now: () => number;
  private readonly random: () => number;
  private context: ContactFeedPollContext | null = null;
  private lifecycleKey = '';
  private authorizationKey = '';
  private schedules = new Map<string, PeerSchedule>();
  private timer: ReturnType<typeof setTimeout> | null = null;
  private generation = 0;
  private runningGeneration: number | null = null;
  private trailingRun = false;

  constructor(
    private readonly dependencies: ContactFeedPollerDependencies,
    options: ContactFeedPollerOptions = {},
  ) {
    this.concurrency = Math.max(1, Math.floor(options.concurrency ?? 3));
    this.startupJitterMs = Math.max(0, options.startupJitterMs ?? 2_000);
    this.jitterRatio = Math.min(0.5, Math.max(0, options.jitterRatio ?? 0.1));
    this.backoffBaseMs = Math.max(1, options.backoffBaseMs ?? 15_000);
    this.maxBackoffMs = Math.max(this.backoffBaseMs, options.maxBackoffMs ?? 30 * 60_000);
    this.minIntervalMs = Math.max(1, options.minIntervalMs ?? 60_000);
    this.now = dependencies.now ?? Date.now;
    this.random = dependencies.random ?? Math.random;
  }

  update(context: ContactFeedPollContext): void {
    const contacts = eligibleContactIds(context.contacts, context.requests);
    const intervalMs = Math.max(this.minIntervalMs, context.intervalMs);
    const lifecycleKey = `${context.profileId ?? ''}:${context.online}:${context.enabled}`;
    const authorizationKey = contacts.join('\u0000');
    const previousLifecycle = this.lifecycleKey;
    const previousAuthorization = this.authorizationKey;
    const intervalChanged = this.context !== null && this.context.intervalMs !== intervalMs;
    this.context = { ...context, intervalMs };
    this.lifecycleKey = lifecycleKey;
    this.authorizationKey = authorizationKey;

    if (!this.isActive()) {
      this.cancelAndReset();
      return;
    }

    if (previousLifecycle !== lifecycleKey) {
      this.cancelAndReset();
      this.seedSchedules(contacts, this.now() + this.random() * this.startupJitterMs);
      this.scheduleNext();
      return;
    }

    if (previousAuthorization !== authorizationKey) {
      const authorized = new Set(contacts);
      for (const peerId of this.schedules.keys()) {
        if (!authorized.has(peerId)) this.schedules.delete(peerId);
      }
      this.seedSchedules(contacts, this.now() + this.random() * this.startupJitterMs);
      this.scheduleNext(true);
      return;
    }

    if (intervalChanged) {
      const dueAt = this.now() + this.jittered(intervalMs);
      for (const schedule of this.schedules.values()) schedule.dueAt = dueAt;
      this.scheduleNext(true);
      return;
    }

    this.scheduleNext();
  }

  stop(): void {
    this.context = null;
    this.lifecycleKey = '';
    this.authorizationKey = '';
    this.cancelAndReset();
  }

  private isActive(): boolean {
    return Boolean(this.context?.profileId && this.context.online && this.context.enabled);
  }

  private cancelAndReset(): void {
    this.generation += 1;
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    this.schedules.clear();
    this.trailingRun = false;
  }

  private seedSchedules(peerIds: readonly string[], dueAt: number): void {
    for (const peerId of peerIds) {
      if (!this.schedules.has(peerId)) this.schedules.set(peerId, { failures: 0, dueAt });
    }
  }

  private scheduleNext(replace = false): void {
    if (!this.isActive() || this.schedules.size === 0 || (this.timer && !replace)) return;
    if (this.timer) clearTimeout(this.timer);
    const dueAt = Math.min(...[...this.schedules.values()].map((schedule) => schedule.dueAt));
    this.timer = setTimeout(
      () => {
        this.timer = null;
        void this.run();
      },
      Math.max(0, dueAt - this.now()),
    );
  }

  private async run(): Promise<void> {
    if (!this.isActive()) return;
    const generation = this.generation;
    if (this.runningGeneration === generation) {
      this.trailingRun = true;
      return;
    }

    const context = this.context;
    if (!context) return;
    const authorized = new Set(eligibleContactIds(context.contacts, context.requests));
    const now = this.now();
    const duePeers = [...this.schedules]
      .filter(([peerId, schedule]) => authorized.has(peerId) && schedule.dueAt <= now)
      .map(([peerId]) => peerId);
    if (duePeers.length === 0) {
      this.scheduleNext();
      return;
    }

    this.runningGeneration = generation;
    let cursor = 0;
    const worker = async () => {
      while (cursor < duePeers.length) {
        const peerId = duePeers[cursor++];
        await this.pollPeer(peerId, generation);
      }
    };
    await Promise.all(
      Array.from({ length: Math.min(this.concurrency, duePeers.length) }, () => worker()),
    );
    if (this.runningGeneration === generation) this.runningGeneration = null;

    if (generation !== this.generation || !this.isActive()) {
      this.scheduleNext();
      return;
    }
    if (this.trailingRun) {
      this.trailingRun = false;
      void this.run();
      return;
    }
    this.scheduleNext();
  }

  private async pollPeer(peerId: string, generation: number): Promise<void> {
    const schedule = this.schedules.get(peerId);
    if (!schedule) return;
    try {
      const result = await this.dependencies.fetchContact(peerId);
      if (generation !== this.generation || !this.isActive() || !this.schedules.has(peerId)) return;
      schedule.failures = 0;
      schedule.dueAt = this.now() + this.jittered(this.context?.intervalMs ?? DEFAULT_INTERVAL_MS);
      if (result?.changed !== false) this.dependencies.publishRefresh(peerId);
    } catch (error) {
      if (generation !== this.generation || !this.isActive() || !this.schedules.has(peerId)) return;
      schedule.failures += 1;
      const backoff = Math.min(
        this.maxBackoffMs,
        this.backoffBaseMs * 2 ** Math.min(20, schedule.failures - 1),
      );
      schedule.dueAt = this.now() + this.jittered(backoff);
      console.warn(`[ContactFeedPoller] Poll failed for ${peerId}`, error);
    }
  }

  private jittered(delayMs: number): number {
    const multiplier = 1 + (this.random() * 2 - 1) * this.jitterRatio;
    return Math.max(1, Math.round(delayMs * multiplier));
  }
}

export function eligibleContactIds(
  contacts: readonly Contact[],
  requests: readonly ContactRequest[],
): string[] {
  const latestRequest = new Map<string, ContactRequest>();
  for (const request of requests) {
    const current = latestRequest.get(request.peerId);
    if (!current || request.updatedAt > current.updatedAt)
      latestRequest.set(request.peerId, request);
  }
  return [
    ...new Set(
      contacts
        .filter((contact) => {
          if (contact.isBlocked) return false;
          const request = latestRequest.get(contact.peerId);
          return !request || request.status === 'accepted';
        })
        .map((contact) => contact.peerId),
    ),
  ].sort();
}
