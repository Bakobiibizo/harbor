import type { GameRuntimeBundle } from '../types';

export const GAME_RUNTIME_LIMITS = {
  maximumConsecutiveSlowFrames: 5,
  maximumFrameMilliseconds: 50,
  maximumHostCallsPerFrame: 8192,
  maximumMemoryBytes: 64 * 1024 * 1024,
  maximumRenderCommandsPerFrame: 4096,
  watchdogMilliseconds: 2000,
} as const;

export type GameInputEvent =
  | { type: 'keyboard'; action: 'down' | 'up'; code: string }
  | {
      type: 'pointer';
      action: 'down' | 'move' | 'up' | 'cancel';
      button: number;
      x: number;
      y: number;
    };

export type CanvasCommand =
  | { op: 'clear'; color: string }
  | {
      op: 'fill-rect';
      color: string;
      height: number;
      width: number;
      x: number;
      y: number;
    };

export type GameWorkerCommand =
  | { type: 'load'; bundle: GameRuntimeBundle }
  | { type: 'start' }
  | { type: 'stop' }
  | { type: 'resize'; width: number; height: number }
  | { type: 'input'; event: GameInputEvent };

export type GameWorkerEvent =
  | { type: 'ready' }
  | { type: 'running' }
  | { type: 'stopped' }
  | { type: 'heartbeat'; timestamp: number }
  | { type: 'render'; commands: CanvasCommand[] }
  | { type: 'audio-tone'; durationMilliseconds: number; frequency: number; volume: number }
  | { type: 'error'; message: string };
