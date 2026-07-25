import toast from 'react-hot-toast';
import { HarborError, isErrorResponse, getErrorMessage } from './errors';
import { createLogger } from './logger';

const logger = createLogger('errorHandler');

export function showErrorToast(error: unknown): void {
  const harborError = error instanceof HarborError ? error : HarborError.fromUnknown(error);

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
