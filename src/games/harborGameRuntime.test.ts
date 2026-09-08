import { fireEvent } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import {
  attachGameInput,
  createHarborGameRuntime,
  renderCanvasCommands,
  type GameWorkerPort,
} from './harborGameRuntime';
import type { GameWorkerCommand, GameWorkerEvent } from './runtimeProtocol';
import type { GameRuntimeBundle } from '../types';

const bundle: GameRuntimeBundle = {
  assetManifest: {},
  assets: {},
  gameId: 'game-1',
  manifest: {
    compatibility: {
      hostIntegration: 'worker-host-canvas-proxy',
      packageFormat: 'neo-grounds-runtime-package',
      renderer: 'canvas2d-command-buffer',
      runtimeAbi: 'neo-grounds-wasm-component-v1',
      schemaVersion: 1,
    },
    metadata: { gameId: 'game-1', title: 'Game', versionId: 'version-1' },
    platformApi: { bindingMode: 'declared-permissions-only', permissions: [] },
    wasmModule: { exports: [], imports: [] },
  },
  versionId: 'version-1',
  wasmBytes: [0, 97, 115, 109],
};

function fakeWorker() {
  const messages: GameWorkerCommand[] = [];
  const listeners = new Map<string, Set<(event: never) => void>>();
  const worker = {
    addEventListener(type: string, listener: (event: never) => void) {
      const registered = listeners.get(type) ?? new Set();
      registered.add(listener);
      listeners.set(type, registered);
    },
    removeEventListener(type: string, listener: (event: never) => void) {
      listeners.get(type)?.delete(listener);
    },
    postMessage: vi.fn((message: GameWorkerCommand) => messages.push(message)),
    terminate: vi.fn(),
  };
  return {
    emit(event: GameWorkerEvent) {
      for (const listener of listeners.get('message') ?? []) {
        listener({ data: event } as never);
      }
    },
    messages,
    worker: worker as unknown as GameWorkerPort,
  };
}

describe('Harbor game runtime host', () => {
  it('loads only a local bundle, renders Canvas commands, records launch, and cleans up', async () => {
    const runtimeWorker = fakeWorker();
    const fillRect = vi.fn();
    const context = { fillRect, fillStyle: '' } as unknown as CanvasRenderingContext2D;
    const canvas = document.createElement('canvas');
    canvas.getContext = vi.fn(() => context) as never;
    const audio = { close: vi.fn(), playTone: vi.fn() };
    const recordLaunch = vi.fn().mockResolvedValue(undefined);
    const loadBundle = vi.fn().mockResolvedValue(bundle);
    const clearInterval = vi.fn();
    const controller = await createHarborGameRuntime({
      audio,
      canvas,
      clearScheduledInterval: clearInterval,
      createWorker: () => runtimeWorker.worker,
      gameId: 'game-1',
      loadBundle,
      recordLaunch,
      scheduleInterval: vi.fn(() => 1 as never),
    });
    expect(loadBundle).toHaveBeenCalledWith('game-1');
    expect(runtimeWorker.messages[0]).toEqual({ bundle, type: 'load' });
    runtimeWorker.emit({ type: 'ready' });
    controller.start();
    expect(runtimeWorker.messages.at(-1)).toEqual({ type: 'start' });
    runtimeWorker.emit({ type: 'running' });
    expect(recordLaunch).toHaveBeenCalledWith('game-1');
    runtimeWorker.emit({
      commands: [
        { color: 'rgba(0, 0, 0, 1)', op: 'clear' },
        { color: 'rgba(255, 0, 0, 1)', height: 20, op: 'fill-rect', width: 10, x: 1, y: 2 },
      ],
      type: 'render',
    });
    expect(fillRect).toHaveBeenCalledTimes(2);
    runtimeWorker.emit({
      durationMilliseconds: 100,
      frequency: 440,
      type: 'audio-tone',
      volume: 0.5,
    });
    expect(audio.playTone).toHaveBeenCalledWith(440, 100, 0.5);
    controller.destroy();
    expect(runtimeWorker.worker.terminate).toHaveBeenCalled();
    expect(audio.close).toHaveBeenCalled();
    expect(clearInterval).toHaveBeenCalled();
  });

  it('terminates a stalled Worker without leaving it active', async () => {
    const runtimeWorker = fakeWorker();
    const canvas = document.createElement('canvas');
    canvas.getContext = vi.fn(() => ({ fillRect: vi.fn(), fillStyle: '' })) as never;
    let timestamp = 0;
    let watchdog: (() => void) | undefined;
    const onError = vi.fn();
    const controller = await createHarborGameRuntime({
      audio: { close: vi.fn(), playTone: vi.fn() },
      canvas,
      createWorker: () => runtimeWorker.worker,
      gameId: 'game-1',
      loadBundle: vi.fn().mockResolvedValue(bundle),
      now: () => timestamp,
      onError,
      recordLaunch: vi.fn().mockResolvedValue(undefined),
      scheduleInterval: (callback) => {
        watchdog = callback;
        return 1 as never;
      },
    });
    runtimeWorker.emit({ type: 'ready' });
    controller.start();
    runtimeWorker.emit({ type: 'running' });
    timestamp = 2_001;
    watchdog?.();
    expect(controller.getState()).toBe('error');
    expect(runtimeWorker.worker.terminate).toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith('Game stopped responding and was terminated');
    controller.destroy();
  });

  it('normalizes keyboard and pointer input and detaches every listener', () => {
    const canvas = document.createElement('canvas');
    canvas.tabIndex = 0;
    canvas.width = 800;
    canvas.height = 600;
    canvas.getBoundingClientRect = () =>
      ({
        bottom: 300,
        height: 300,
        left: 0,
        right: 400,
        toJSON: vi.fn(),
        top: 0,
        width: 400,
        x: 0,
        y: 0,
      }) as DOMRect;
    document.body.append(canvas);
    const send = vi.fn();
    const detach = attachGameInput(canvas, send);
    canvas.focus();
    fireEvent.keyDown(document, { code: 'Space' });
    fireEvent(
      canvas,
      new MouseEvent('pointerdown', { bubbles: true, button: 0, clientX: 200, clientY: 150 }),
    );
    expect(send).toHaveBeenCalledWith({ action: 'down', code: 'Space', type: 'keyboard' });
    expect(send).toHaveBeenCalledWith({
      action: 'down',
      button: 0,
      type: 'pointer',
      x: 400,
      y: 300,
    });
    detach();
    send.mockClear();
    fireEvent.keyDown(document, { code: 'Space' });
    expect(send).not.toHaveBeenCalled();
    canvas.remove();
  });

  it('rejects invalid Canvas command batches at the host boundary', () => {
    const context = { fillRect: vi.fn(), fillStyle: '' } as unknown as CanvasRenderingContext2D;
    expect(() =>
      renderCanvasCommands(context, 800, 600, [
        { color: 'url(https://attacker.invalid)', op: 'clear' },
      ]),
    ).toThrow(/invalid render color/iu);
    expect(() =>
      renderCanvasCommands(
        context,
        800,
        600,
        Array.from({ length: 4097 }, () => ({
          color: 'rgba(0, 0, 0, 1)' as const,
          op: 'clear' as const,
        })),
      ),
    ).toThrow(/batch exceeds/iu);
  });
});
