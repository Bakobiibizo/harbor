export type KnownErrorCode =
  | 'DATABASE_ERROR'
  | 'DATABASE_CONNECTION'
  | 'DATABASE_MIGRATION'
  | 'CRYPTO_ERROR'
  | 'CRYPTO_KEY_GENERATION'
  | 'CRYPTO_ENCRYPTION'
  | 'CRYPTO_DECRYPTION'
  | 'IDENTITY_ERROR'
  | 'IDENTITY_NOT_FOUND'
  | 'IDENTITY_LOCKED'
  | 'IDENTITY_INVALID_PASSPHRASE'
  | 'SERIALIZATION_ERROR'
  | 'IO_ERROR'
  | 'INVALID_DATA'
  | 'NOT_FOUND'
  | 'ALREADY_EXISTS'
  | 'PERMISSION_DENIED'
  | 'UNAUTHORIZED'
  | 'VALIDATION_ERROR'
  | 'NETWORK_ERROR'
  | 'NETWORK_CONNECTION_FAILED'
  | 'NETWORK_NOT_INITIALIZED'
  | 'NETWORK_SERVICE_UNAVAILABLE'
  | 'NETWORK_PEER_UNREACHABLE'
  | 'NETWORK_TIMEOUT'
  | 'INTERNAL_ERROR';

/** Known backend codes plus future structured codes not yet shipped by this frontend. */
export type ErrorCode = KnownErrorCode | (string & {});

export interface ErrorResponse {
  code: ErrorCode;
  message: string;
  details?: string;
  recovery?: string;
}

export class HarborError extends Error {
  readonly code: ErrorCode;
  readonly details?: string;
  readonly recovery?: string;
  readonly command?: string;
  readonly cause?: unknown;

  constructor(response: ErrorResponse, context?: { command?: string; cause?: unknown }) {
    super(response.message);
    this.name = 'HarborError';
    this.code = response.code;
    this.details = response.details;
    this.recovery = response.recovery;
    this.command = context?.command;
    if (context && 'cause' in context) this.cause = context.cause;
  }

  static fromUnknown(error: unknown, context?: { command?: string }): HarborError {
    if (error instanceof HarborError) {
      if (!context?.command || error.command === context.command) return error;
      return new HarborError(error, { command: context.command, cause: error });
    }

    if (isErrorResponse(error)) {
      return new HarborError(
        {
          code: error.code,
          message: error.message.trim(),
          details: optionalString(error.details),
          recovery: optionalString(error.recovery),
        },
        { command: context?.command, cause: error },
      );
    }

    if (error instanceof Error) {
      return new HarborError(
        {
          code: 'INTERNAL_ERROR',
          message: error.message.trim() || error.name || 'An unexpected error occurred',
          details: error.stack,
        },
        { command: context?.command, cause: error },
      );
    }

    return new HarborError(
      {
        code: 'INTERNAL_ERROR',
        message: getErrorMessage(error),
        details: 'The native command rejected with an unstructured value.',
      },
      { command: context?.command, cause: error },
    );
  }

  isRecoverable(): boolean {
    const recoverableCodes: ErrorCode[] = [
      'NETWORK_TIMEOUT',
      'NETWORK_CONNECTION_FAILED',
      'NETWORK_PEER_UNREACHABLE',
      'NETWORK_NOT_INITIALIZED',
      'NETWORK_SERVICE_UNAVAILABLE',
      'IDENTITY_LOCKED',
      'IDENTITY_INVALID_PASSPHRASE',
      'VALIDATION_ERROR',
    ];
    return recoverableCodes.includes(this.code);
  }

  isCritical(): boolean {
    const criticalCodes: ErrorCode[] = [
      'DATABASE_ERROR',
      'DATABASE_CONNECTION',
      'CRYPTO_ERROR',
      'INTERNAL_ERROR',
    ];
    return criticalCodes.includes(this.code);
  }
}

export function isErrorResponse(value: unknown): value is ErrorResponse {
  if (typeof value !== 'object' || value === null) return false;
  const obj = value as Record<string, unknown>;
  return (
    typeof obj.code === 'string' &&
    obj.code.length > 0 &&
    typeof obj.message === 'string' &&
    obj.message.trim().length > 0 &&
    (obj.details === undefined || typeof obj.details === 'string') &&
    (obj.recovery === undefined || typeof obj.recovery === 'string')
  );
}

function optionalString(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim().length > 0 ? value : undefined;
}

export function getErrorMessage(error: unknown): string {
  if (error instanceof HarborError) {
    return error.message;
  }
  if (isErrorResponse(error)) {
    return error.message;
  }
  if (error instanceof Error) {
    return error.message.trim() || error.name || 'An unexpected error occurred';
  }
  if (typeof error === 'string') return error.trim() || 'An unexpected error occurred';
  if (typeof error === 'number' || typeof error === 'bigint' || typeof error === 'boolean') {
    return String(error);
  }
  if (typeof error === 'object' && error !== null) return 'An unexpected error occurred';
  return 'An unexpected error occurred';
}
