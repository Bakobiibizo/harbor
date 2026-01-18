/** Call state */
export type CallState = 'ringing' | 'incoming' | 'connected' | 'ended';

/** Hangup reason */
export type HangupReason = 'normal' | 'busy' | 'declined' | 'error';

/** An outgoing offer result */
export interface OfferResult {
  callId: string;
  callerPeerId: string;
  calleePeerId: string;
  sdp: string;
  timestamp: number;
  signature: number[];
}

/** An answer result */
export interface AnswerResult {
  callId: string;
  callerPeerId: string;
  calleePeerId: string;
  sdp: string;
  timestamp: number;
  signature: number[];
}

/** An ICE candidate result */
export interface IceResult {
  callId: string;
  senderPeerId: string;
  candidate: string;
  sdpMid: string | null;
  sdpMlineIndex: number | null;
  timestamp: number;
  signature: number[];
}

/** A hangup result */
export interface HangupResult {
  callId: string;
  senderPeerId: string;
  reason: string;
  timestamp: number;
  signature: number[];
}
