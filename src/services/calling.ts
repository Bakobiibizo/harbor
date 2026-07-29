import type {
  OfferResult,
  AnswerResult,
  IceResult,
  HangupResult,
  HangupReason,
  CallSession,
  GroupMembershipAction,
  GroupMembershipSignal,
  GroupCallRoom,
} from '../types';
import { invokeCommand } from './command';

/** Calling service - wraps Tauri commands for voice calling */
export const callingService = {
  /** Get active calls persisted by the backend */
  async getActiveCalls(): Promise<CallSession[]> {
    return invokeCommand('get_active_calls');
  },

  /** Get call history persisted by the backend */
  async getCallHistory(limit = 100): Promise<CallSession[]> {
    return invokeCommand('get_call_history', { limit });
  },

  async getActiveGroupCalls(): Promise<GroupCallRoom[]> {
    return invokeCommand('get_active_group_calls');
  },

  async sendGroupMembership(input: {
    roomId?: string;
    creatorPeerId?: string;
    action: GroupMembershipAction;
    rosterVersion: number;
    participants: string[];
    mediaMode: 'audio' | 'video';
  }): Promise<GroupMembershipSignal> {
    return invokeCommand('send_group_membership', { input });
  },

  /** Start a call (create an offer) */
  async startCall(calleePeerId: string, sdp: string): Promise<OfferResult> {
    return invokeCommand('start_call', { calleePeerId, sdp });
  },

  /** Answer a call */
  async answerCall(callId: string, callerPeerId: string, sdp: string): Promise<AnswerResult> {
    return invokeCommand('answer_call', { callId, callerPeerId, sdp });
  },

  /** Send an ICE candidate */
  async sendIceCandidate(
    callId: string,
    targetPeerId: string,
    candidate: string,
    sdpMid?: string,
    sdpMlineIndex?: number,
  ): Promise<IceResult> {
    return invokeCommand('send_ice_candidate', {
      callId,
      targetPeerId,
      candidate,
      sdpMid,
      sdpMlineIndex,
    });
  },

  /** Hang up a call */
  async hangupCall(
    callId: string,
    targetPeerId: string,
    reason?: HangupReason,
  ): Promise<HangupResult> {
    return invokeCommand('hangup_call', { callId, targetPeerId, reason });
  },

  /** Decline an incoming call */
  async declineCall(callId: string, callerPeerId: string): Promise<HangupResult> {
    return invokeCommand('decline_call', { callId, callerPeerId });
  },

  /** Send a busy response for an incoming call */
  async busyCall(callId: string, callerPeerId: string): Promise<HangupResult> {
    return invokeCommand('busy_call', { callId, callerPeerId });
  },

  /** Process an incoming offer (validate it) */
  async processOffer(
    callId: string,
    callerPeerId: string,
    calleePeerId: string,
    sdp: string,
    timestamp: number,
    signature: number[],
  ): Promise<void> {
    return invokeCommand('process_offer', {
      callId,
      callerPeerId,
      calleePeerId,
      sdp,
      timestamp,
      signature,
    });
  },

  /** Process an incoming answer (validate it) */
  async processAnswer(
    callId: string,
    callerPeerId: string,
    calleePeerId: string,
    sdp: string,
    timestamp: number,
    signature: number[],
  ): Promise<void> {
    return invokeCommand('process_answer', {
      callId,
      callerPeerId,
      calleePeerId,
      sdp,
      timestamp,
      signature,
    });
  },

  /** Process an incoming ICE candidate (validate it) */
  async processIceCandidate(
    callId: string,
    senderPeerId: string,
    candidate: string,
    sdpMid: string | undefined,
    sdpMlineIndex: number | undefined,
    timestamp: number,
    signature: number[],
  ): Promise<void> {
    return invokeCommand('process_ice_candidate', {
      callId,
      senderPeerId,
      candidate,
      sdpMid,
      sdpMlineIndex,
      timestamp,
      signature,
    });
  },

  /** Process an incoming hangup (validate it) */
  async processHangup(
    callId: string,
    senderPeerId: string,
    reason: string,
    timestamp: number,
    signature: number[],
  ): Promise<void> {
    return invokeCommand('process_hangup', {
      callId,
      senderPeerId,
      reason,
      timestamp,
      signature,
    });
  },
};
