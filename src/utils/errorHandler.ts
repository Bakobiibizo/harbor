import { invoke } from "@tauri-apps/api/core";
import toast from "react-hot-toast";
import { HarborError, isErrorResponse, getErrorMessage } from "./errors";
import { createLogger } from "./logger";

const logger = createLogger("errorHandler");

export interface InvokeOptions {
  showToast?: boolean;
  toastDuration?: number;
  retryCount?: number;
  retryDelay?: number;
}

const defaultOptions: InvokeOptions = {
  showToast: true,
  toastDuration: 4000,
  retryCount: 0,
  retryDelay: 1000,
};

async function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export async function safeInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
  options?: InvokeOptions
): Promise<T> {
  const opts = { ...defaultOptions, ...options };

  for (let attempt = 0; attempt <= opts.retryCount; attempt++) {
    try {
      if (attempt > 0) {
        logger.info(`Retrying ${command} (attempt ${attempt + 1})`);
        await delay(opts.retryDelay ?? 1000);
      }

      const result = await invoke<T>(command, args);
      return result;
    } catch (error) {
      logger.error(`Command ${command} failed`, error);

      const harborError = HarborError.fromUnknown(error);

      if (!harborError.isRecoverable() || attempt >= opts.retryCount) {
        if (opts.showToast) {
          showErrorToast(harborError);
        }
        throw harborError;
      }
    }
  }

  // All paths in the loop either return or throw; this is unreachable.
  // Added only to satisfy TypeScript's control flow analysis, if needed.
  throw new Error("safeInvoke reached an unreachable state after retries");
}

export function showErrorToast(error: unknown): void {
  const harborError =
    error instanceof HarborError ? error : HarborError.fromUnknown(error);

  const message = harborError.message;
  const recovery = harborError.recovery;

  const toastMessage = recovery ? `${message}\n${recovery}` : message;

  toast.error(toastMessage, {
    duration: harborError.isCritical() ? 6000 : 4000,
  });
}

export function showSuccessToast(message: string): void {
  toast.success(message, { duration: 3000 });
}

export function handleError(error: unknown, context?: string): HarborError {
  const harborError = HarborError.fromUnknown(error);

  if (context) {
    logger.error(`${context}: ${harborError.message}`, harborError);
  } else {
    logger.error(harborError.message, harborError);
  }

  return harborError;
}

export { HarborError, isErrorResponse, getErrorMessage };
