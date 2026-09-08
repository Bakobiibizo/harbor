import { loadGameRuntime, recordGameLaunch } from '../services/games';
import {
  GAME_RUNTIME_LIMITS,
  type CanvasCommand,
  type GameInputEvent,
  type GameWorkerCommand,
  type GameWorkerEvent,
} from './runtimeProtocol';

export type HarborGameRuntimeState = 'loading' | 'ready' | 'running' | 'stopped' | 'error';

export interface GameWorkerPort {
  addEventListener(type: 'message', listener: (event: MessageEvent<GameWorkerEvent>) => void): void;
  addEventListener(type: 'error', listener: (event: ErrorEvent) => void): void;
  removeEventListener(
    type: 'message',
    listener: (event: MessageEvent<GameWorkerEvent>) => void,
  ): void;
  removeEventListener(type: 'error', listener: (event: ErrorEvent) => void): void;
  postMessage(command: GameWorkerCommand): void;
  terminate(): void;
}

export interface GameAudioBoundary {
  playTone(frequency: number, durationMilliseconds: number, volume: number): void;
  close(): void;
}

export interface HarborGameRuntimeController {
  destroy(): void;
  getState(): HarborGameRuntimeState;
  resize(width: number, height: number): void;
  sendInput(event: GameInputEvent): void;
  start(): void;
  stop(): void;
}

export async function createHarborGameRuntime({
  audio = createWebAudioBoundary(),
  canvas,
  createWorker = defaultWorkerFactory,
  gameId,
  loadBundle = loadGameRuntime,
  now = () => performance.now(),
  onError,
  onStateChange,
  recordLaunch = recordGameLaunch,
  scheduleInterval = (callback, milliseconds) => globalThis.setInterval(callback, milliseconds),
  clearScheduledInterval = (handle) => globalThis.clearInterval(handle),
}: {
  audio?: GameAudioBoundary;
  canvas: HTMLCanvasElement;
  createWorker?: () => GameWorkerPort;
  gameId: string;
  loadBundle?: typeof loadGameRuntime;
  now?: () => number;
  onError?: (message: string) => void;
  onStateChange?: (state: HarborGameRuntimeState) => void;
  recordLaunch?: typeof recordGameLaunch;
  scheduleInterval?: (callback: () => void, milliseconds: number) => ReturnType<typeof setInterval>;
  clearScheduledInterval?: (handle: ReturnType<typeof setInterval>) => void;
}): Promise<HarborGameRuntimeController> {
  let state: HarborGameRuntimeState = 'loading';
  let destroyed = false;
  let lastHeartbeat = now();
  const context = canvas.getContext('2d');
  if (!context) throw new Error('Canvas 2D is unavailable');
  onStateChange?.(state);
  const bundle = await loadBundle(gameId);
  const worker = createWorker();

  const setState = (next: HarborGameRuntimeState) => {
    state = next;
    onStateChange?.(next);
  };
  const fail = (message: string) => {
    if (destroyed || state === 'error') return;
    setState('error');
    worker.terminate();
    audio.close();
    onError?.(message);
  };
  const onMessage = (event: MessageEvent<GameWorkerEvent>) => {
    if (destroyed) return;
    switch (event.data.type) {
      case 'ready':
        setState('ready');
        break;
      case 'running':
        lastHeartbeat = now();
        setState('running');
        void recordLaunch(gameId).catch((error) => {
          fail(error instanceof Error ? error.message : 'Could not record game launch');
        });
        break;
      case 'stopped':
        setState('stopped');
        break;
      case 'heartbeat':
        lastHeartbeat = now();
        break;
      case 'render':
        try {
          renderCanvasCommands(context, canvas.width, canvas.height, event.data.commands);
        } catch (error) {
          fail(error instanceof Error ? error.message : 'Game rendering failed');
        }
        break;
      case 'audio-tone':
        audio.playTone(event.data.frequency, event.data.durationMilliseconds, event.data.volume);
        break;
      case 'error':
        fail(event.data.message);
    }
  };
  const onWorkerError = (event: ErrorEvent) => {
    fail(event.message || 'Game Worker crashed');
  };
  worker.addEventListener('message', onMessage);
  worker.addEventListener('error', onWorkerError);
  worker.postMessage({ bundle, type: 'load' });
  const watchdog = scheduleInterval(() => {
    if (
      (state === 'loading' || state === 'running') &&
      now() - lastHeartbeat > GAME_RUNTIME_LIMITS.watchdogMilliseconds
    ) {
      fail('Game stopped responding and was terminated');
    }
  }, 250);

  return {
    destroy() {
      if (destroyed) return;
      destroyed = true;
      clearScheduledInterval(watchdog);
      worker.removeEventListener('message', onMessage);
      worker.removeEventListener('error', onWorkerError);
      worker.postMessage({ type: 'stop' });
      worker.terminate();
      audio.close();
      state = 'stopped';
      onStateChange?.('stopped');
    },
    getState: () => state,
    resize(width, height) {
      if (destroyed || !validDimension(width) || !validDimension(height)) return;
      canvas.width = Math.floor(width);
      canvas.height = Math.floor(height);
      worker.postMessage({ height: canvas.height, type: 'resize', width: canvas.width });
    },
    sendInput(event) {
      if (!destroyed && state === 'running') worker.postMessage({ event, type: 'input' });
    },
    start() {
      if (!destroyed && (state === 'ready' || state === 'stopped')) {
        worker.postMessage({ type: 'start' });
      }
    },
    stop() {
      if (!destroyed && state === 'running') worker.postMessage({ type: 'stop' });
    },
  };
}

export function renderCanvasCommands(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  commands: CanvasCommand[],
): void {
  if (commands.length > GAME_RUNTIME_LIMITS.maximumRenderCommandsPerFrame) {
    throw new Error('Render command batch exceeds the runtime limit');
  }
  for (const command of commands) {
    if (!validColor(command.color)) throw new Error('Game emitted an invalid render color');
    context.fillStyle = command.color;
    if (command.op === 'clear') {
      context.fillRect(0, 0, width, height);
    } else if (
      [command.x, command.y, command.width, command.height].every(
        (value) => Number.isFinite(value) && Math.abs(value) <= 32_768,
      ) &&
      command.width >= 0 &&
      command.height >= 0
    ) {
      context.fillRect(command.x, command.y, command.width, command.height);
    } else {
      throw new Error('Game emitted invalid rectangle coordinates');
    }
  }
}

export function attachGameInput(
  canvas: HTMLCanvasElement,
  send: (event: GameInputEvent) => void,
): () => void {
  const keyboardDown = (event: KeyboardEvent) => {
    if (document.activeElement !== canvas || event.repeat) return;
    send({ action: 'down', code: event.code, type: 'keyboard' });
    if (gameplayKey(event.code)) event.preventDefault();
  };
  const keyboardUp = (event: KeyboardEvent) => {
    if (document.activeElement !== canvas) return;
    send({ action: 'up', code: event.code, type: 'keyboard' });
  };
  const pointer = (action: 'down' | 'move' | 'up' | 'cancel') => (event: PointerEvent) => {
    const bounds = canvas.getBoundingClientRect();
    if (action === 'down') canvas.focus();
    send({
      action,
      button: event.button,
      type: 'pointer',
      x: ((event.clientX - bounds.left) / Math.max(1, bounds.width)) * canvas.width,
      y: ((event.clientY - bounds.top) / Math.max(1, bounds.height)) * canvas.height,
    });
  };
  const pointerDown = pointer('down');
  const pointerMove = pointer('move');
  const pointerUp = pointer('up');
  const pointerCancel = pointer('cancel');
  document.addEventListener('keydown', keyboardDown);
  document.addEventListener('keyup', keyboardUp);
  canvas.addEventListener('pointerdown', pointerDown);
  canvas.addEventListener('pointermove', pointerMove);
  canvas.addEventListener('pointerup', pointerUp);
  canvas.addEventListener('pointercancel', pointerCancel);
  return () => {
    document.removeEventListener('keydown', keyboardDown);
    document.removeEventListener('keyup', keyboardUp);
    canvas.removeEventListener('pointerdown', pointerDown);
    canvas.removeEventListener('pointermove', pointerMove);
    canvas.removeEventListener('pointerup', pointerUp);
    canvas.removeEventListener('pointercancel', pointerCancel);
  };
}

export function createWebAudioBoundary(): GameAudioBoundary {
  let context: AudioContext | null = null;
  return {
    close() {
      if (context) void context.close();
      context = null;
    },
    playTone(frequency, durationMilliseconds, volume) {
      if (
        !Number.isFinite(frequency) ||
        frequency < 20 ||
        frequency > 20_000 ||
        !Number.isFinite(durationMilliseconds) ||
        durationMilliseconds <= 0 ||
        durationMilliseconds > 5_000 ||
        !Number.isFinite(volume) ||
        volume < 0 ||
        volume > 1
      ) {
        return;
      }
      context ??= new AudioContext();
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      oscillator.frequency.value = frequency;
      gain.gain.value = volume;
      oscillator.connect(gain);
      gain.connect(context.destination);
      oscillator.start();
      oscillator.stop(context.currentTime + durationMilliseconds / 1_000);
    },
  };
}

function defaultWorkerFactory(): GameWorkerPort {
  return new Worker(new URL('../workers/neoGroundsGameWorker.ts', import.meta.url), {
    name: 'harbor-neo-grounds-game',
    type: 'module',
  });
}

function validColor(value: string): boolean {
  return /^rgba\(\d{1,3}, \d{1,3}, \d{1,3}, (?:0|1|0\.\d+)\)$/u.test(value);
}

function validDimension(value: number): boolean {
  return Number.isFinite(value) && value >= 1 && value <= 8192;
}

function gameplayKey(code: string): boolean {
  return ['ArrowDown', 'ArrowLeft', 'ArrowRight', 'ArrowUp', 'Space'].includes(code);
}
