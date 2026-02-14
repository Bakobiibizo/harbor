import toast from 'react-hot-toast';
import { createLogger } from './logger';
import { getErrorMessage } from './errors';

const logger = createLogger('global');

/**
 * Known error patterns that are noisy but non-actionable for users.
 * These are still logged but do not trigger toast notifications.
 */
const SUPPRESSED_TOAST_PATTERNS = [
  'ResizeObserver loop',
  'Loading chunk',
  'dynamically imported module',
];

function shouldSuppressToast(message: string): boolean {
  return SUPPRESSED_TOAST_PATTERNS.some((pattern) =>
    message.toLowerCase().includes(pattern.toLowerCase()),
  );
}

/**
 * Installs global handlers for uncaught errors and unhandled promise rejections.
 *
 * - All errors are logged via the Harbor logger.
 * - User-facing toast notifications are shown unless the error matches a
 *   known non-actionable pattern.
 *
 * Call this once, early in the application bootstrap (before React renders).
 */
export function installGlobalErrorHandlers(): void {
  // --- Unhandled promise rejections ---
  window.addEventListener('unhandledrejection', (event: PromiseRejectionEvent) => {
    const message = getErrorMessage(event.reason);

    logger.error('Unhandled promise rejection', event.reason);

    if (!shouldSuppressToast(message)) {
      toast.error(`Unexpected error: ${message}`, { duration: 5000 });
    }

    // Prevent the default browser handling (console error with "Uncaught (in promise)")
    // since we already logged it above.
    event.preventDefault();
  });

  // --- Uncaught synchronous errors ---
  window.addEventListener('error', (event: ErrorEvent) => {
    const message = event.message || 'Unknown error';

    logger.error(`Uncaught error: ${message}`, {
      filename: event.filename,
      lineno: event.lineno,
      colno: event.colno,
      error: event.error,
    });

    if (!shouldSuppressToast(message)) {
      toast.error(`Unexpected error: ${message}`, { duration: 5000 });
    }
  });

  logger.info('Global error handlers installed');
}
