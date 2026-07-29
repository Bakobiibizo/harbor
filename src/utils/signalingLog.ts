import type { SignalingEnvelope } from '../types';

export type SignalingLogDirection = 'inbound' | 'outbound';

export interface SignalingLogSummary {
  kind: SignalingEnvelope['payload']['type'];
  correlationId: string;
  direction: SignalingLogDirection;
  result: string;
}

/**
 * The only signaling shape permitted at a frontend logging boundary.
 * Never spread or attach the envelope: its payload may contain SDP, ICE
 * credentials/candidates, DTLS fingerprints, signatures, and group nonces.
 */
export function summarizeSignalingForLog(
  envelope: SignalingEnvelope,
  direction: SignalingLogDirection,
  result: string,
): SignalingLogSummary {
  return {
    kind: envelope.payload.type,
    correlationId:
      envelope.payload.type === 'group_membership'
        ? envelope.payload.payload.roomId
        : envelope.payload.payload.callId,
    direction,
    result,
  };
}
