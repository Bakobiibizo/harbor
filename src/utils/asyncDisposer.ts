export type AsyncDisposer = () => void | Promise<void>;

export interface AsyncDisposerScope {
  readonly disposed: boolean;
  add(disposer: AsyncDisposer): boolean;
  dispose(): void;
}

type ErrorReporter = (error: unknown) => void;

function reportSafely(reportError: ErrorReporter, error: unknown): void {
  try {
    reportError(error);
  } catch {
    // Resource cleanup must never create another unhandled error.
  }
}

function disposeSafely(disposer: AsyncDisposer, reportError: ErrorReporter): void {
  try {
    void Promise.resolve(disposer()).catch((error) => reportSafely(reportError, error));
  } catch (error) {
    reportSafely(reportError, error);
  }
}

/**
 * Owns resources created by asynchronous setup. A resource that arrives after
 * the scope has closed is disposed immediately instead of being leaked.
 */
export function createAsyncDisposerScope(reportError: ErrorReporter): AsyncDisposerScope {
  let disposed = false;
  const disposers: AsyncDisposer[] = [];

  return {
    get disposed() {
      return disposed;
    },
    add(disposer) {
      if (disposed) {
        disposeSafely(disposer, reportError);
        return false;
      }
      disposers.push(disposer);
      return true;
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      const activeDisposers = disposers.splice(0);
      for (const disposer of activeDisposers.reverse()) {
        disposeSafely(disposer, reportError);
      }
    },
  };
}

/**
 * Registers a group as one lifecycle unit. A failure tears down every listener
 * already installed, while cancellation disposes a late listener immediately.
 * Registration and cleanup failures are reported and never escape unhandled.
 */
export async function registerAtomicResources(
  scope: AsyncDisposerScope,
  registrations: ReadonlyArray<() => Promise<AsyncDisposer>>,
  reportError: ErrorReporter,
): Promise<boolean> {
  try {
    for (const register of registrations) {
      if (scope.disposed) return false;
      const disposer = await register();
      if (!scope.add(disposer)) return false;
    }
    return !scope.disposed;
  } catch (error) {
    scope.dispose();
    reportSafely(reportError, error);
    return false;
  }
}
