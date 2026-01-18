import { invoke } from '@tauri-apps/api/core';
import type { OfferResult, AnswerResult, IceResult, HangupResult, HangupReason } from '../types';

/** Calling service - wraps Tauri commands for voice calling */
export const callingService = {
  /** Start a call (create an offer) */
  async startCall(calleePeerId: string, sdp: string): Promise<OfferResult> {
    return invoke<OfferResult>('start_call', { calleePeerId, sdp });
  },

  /** Answer a call */
  async answerCall(callId: string, callerPeerId: string, sdp: string): Promise<AnswerResult> {
    return invoke<AnswerResult>('answer_call', { callId, callerPeerId, sdp });
  },

  /** Send an ICE candidate */
  async sendIceCandidate(
    callId: string,
    candidate: string,
    sdpMid?: string,
    sdpMlineIndex?: number,
  ): Promise<IceResult> {
    return invoke<IceResult>('send_ice_candidate', {
      callId,
      candidate,
      sdpMid,
      sdpMlineIndex,
    });
  },

  /** Hang up a call */
  async hangupCall(callId: string, reason?: HangupReason): Promise<HangupResult> {
    return invoke<HangupResult>('hangup_call', { callId, reason });
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
    return invoke<void>('process_offer', {
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
    return invoke<void>('process_answer', {
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
    return invoke<void>('process_ice_candidate', {
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
    return invoke<void>('process_hangup', {
      callId,
      senderPeerId,
      reason,
      timestamp,
      signature,
    });
  },
};
