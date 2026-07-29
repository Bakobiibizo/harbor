import { describe, expect, it, vi } from 'vitest';
import { createAsyncDisposerScope, registerAtomicResources } from './asyncDisposer';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe('async disposer lifecycle', () => {
  it('tears down a partially registered group exactly once when registration fails', async () => {
    const disposeFirst = vi.fn();
    const disposeThird = vi.fn();
    const failure = new Error('second listener unavailable');
    const reportError = vi.fn();
    const scope = createAsyncDisposerScope(reportError);

    const registered = await registerAtomicResources(
      scope,
      [
        async () => disposeFirst,
        async () => {
          throw failure;
        },
        async () => disposeThird,
      ],
      reportError,
    );

    expect(registered).toBe(false);
    expect(scope.disposed).toBe(true);
    expect(disposeFirst).toHaveBeenCalledTimes(1);
    expect(disposeThird).not.toHaveBeenCalled();
    expect(reportError).toHaveBeenCalledWith(failure);

    scope.dispose();
    expect(disposeFirst).toHaveBeenCalledTimes(1);
  });

  it('immediately disposes a listener that finishes registering after teardown', async () => {
    const registration = deferred<() => void>();
    const dispose = vi.fn();
    const reportError = vi.fn();
    const scope = createAsyncDisposerScope(reportError);
    const setup = registerAtomicResources(scope, [() => registration.promise], reportError);

    scope.dispose();
    registration.resolve(dispose);

    await expect(setup).resolves.toBe(false);
    expect(dispose).toHaveBeenCalledTimes(1);
    scope.dispose();
    expect(dispose).toHaveBeenCalledTimes(1);
  });

  it('keeps delayed setup from an old mount separate from a replacement mount', async () => {
    const oldRegistration = deferred<() => void>();
    const disposeOld = vi.fn();
    const disposeCurrent = vi.fn();
    const reportError = vi.fn();
    const oldScope = createAsyncDisposerScope(reportError);
    const currentScope = createAsyncDisposerScope(reportError);
    const oldSetup = registerAtomicResources(
      oldScope,
      [() => oldRegistration.promise],
      reportError,
    );

    oldScope.dispose();
    await expect(
      registerAtomicResources(currentScope, [async () => disposeCurrent], reportError),
    ).resolves.toBe(true);
    oldRegistration.resolve(disposeOld);
    await expect(oldSetup).resolves.toBe(false);

    expect(disposeOld).toHaveBeenCalledTimes(1);
    expect(disposeCurrent).not.toHaveBeenCalled();
    currentScope.dispose();
    currentScope.dispose();
    expect(disposeCurrent).toHaveBeenCalledTimes(1);
  });

  it('captures asynchronous cleanup rejection instead of leaking it', async () => {
    const cleanupFailure = new Error('unregister failed');
    const reportError = vi.fn();
    const scope = createAsyncDisposerScope(reportError);
    scope.add(async () => {
      throw cleanupFailure;
    });

    scope.dispose();
    await Promise.resolve();

    expect(reportError).toHaveBeenCalledWith(cleanupFailure);
  });
});
