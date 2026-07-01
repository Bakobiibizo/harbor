/** Call state */
export type CallState = 'ringing' | 'incoming' | 'connected' | 'ended';

/** Hangup reason */
export type HangupReason = 'normal' | 'busy' | 'declined' | 'error';

/** Call direction relative to the local account */
export type CallDirection = 'outgoing' | 'incoming';

/** Persisted call media kind */
export type CallMediaKind = 'audio' | 'video';

/** Persisted active call/history row */
export interface CallSession {
  callId: string;
  peerId: string;
  callerPeerId: string | null;
  calleePeerId: string | null;
  direction: CallDirection;
  mediaKind: CallMediaKind;
  state: CallState;
  startedAt: number;
  endedAt: number | null;
  durationSeconds: number | null;
  terminalReason: string | null;
}

/** Supported ICE server URL schemes */
export type IceServerProtocol = 'stun' | 'stuns' | 'turn' | 'turns';

/** How a TURN credential may be retained by settings persistence */
export type TurnCredentialPersistence = 'session' | 'device';

/** Operator-configured ICE server entry for WebRTC calls */
export interface IceServerConfig {
  id: string;
  urls: string[];
  username?: string;
  credential?: string;
  credentialPersistence?: TurnCredentialPersistence;
}

/** Raw ICE server form/config input before validation */
export interface IceServerInput {
  id?: string;
  urls: string | string[];
  username?: string;
  credential?: string;
  credentialPersistence?: TurnCredentialPersistence;
}

export type IceServerValidationResult =
  | { ok: true; server: IceServerConfig }
  | { ok: false; error: string };

/** Redacted ICE server entry safe for display/logging */
export interface RedactedIceServerConfig {
  id: string;
  urls: string[];
  username?: string;
  credentialPersistence?: TurnCredentialPersistence;
  hasCredential: boolean;
  redactedCredential?: string;
}

export type CallIceErrorCode =
  | 'strict-nat-no-turn'
  | 'relay-only-without-turn'
  | 'turn-credentials-missing'
  | 'ice-candidate-error'
  | 'ice-failed';

export interface CallIceError {
  code: CallIceErrorCode;
  message: string;
  recoverable: boolean;
  details?: Record<string, unknown>;
}

export interface CallIceStateSnapshot {
  iceGatheringState: RTCIceGatheringState;
  iceConnectionState: RTCIceConnectionState;
  connectionState: RTCPeerConnectionState;
  error: CallIceError | null;
}

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
  targetPeerId: string;
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
  targetPeerId: string;
  reason: string;
  timestamp: number;
  signature: number[];
}

export type SignalingPayload =
  | { type: 'offer'; payload: OfferResult }
  | { type: 'answer'; payload: AnswerResult }
  | { type: 'ice'; payload: Omit<IceResult, 'targetPeerId'> }
  | { type: 'hangup'; payload: Omit<HangupResult, 'targetPeerId'> }
  | { type: 'decline'; payload: Omit<HangupResult, 'targetPeerId'> }
  | { type: 'busy'; payload: Omit<HangupResult, 'targetPeerId'> };

export interface SignalingEnvelope {
  senderPeerId: string;
  recipientPeerId: string;
  payload: SignalingPayload;
}
