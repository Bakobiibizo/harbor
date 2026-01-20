export type ErrorCode =
  | "DATABASE_ERROR"
  | "DATABASE_CONNECTION"
  | "DATABASE_MIGRATION"
  | "CRYPTO_ERROR"
  | "CRYPTO_KEY_GENERATION"
  | "CRYPTO_ENCRYPTION"
  | "CRYPTO_DECRYPTION"
  | "IDENTITY_ERROR"
  | "IDENTITY_NOT_FOUND"
  | "IDENTITY_LOCKED"
  | "IDENTITY_INVALID_PASSPHRASE"
  | "SERIALIZATION_ERROR"
  | "IO_ERROR"
  | "INVALID_DATA"
  | "NOT_FOUND"
  | "ALREADY_EXISTS"
  | "PERMISSION_DENIED"
  | "UNAUTHORIZED"
  | "VALIDATION_ERROR"
  | "NETWORK_ERROR"
  | "NETWORK_CONNECTION_FAILED"
  | "NETWORK_PEER_UNREACHABLE"
  | "NETWORK_TIMEOUT"
  | "INTERNAL_ERROR";

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

  constructor(response: ErrorResponse) {
    super(response.message);
    this.name = "HarborError";
    this.code = response.code;
    this.details = response.details;
    this.recovery = response.recovery;
  }

  static fromUnknown(error: unknown): HarborError {
    if (error instanceof HarborError) {
      return error;
    }

    if (typeof error === "object" && error !== null) {
      const errorObj = error as Record<string, unknown>;
      if ("code" in errorObj && "message" in errorObj) {
        return new HarborError(errorObj as unknown as ErrorResponse);
      }
    }

    if (error instanceof Error) {
      return new HarborError({
        code: "INTERNAL_ERROR",
        message: error.message,
        details: error.stack,
      });
    }

    return new HarborError({
      code: "INTERNAL_ERROR",
      message: String(error),
    });
  }

  isRecoverable(): boolean {
    const recoverableCodes: ErrorCode[] = [
      "NETWORK_TIMEOUT",
      "NETWORK_CONNECTION_FAILED",
      "NETWORK_PEER_UNREACHABLE",
      "IDENTITY_LOCKED",
      "IDENTITY_INVALID_PASSPHRASE",
      "VALIDATION_ERROR",
    ];
    return recoverableCodes.includes(this.code);
  }

  isCritical(): boolean {
    const criticalCodes: ErrorCode[] = [
      "DATABASE_ERROR",
      "DATABASE_CONNECTION",
      "CRYPTO_ERROR",
      "INTERNAL_ERROR",
    ];
    return criticalCodes.includes(this.code);
  }
}

export function isErrorResponse(value: unknown): value is ErrorResponse {
  if (typeof value !== "object" || value === null) return false;
  const obj = value as Record<string, unknown>;
  return typeof obj.code === "string" && typeof obj.message === "string";
}

export function getErrorMessage(error: unknown): string {
  if (error instanceof HarborError) {
    return error.message;
  }
  if (isErrorResponse(error)) {
    return error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
