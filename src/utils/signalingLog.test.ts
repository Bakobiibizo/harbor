import { describe, expect, it } from 'vitest';
import type { SignalingEnvelope, SignalingPayload } from '../types';
import { summarizeSignalingForLog } from './signalingLog';

const signature = [91, 92, 93, 94];

function envelope(payload: SignalingPayload): SignalingEnvelope {
  return {
    senderPeerId: 'sender-peer',
    recipientPeerId: 'recipient-peer',
    payload,
  };
}

const cases: Array<{
  kind: SignalingPayload['type'];
  envelope: SignalingEnvelope;
  secret: string;
}> = [
  {
    kind: 'offer',
    secret: 'offer-ice-password-secret',
    envelope: envelope({
      type: 'offer',
      payload: {
        callId: 'call-offer',
        callerPeerId: 'sender-peer',
        calleePeerId: 'recipient-peer',
        sdp: 'a=ice-pwd:offer-ice-password-secret\r\na=fingerprint:offer-fingerprint-secret',
        timestamp: 1,
        signature,
      },
    }),
  },
  {
    kind: 'answer',
    secret: 'answer-ice-ufrag-secret',
    envelope: envelope({
      type: 'answer',
      payload: {
        callId: 'call-answer',
        callerPeerId: 'recipient-peer',
        calleePeerId: 'sender-peer',
        sdp: 'a=ice-ufrag:answer-ice-ufrag-secret\r\na=fingerprint:answer-fingerprint-secret',
        timestamp: 1,
        signature,
      },
    }),
  },
  {
    kind: 'ice',
    secret: 'candidate:ice-candidate-secret',
    envelope: envelope({
      type: 'ice',
      payload: {
        callId: 'call-ice',
        senderPeerId: 'sender-peer',
        candidate: 'candidate:ice-candidate-secret',
        sdpMid: 'ice-control-token-secret',
        sdpMlineIndex: 0,
        timestamp: 1,
        signature,
      },
    }),
  },
  {
    kind: 'hangup',
    secret: 'hangup-signature-secret',
    envelope: envelope({
      type: 'hangup',
      payload: {
        callId: 'call-hangup',
        senderPeerId: 'sender-peer',
        reason: 'normal',
        timestamp: 1,
        signature: Array.from(new TextEncoder().encode('hangup-signature-secret')),
      },
    }),
  },
  {
    kind: 'group_membership',
    secret: 'group-nonce-control-token-secret',
    envelope: envelope({
      type: 'group_membership',
      payload: {
        roomId: 'room-group',
        creatorPeerId: 'sender-peer',
        senderPeerId: 'sender-peer',
        action: 'invite',
        topology: 'relay_assisted_mesh_v1',
        rosterVersion: 1,
        participants: ['recipient-peer', 'sender-peer'],
        mediaMode: 'video',
        nonce: 'group-nonce-control-token-secret',
        timestamp: 1,
        signature,
      },
    }),
  },
];

describe('summarizeSignalingForLog', () => {
  it.each(cases)('redacts $kind payload secrets while retaining trace fields', (testCase) => {
    const summary = summarizeSignalingForLog(testCase.envelope, 'inbound', 'verified');
    const logged = JSON.stringify(summary);
    const payload = testCase.envelope.payload;
    const correlationId =
      payload.type === 'group_membership' ? payload.payload.roomId : payload.payload.callId;

    expect(summary).toEqual({
      kind: testCase.kind,
      correlationId,
      direction: 'inbound',
      result: 'verified',
    });
    expect(logged).not.toContain(testCase.secret);
    expect(logged).not.toContain('ice-pwd');
    expect(logged).not.toContain('ice-ufrag');
    expect(logged).not.toContain('fingerprint');
    expect(logged).not.toContain('candidate:');
    expect(logged).not.toContain('signature');
    expect(logged).not.toContain('nonce');
    expect(logged).not.toContain('control-token');
  });
});
