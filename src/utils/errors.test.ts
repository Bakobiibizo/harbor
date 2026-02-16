import { describe, it, expect } from 'vitest';
import { HarborError, isErrorResponse, getErrorMessage } from './errors';

describe('HarborError', () => {
  describe('constructor', () => {
    it('should create an error with code, message, and optional fields', () => {
      const error = new HarborError({
        code: 'DATABASE_ERROR',
        message: 'Connection failed',
        details: 'Timeout after 30s',
        recovery: 'Check your database configuration',
      });

      expect(error.name).toBe('HarborError');
      expect(error.code).toBe('DATABASE_ERROR');
      expect(error.message).toBe('Connection failed');
      expect(error.details).toBe('Timeout after 30s');
      expect(error.recovery).toBe('Check your database configuration');
      expect(error).toBeInstanceOf(Error);
    });

    it('should work without optional fields', () => {
      const error = new HarborError({
        code: 'INTERNAL_ERROR',
        message: 'Something went wrong',
      });

      expect(error.details).toBeUndefined();
      expect(error.recovery).toBeUndefined();
    });
  });

  describe('fromUnknown', () => {
    it('should return the same HarborError if given one', () => {
      const original = new HarborError({
        code: 'NETWORK_ERROR',
        message: 'Network issue',
      });

      const result = HarborError.fromUnknown(original);
      expect(result).toBe(original);
    });

    it('should convert an object with code and message to HarborError', () => {
      const obj = {
        code: 'VALIDATION_ERROR',
        message: 'Invalid input',
        details: 'Name is required',
      };

      const result = HarborError.fromUnknown(obj);
      expect(result).toBeInstanceOf(HarborError);
      expect(result.code).toBe('VALIDATION_ERROR');
      expect(result.message).toBe('Invalid input');
    });

    it('should convert a standard Error to HarborError with INTERNAL_ERROR code', () => {
      const error = new Error('Something broke');

      const result = HarborError.fromUnknown(error);
      expect(result).toBeInstanceOf(HarborError);
      expect(result.code).toBe('INTERNAL_ERROR');
      expect(result.message).toBe('Something broke');
      expect(result.details).toBeDefined(); // should include stack
    });

    it('should convert a string to HarborError', () => {
      const result = HarborError.fromUnknown('string error');
      expect(result).toBeInstanceOf(HarborError);
      expect(result.code).toBe('INTERNAL_ERROR');
      expect(result.message).toBe('string error');
    });

    it('should convert null/undefined to HarborError', () => {
      const result = HarborError.fromUnknown(null);
      expect(result).toBeInstanceOf(HarborError);
      expect(result.code).toBe('INTERNAL_ERROR');
      expect(result.message).toBe('null');
    });
  });

  describe('isRecoverable', () => {
    it('should return true for recoverable error codes', () => {
      const recoverableCodes = [
        'NETWORK_TIMEOUT',
        'NETWORK_CONNECTION_FAILED',
        'NETWORK_PEER_UNREACHABLE',
        'IDENTITY_LOCKED',
        'IDENTITY_INVALID_PASSPHRASE',
        'VALIDATION_ERROR',
      ] as const;

      for (const code of recoverableCodes) {
        const error = new HarborError({ code, message: 'test' });
        expect(error.isRecoverable()).toBe(true);
      }
    });

    it('should return false for non-recoverable error codes', () => {
      const nonRecoverableCodes = [
        'DATABASE_ERROR',
        'CRYPTO_ERROR',
        'INTERNAL_ERROR',
        'PERMISSION_DENIED',
      ] as const;

      for (const code of nonRecoverableCodes) {
        const error = new HarborError({ code, message: 'test' });
        expect(error.isRecoverable()).toBe(false);
      }
    });
  });

  describe('isCritical', () => {
    it('should return true for critical error codes', () => {
      const criticalCodes = [
        'DATABASE_ERROR',
        'DATABASE_CONNECTION',
        'CRYPTO_ERROR',
        'INTERNAL_ERROR',
      ] as const;

      for (const code of criticalCodes) {
        const error = new HarborError({ code, message: 'test' });
        expect(error.isCritical()).toBe(true);
      }
    });

    it('should return false for non-critical error codes', () => {
      const nonCriticalCodes = ['NETWORK_ERROR', 'VALIDATION_ERROR', 'NOT_FOUND'] as const;

      for (const code of nonCriticalCodes) {
        const error = new HarborError({ code, message: 'test' });
        expect(error.isCritical()).toBe(false);
      }
    });
  });
});

describe('isErrorResponse', () => {
  it('should return true for valid error response objects', () => {
    expect(isErrorResponse({ code: 'INTERNAL_ERROR', message: 'Error' })).toBe(true);
    expect(isErrorResponse({ code: 'DATABASE_ERROR', message: 'DB down', details: 'extra' })).toBe(
      true,
    );
  });

  it('should return false for invalid objects', () => {
    expect(isErrorResponse(null)).toBe(false);
    expect(isErrorResponse(undefined)).toBe(false);
    expect(isErrorResponse('string')).toBe(false);
    expect(isErrorResponse(42)).toBe(false);
    expect(isErrorResponse({ code: 123, message: 'test' })).toBe(false);
    expect(isErrorResponse({ code: 'INTERNAL_ERROR' })).toBe(false);
    expect(isErrorResponse({ message: 'test' })).toBe(false);
    expect(isErrorResponse({})).toBe(false);
  });
});

describe('getErrorMessage', () => {
  it('should extract message from HarborError', () => {
    const error = new HarborError({ code: 'INTERNAL_ERROR', message: 'Harbor error msg' });
    expect(getErrorMessage(error)).toBe('Harbor error msg');
  });

  it('should extract message from ErrorResponse object', () => {
    expect(getErrorMessage({ code: 'NOT_FOUND', message: 'Not found' })).toBe('Not found');
  });

  it('should extract message from standard Error', () => {
    expect(getErrorMessage(new Error('Standard error'))).toBe('Standard error');
  });

  it('should stringify unknown values', () => {
    expect(getErrorMessage('string error')).toBe('string error');
    expect(getErrorMessage(42)).toBe('42');
    expect(getErrorMessage(null)).toBe('null');
  });
});
