import { describe, expect, it } from 'vitest';
import { callFailureFrom } from './callErrors';

describe('callFailureFrom', () => {
  it('turns structured Tauri errors into human signaling failures', () => {
    const failure = callFailureFrom({
      code: 'NETWORK_PEER_UNREACHABLE',
      message: 'Could not reach the peer',
      details: 'Network error: peer unreachable',
    });
    expect(failure.code).toBe('signaling_failed');
    expect(failure.message).not.toContain('[object Object]');
    expect(failure.diagnostic).toEqual({ sourceCode: 'NETWORK_PEER_UNREACHABLE' });
  });

  it.each([
    [new DOMException('denied', 'NotAllowedError'), 'permission_denied'],
    [new DOMException('media capture API unavailable', 'NotSupportedError'), 'missing_media_api'],
    [new DOMException('no microphone', 'NotFoundError'), 'missing_device'],
    ['ICE failed without a usable TURN server', 'turn_required'],
    ['call timed out', 'timeout'],
    ['remote is busy', 'busy'],
    ['call declined', 'rejected'],
  ])('classifies %s as %s', (error, expected) => {
    expect(callFailureFrom(error).code).toBe(expected);
  });

  it('never renders opaque objects or serializes their contents into diagnostics', () => {
    const failure = callFailureFrom({ privateKey: 'do-not-copy', sdp: 'private-session' }, 'start');
    expect(failure.code).toBe('unknown');
    expect(failure.message).toBe('Harbor could not start the call.');
    expect(JSON.stringify(failure)).not.toContain('do-not-copy');
    expect(JSON.stringify(failure)).not.toContain('private-session');
  });
});
