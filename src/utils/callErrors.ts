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
    recovery: 'Restart Harbor. If this continues, update Harbor and include diagnostics in a bug report.',
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

function classify(message: string, code = '', name = ''): CallFailureCode {
  const value = `${code} ${name} ${message}`.toLowerCase();
  if (/notallowed|securityerror|permission.?denied|permission denied/.test(value)) {
    return 'permission_denied';
  }
  if (/not supported|notsupported|media.?devices|media capture api|audio api|getusermedia/.test(value)) {
    return 'missing_media_api';
  }
  if (/notfounderror|devicesnotfound|missing.?device|no (usable )?(microphone|camera|audio device)/.test(value)) {
    return 'missing_device';
  }
  if (/turn.*(required|missing|unavailable)|(?:without|no).*turn|relay-only/.test(value)) {
    return 'turn_required';
  }
  if (/ice.*(failed|failure)|media path/.test(value)) return 'ice_failed';
  if (/network_timeout|timeout|timed out/.test(value)) return 'timeout';
  if (/busy|already in another call/.test(value)) return 'busy';
  if (/declin|reject/.test(value)) return 'rejected';
  if (/incompatib|unsupported (peer|version|offer)|webrtc is unavailable/.test(value)) {
    return 'incompatible_peer';
  }
  if (/network|signaling|signal|peer.?unreachable|offline|could not reach|connection failed/.test(value)) {
    return 'signaling_failed';
  }
  return 'unknown';
}

export function callFailureFrom(error: unknown, context?: string): CallFailure {
  const raw = source(error);
  const code = classify(raw.message, raw.code, raw.name);
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
