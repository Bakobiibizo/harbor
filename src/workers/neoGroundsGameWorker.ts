/// <reference lib="webworker" />

import { GameWorkerRuntime } from '../games/gameWorkerRuntime';
import type { GameWorkerCommand, GameWorkerEvent } from '../games/runtimeProtocol';

const worker = self as unknown as DedicatedWorkerGlobalScope;
const runtime = new GameWorkerRuntime({
  post: (event: GameWorkerEvent) => worker.postMessage(event),
});

worker.addEventListener('message', (event: MessageEvent<GameWorkerCommand>) => {
  void handleCommand(event.data).catch((error) => {
    runtime.stop();
    worker.postMessage({
      message: error instanceof Error ? error.message : 'Game runtime failed',
      type: 'error',
    } satisfies GameWorkerEvent);
  });
});

async function handleCommand(command: GameWorkerCommand): Promise<void> {
  switch (command.type) {
    case 'load':
      await runtime.load(command.bundle);
      return;
    case 'start':
      runtime.start();
      return;
    case 'stop':
      runtime.stop();
      return;
    case 'resize':
      runtime.resize(command.width, command.height);
      return;
    case 'input':
      runtime.input(command.event);
  }
}
