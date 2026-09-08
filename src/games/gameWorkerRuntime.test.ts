import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import {
  assertHostCallCount,
  assertMemoryByteLength,
  assertRenderCommandCount,
  GameWorkerRuntime,
  hashInputCode,
  validateImports,
} from './gameWorkerRuntime';
import type { GameRuntimeBundle } from '../types';
import type { GameWorkerEvent } from './runtimeProtocol';

async function goldenBundle(permissions: string[] = ['save_data']): Promise<GameRuntimeBundle> {
  return {
    assetManifest: { assets: [] },
    assets: {},
    gameId: 'game_golden_runner',
    manifest: {
      compatibility: {
        hostIntegration: 'worker-host-canvas-proxy',
        packageFormat: 'neo-grounds-runtime-package',
        renderer: 'canvas2d-command-buffer',
        runtimeAbi: 'neo-grounds-wasm-component-v1',
        schemaVersion: 1,
      },
      metadata: {
        gameId: 'game_golden_runner',
        title: 'Golden Runner',
        versionId: 'version_golden_1',
      },
      platformApi: { bindingMode: 'declared-permissions-only', permissions },
      wasmModule: {
        exports: ['ng_init', 'ng_update', 'ng_render', 'ng_shutdown'],
        imports: [],
      },
    },
    versionId: 'version_golden_1',
    wasmBytes: [
      ...(await readFile(resolve('src-tauri/tests/fixtures/harbor-game-v1/minimal-game.wasm'))),
    ],
  };
}

const scheduler = {
  cancel: vi.fn(),
  schedule: vi.fn(() => 1),
};

describe('GameWorkerRuntime', () => {
  it('runs the real shared WASM lifecycle, input state, stop, and offline restart', async () => {
    const events: GameWorkerEvent[] = [];
    const runtime = new GameWorkerRuntime({ post: (event) => events.push(event), scheduler });
    await runtime.load(await goldenBundle());
    runtime.resize(1024, 768);
    runtime.input({ action: 'down', code: 'Space', type: 'keyboard' });
    expect(hashInputCode('Space')).toBe(3250860581);
    runtime.start();
    runtime.runFrame();
    runtime.stop();
    runtime.start();
    runtime.runFrame();
    expect(events.map((event) => event.type)).toEqual([
      'ready',
      'running',
      'render',
      'heartbeat',
      'stopped',
      'running',
      'render',
      'heartbeat',
    ]);
  });

  it('rejects unsupported imports and multiplayer before launch', async () => {
    expect(() =>
      validateImports([{ kind: 'function', module: 'wasi_snapshot_preview1', name: 'path_open' }]),
    ).toThrow(/unsupported WASM import/iu);
    expect(() =>
      validateImports([{ kind: 'memory', module: 'neo_grounds', name: 'memory' }]),
    ).toThrow(/unsupported WASM import/iu);
    const runtime = new GameWorkerRuntime({ post: vi.fn(), scheduler });
    await expect(runtime.load(await goldenBundle(['multiplayer_signals']))).rejects.toThrow(
      /multiplayer is not supported/iu,
    );
  });

  it('enforces memory, host-call, and render-command bounds', () => {
    expect(() => assertMemoryByteLength(64 * 1024 * 1024 + 1)).toThrow(/memory limit/iu);
    expect(() => assertHostCallCount(8193)).toThrow(/host-call rate/iu);
    expect(() => assertRenderCommandCount(4097)).toThrow(/render command/iu);
  });

  it('contains no Tauri, document, storage, filesystem, or network bridge', async () => {
    const source = await readFile(resolve('src/workers/neoGroundsGameWorker.ts'), 'utf8');
    expect(source).not.toMatch(
      /@tauri|document|localStorage|sessionStorage|fetch\(|XMLHttpRequest/iu,
    );
  });

  it('fails after five consecutive frames over the measured budget', async () => {
    let timestamp = 0;
    const runtime = new GameWorkerRuntime({
      now: () => (timestamp += 60),
      post: vi.fn(),
      scheduler,
    });
    await runtime.load(await goldenBundle());
    runtime.start();
    for (let frame = 0; frame < 4; frame += 1) runtime.runFrame();
    expect(() => runtime.runFrame()).toThrow(/frame-time limit/iu);
  });
});
