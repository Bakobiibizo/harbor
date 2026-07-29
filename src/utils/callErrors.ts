import { isErrorResponse, type ErrorResponse } from './errors';

export type CallFailureCode =
  | 'permission_denied'
  | 'missing_media_api'
  | 'missing_device'
  | 'signaling_failed'
  | 'timeout'
  | 'ice_failed'
  | 'turn_required'
  | 'busy'
  | 'rejected'
  | 'incompatible_peer'
  | 'unknown';

export interface CallFailure {
  code: CallFailureCode;
  message: string;
  recovery: string;
  diagnostic: {
    context?: string;
    sourceCode?: string;
    sourceName?: string;
  };
}

const COPY: Record<CallFailureCode, Pick<CallFailure, 'message' | 'recovery'>> = {
  permission_denied: {
    message: 'Harbor does not have permission to use your microphone or camera.',
    recovery: 'Allow microphone and camera access in system settings, then try the call again.',
  },
  missing_media_api: {
    message: 'This Harbor build cannot access the system audio or video API.',
    recovery:
      'Restart Harbor. If this continues, update Harbor and include diagnostics in a bug report.',
  },
  missing_device: {
    message: 'No usable microphone or camera was found.',
    recovery: 'Connect or enable a media device, check system privacy settings, and try again.',
  },
  signaling_failed: {
    message: 'Harbor could not reach this contact to set up the call.',
    recovery: 'Confirm both people are online and connected to a relay, then try again.',
  },
  timeout: {
    message: 'The call timed out before a secure media connection was established.',
    recovery: 'Try again. If both people use strict NAT, configure a TURN server in Settings.',
  },
  ice_failed: {
    message: 'Harbor could not establish a direct media path.',
    recovery: 'Try another network or configure a TURN server in Settings.',
  },
  turn_required: {
    message: 'These networks require a TURN server to relay encrypted call media.',
    recovery: 'Configure a trusted TURN server in Settings and try again.',
  },
  busy: {
    message: 'This contact is already in another call.',
    recovery: 'Wait for them to finish and try again.',
  },
  rejected: {
    message: 'The call was declined.',
    recovery: 'You can try again later or send a message instead.',
  },
  incompatible_peer: {
    message: 'The other Harbor app does not support this call setup.',
    recovery: 'Make sure both people are using the latest Harbor version.',
  },
  unknown: {
    message: 'Harbor could not start the call.',
    recovery: 'Try again. If it continues, submit a bug report with call diagnostics.',
  },
};

function source(error: unknown): { message: string; code?: string; name?: string } {
  if (isErrorResponse(error)) {
    const response = error as ErrorResponse;
    return { message: response.details || response.message, code: response.code };
  }
  if (error instanceof DOMException) {
    return { message: error.message, name: error.name };
  }
  if (error instanceof Error) {
    return { message: error.message, name: error.name };
  }
  if (typeof error === 'string') {
    return { message: error };
  }
  if (typeof error === 'object' && error !== null) {
    const record = error as Record<string, unknown>;
    return {
      message: typeof record.message === 'string' ? record.message : '',
      code: typeof record.code === 'string' ? record.code : undefined,
      name: typeof record.name === 'string' ? record.name : undefined,
    };
  }
  return { message: '' };
}

const CALL_CODES = new Set<CallFailureCode>([
  'permission_denied',
  'missing_media_api',
  'missing_device',
  'signaling_failed',
  'timeout',
  'ice_failed',
  'turn_required',
  'busy',
  'rejected',
  'incompatible_peer',
  'unknown',
]);

const BACKEND_CODES: Readonly<Record<string, CallFailureCode>> = {
  PERMISSION_DENIED: 'permission_denied',
  NETWORK_TIMEOUT: 'timeout',
  NETWORK_ERROR: 'signaling_failed',
  NETWORK_CONNECTION_FAILED: 'signaling_failed',
  NETWORK_NOT_INITIALIZED: 'signaling_failed',
  NETWORK_SERVICE_UNAVAILABLE: 'signaling_failed',
  NETWORK_PEER_UNREACHABLE: 'signaling_failed',
};

const DOM_EXCEPTION_NAMES: Readonly<Record<string, CallFailureCode>> = {
  NotAllowedError: 'permission_denied',
  SecurityError: 'permission_denied',
  NotSupportedError: 'missing_media_api',
  NotFoundError: 'missing_device',
  DevicesNotFoundError: 'missing_device',
  TimeoutError: 'timeout',
};

/**
 * Classification is deliberately code-only. User-facing or localized prose is
 * never an API contract and changing it must not alter call state transitions.
 */
function classify(code = '', name = ''): CallFailureCode {
  if (CALL_CODES.has(code as CallFailureCode)) return code as CallFailureCode;
  return BACKEND_CODES[code] ?? DOM_EXCEPTION_NAMES[name] ?? 'unknown';
}

export function callFailureFrom(error: unknown, context?: string): CallFailure {
  const raw = source(error);
  const code = classify(raw.code, raw.name);
  return {
    code,
    ...COPY[code],
    diagnostic: {
      ...(context ? { context } : {}),
      ...(raw.code ? { sourceCode: raw.code } : {}),
      ...(raw.name ? { sourceName: raw.name } : {}),
    },
  };
}
