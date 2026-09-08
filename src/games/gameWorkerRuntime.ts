import {
  GAME_RUNTIME_LIMITS,
  type CanvasCommand,
  type GameInputEvent,
  type GameWorkerEvent,
} from './runtimeProtocol';
import type { GameRuntimeBundle } from '../types';

interface LifecycleExports {
  memory?: WebAssembly.Memory;
  ng_init?: () => void;
  ng_render?: () => void;
  ng_shutdown?: () => void;
  ng_update?: (deltaMilliseconds: number) => void;
}

interface Scheduler {
  cancel(handle: number): void;
  schedule(callback: () => void, milliseconds: number): number;
}

const ALLOWED_IMPORTS = new Set([
  'audio_tone',
  'input_key_down',
  'input_pointer_down',
  'input_pointer_x',
  'input_pointer_y',
  'render_clear',
  'render_fill_rect',
  'viewport_height',
  'viewport_width',
]);

export class GameWorkerRuntime {
  #commands: CanvasCommand[] = [];
  #consecutiveSlowFrames = 0;
  #exports: LifecycleExports | null = null;
  #frameHandle: number | null = null;
  #hostCalls = 0;
  #input = createInputState();
  #lastFrameAt = 0;
  #now: () => number;
  #post: (event: GameWorkerEvent) => void;
  #running = false;
  #scheduler: Scheduler;
  #viewport = { height: 600, width: 800 };

  constructor({
    now = () => performance.now(),
    post,
    scheduler = defaultScheduler,
  }: {
    now?: () => number;
    post: (event: GameWorkerEvent) => void;
    scheduler?: Scheduler;
  }) {
    this.#now = now;
    this.#post = post;
    this.#scheduler = scheduler;
  }

  async load(bundle: GameRuntimeBundle): Promise<void> {
    if (this.#exports || this.#running) throw new Error('A game is already loaded');
    validateManifestCapabilities(bundle);
    const wasmBytes = Uint8Array.from(bundle.wasmBytes);
    if (wasmBytes.byteLength > 8 * 1024 * 1024) throw new Error('WASM module exceeds 8 MiB');
    const module = await WebAssembly.compile(wasmBytes);
    validateImports(WebAssembly.Module.imports(module));
    const instance = await WebAssembly.instantiate(module, {
      neo_grounds: this.#hostImports(),
    });
    const exports = instance.exports as unknown as LifecycleExports;
    for (const name of ['ng_init', 'ng_update', 'ng_render', 'ng_shutdown'] as const) {
      if (typeof exports[name] !== 'function') throw new Error(`Game is missing ${name}`);
    }
    this.#exports = exports;
    this.#enforceMemoryLimit();
    this.#post({ type: 'ready' });
  }

  start(): void {
    if (!this.#exports) throw new Error('No game is loaded');
    if (this.#running) return;
    this.#exports.ng_init?.();
    this.#running = true;
    this.#lastFrameAt = this.#now();
    this.#post({ type: 'running' });
    this.#scheduleFrame();
  }

  stop(): void {
    if (this.#frameHandle !== null) this.#scheduler.cancel(this.#frameHandle);
    this.#frameHandle = null;
    const wasLoaded = Boolean(this.#exports);
    this.#running = false;
    this.#exports?.ng_shutdown?.();
    this.#commands = [];
    this.#input = createInputState();
    if (wasLoaded) this.#post({ type: 'stopped' });
  }

  resize(width: number, height: number): void {
    if (!Number.isFinite(width) || !Number.isFinite(height) || width < 1 || height < 1) return;
    this.#viewport = { height: Math.min(height, 8192), width: Math.min(width, 8192) };
  }

  input(event: GameInputEvent): void {
    if (event.type === 'keyboard') {
      if (!/^[A-Za-z0-9]{1,32}$/u.test(event.code)) return;
      if (event.action === 'down') this.#input.keys.add(hashInputCode(event.code));
      else this.#input.keys.delete(hashInputCode(event.code));
      return;
    }
    if (!Number.isFinite(event.x) || !Number.isFinite(event.y)) return;
    this.#input.pointerX = event.x;
    this.#input.pointerY = event.y;
    this.#input.pointerDown =
      event.action === 'down' || (event.action === 'move' && this.#input.pointerDown);
    if (event.action === 'up' || event.action === 'cancel') this.#input.pointerDown = false;
  }

  runFrame(): void {
    if (!this.#running || !this.#exports) return;
    const started = this.#now();
    const delta = Math.min(100, Math.max(0, started - this.#lastFrameAt));
    this.#lastFrameAt = started;
    this.#commands = [];
    this.#hostCalls = 0;
    this.#exports.ng_update?.(delta);
    this.#exports.ng_render?.();
    this.#enforceMemoryLimit();
    if (!this.#commands.length) {
      this.#commands.push({ color: 'rgba(0, 0, 0, 1)', op: 'clear' });
    }
    this.#post({ commands: this.#commands, type: 'render' });
    this.#post({ timestamp: this.#now(), type: 'heartbeat' });
    const elapsed = this.#now() - started;
    this.#consecutiveSlowFrames =
      elapsed > GAME_RUNTIME_LIMITS.maximumFrameMilliseconds ? this.#consecutiveSlowFrames + 1 : 0;
    if (this.#consecutiveSlowFrames >= GAME_RUNTIME_LIMITS.maximumConsecutiveSlowFrames) {
      throw new Error('Game exceeded the frame-time limit repeatedly');
    }
  }

  #scheduleFrame(): void {
    if (!this.#running) return;
    this.#frameHandle = this.#scheduler.schedule(() => {
      try {
        this.runFrame();
        this.#scheduleFrame();
      } catch (error) {
        this.#running = false;
        this.#post({
          message: error instanceof Error ? error.message : 'Game runtime failed',
          type: 'error',
        });
      }
    }, 16);
  }

  #hostImports(): Record<string, (...values: number[]) => number | void> {
    return {
      audio_tone: (frequency, durationMilliseconds, volume) => {
        this.#countHostCall();
        if (
          Number.isFinite(frequency) &&
          frequency >= 20 &&
          frequency <= 20_000 &&
          Number.isFinite(durationMilliseconds) &&
          durationMilliseconds > 0 &&
          durationMilliseconds <= 5_000 &&
          Number.isFinite(volume) &&
          volume >= 0 &&
          volume <= 1
        ) {
          this.#post({ durationMilliseconds, frequency, type: 'audio-tone', volume });
        }
      },
      input_key_down: (code) => {
        this.#countHostCall();
        return this.#input.keys.has(code >>> 0) ? 1 : 0;
      },
      input_pointer_down: () => {
        this.#countHostCall();
        return this.#input.pointerDown ? 1 : 0;
      },
      input_pointer_x: () => {
        this.#countHostCall();
        return this.#input.pointerX;
      },
      input_pointer_y: () => {
        this.#countHostCall();
        return this.#input.pointerY;
      },
      render_clear: (red, green, blue, alpha) => {
        this.#countHostCall();
        this.#pushCommand({ color: rgba(red, green, blue, alpha), op: 'clear' });
      },
      render_fill_rect: (x, y, width, height, red, green, blue, alpha) => {
        this.#countHostCall();
        if ([x, y, width, height].every(validCoordinate) && width >= 0 && height >= 0) {
          this.#pushCommand({
            color: rgba(red, green, blue, alpha),
            height,
            op: 'fill-rect',
            width,
            x,
            y,
          });
        }
      },
      viewport_height: () => {
        this.#countHostCall();
        return this.#viewport.height;
      },
      viewport_width: () => {
        this.#countHostCall();
        return this.#viewport.width;
      },
    };
  }

  #countHostCall(): void {
    this.#hostCalls += 1;
    assertHostCallCount(this.#hostCalls);
  }

  #pushCommand(command: CanvasCommand): void {
    assertRenderCommandCount(this.#commands.length + 1);
    this.#commands.push(command);
  }

  #enforceMemoryLimit(): void {
    const memory = this.#exports?.memory;
    if (memory) assertMemoryByteLength(memory.buffer.byteLength);
  }
}

export function validateImports(imports: WebAssembly.ModuleImportDescriptor[]): void {
  for (const item of imports) {
    if (
      item.module !== 'neo_grounds' ||
      item.kind !== 'function' ||
      !ALLOWED_IMPORTS.has(item.name)
    ) {
      throw new Error(`Unsupported WASM import: ${item.module}.${item.name}`);
    }
  }
}

export function assertMemoryByteLength(byteLength: number): void {
  if (byteLength > GAME_RUNTIME_LIMITS.maximumMemoryBytes) {
    throw new Error('Game exceeded the 64 MiB memory limit');
  }
}

export function assertHostCallCount(count: number): void {
  if (count > GAME_RUNTIME_LIMITS.maximumHostCallsPerFrame) {
    throw new Error('Game exceeded the host-call rate limit');
  }
}

export function assertRenderCommandCount(count: number): void {
  if (count > GAME_RUNTIME_LIMITS.maximumRenderCommandsPerFrame) {
    throw new Error('Game exceeded the render command limit');
  }
}

export function hashInputCode(value: string): number {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function validateManifestCapabilities(bundle: GameRuntimeBundle): void {
  const compatibility = bundle.manifest.compatibility;
  if (
    compatibility.schemaVersion !== 1 ||
    compatibility.packageFormat !== 'neo-grounds-runtime-package' ||
    compatibility.runtimeAbi !== 'neo-grounds-wasm-component-v1' ||
    compatibility.renderer !== 'canvas2d-command-buffer' ||
    compatibility.hostIntegration !== 'worker-host-canvas-proxy'
  ) {
    throw new Error('Game runtime compatibility is unsupported');
  }
  const unsupported = bundle.manifest.platformApi.permissions.filter(
    (permission) => permission !== 'save_data',
  );
  if (unsupported.includes('multiplayer_signals')) {
    throw new Error('Harbor multiplayer is not supported by this runtime');
  }
  if (unsupported.length)
    throw new Error(`Unsupported game permissions: ${unsupported.join(', ')}`);
}

function validCoordinate(value: number): boolean {
  return Number.isFinite(value) && Math.abs(value) <= 32_768;
}

function rgba(red: number, green: number, blue: number, alpha: number): string {
  const channel = (value: number) => Math.max(0, Math.min(255, Math.round(value)));
  const opacity = Math.max(0, Math.min(1, Number.isFinite(alpha) ? alpha : 1));
  return `rgba(${channel(red)}, ${channel(green)}, ${channel(blue)}, ${opacity})`;
}

function createInputState() {
  return { keys: new Set<number>(), pointerDown: false, pointerX: 0, pointerY: 0 };
}

const defaultScheduler: Scheduler = {
  cancel: (handle) => globalThis.clearTimeout(handle),
  schedule: (callback, milliseconds) => globalThis.setTimeout(callback, milliseconds),
};
